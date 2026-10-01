//! Plant-in-the-loop model: a first-order motor with command transport delay, driven through
//! PXR by a periodic controller, sensor feed and supervisor. Results are simulation evidence
//! about stopping behavior under this model, not measurements of a physical actuator.
use crate::SimDriver;
use pxr_runtime_core::{profile::*, *};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct MotorModel {
    /// First-order velocity response time constant.
    pub time_constant_ms: f64,
    /// Delay between a driver call and the actuator acting on it (bus, PWM update, etc.).
    pub command_delay_ms: u64,
}
impl Default for MotorModel {
    fn default() -> Self {
        Self {
            time_constant_ms: 40.0,
            command_delay_ms: 5,
        }
    }
}

/// Timing of the surrounding control system.
#[derive(Clone, Copy, Debug)]
pub struct Loop {
    pub supervisor_period_ms: u64,
    pub sensor_period_ms: u64,
    pub command_period_ms: u64,
    pub command_valid_for_ms: u32,
    pub cruise_mm_s: i32,
    pub fault_at_ms: u64,
    pub run_until_ms: u64,
}
impl Default for Loop {
    fn default() -> Self {
        Self {
            supervisor_period_ms: 10,
            sensor_period_ms: 10,
            command_period_ms: 20,
            command_valid_for_ms: 100,
            cruise_mm_s: 800,
            fault_at_ms: 1000,
            run_until_ms: 3000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Planner/network goes silent: no more commands or renewals.
    ControllerSilent,
    /// Local e-stop input, delivered through `emergency_stop`.
    EstopPressed,
    /// Trusted sensors report an obstacle.
    ObstacleDetected,
    /// The sensor feed stops while commands keep arriving.
    SensorFeedStale,
    /// The thread running PXR stops for this long, then resumes.
    SupervisorStall(u64),
}

#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub fault: Fault,
    /// Fault to first motor fallback call.
    pub detected_after_ms: Option<u64>,
    /// Fault to the motor settling below 1% of cruise speed for the rest of the run.
    pub stopped_after_ms: Option<u64>,
    /// Distance travelled after the fault.
    pub coast_mm: f64,
    /// Detection bound derived from the configuration and loop timing.
    pub detection_bound_ms: u64,
    /// Detection bound plus actuator delay and settling to 1% (tau * ln 100).
    pub stop_bound_ms: u64,
}

struct PlantDriver {
    sim: SimDriver,
    model: MotorModel,
    now: u64,
    pending: VecDeque<(u64, f64)>,
    target: f64,
    velocity: f64,
    first_fallback: Option<u64>,
}
impl PlantDriver {
    fn command(&mut self, velocity: i32) {
        self.pending
            .push_back((self.now + self.model.command_delay_ms, velocity as f64));
    }
    fn step(&mut self) -> f64 {
        self.now += 1;
        while self.pending.front().is_some_and(|(at, _)| *at <= self.now) {
            self.target = self.pending.pop_front().unwrap().1;
        }
        self.velocity +=
            (self.target - self.velocity) * (1.0 - (-1.0 / self.model.time_constant_ms).exp());
        self.velocity.abs() / 1000.0
    }
}
impl Driver for PlantDriver {
    fn observe(&mut self, resource: u8) -> Result<Observation, DriverError> {
        self.sim.observe(resource)
    }
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        self.sim.execute(capability, parameters)?;
        match capability {
            DRIVE => self.command(parameters[0]),
            STOP => self.command(0),
            _ => {}
        }
        Ok(())
    }
    fn fallback(&mut self, resource: u8, reason: Reason) -> Result<(), DriverError> {
        if resource == 0 {
            self.command(0);
            self.first_fallback.get_or_insert(self.now);
        }
        self.sim.fallback(resource, reason)
    }
}

pub fn run(fault: Fault, model: MotorModel, config: Config, timing: Loop) -> Outcome {
    const OWNER: u64 = 7;
    let mut d = PlantDriver {
        sim: SimDriver::default(),
        model,
        now: 0,
        pending: VecDeque::new(),
        target: 0.0,
        velocity: 0.0,
        first_fallback: None,
    };
    let mut rt = Runtime::new(config, &CAPABILITIES, 42, 0, 0, &mut d).expect("valid profile");
    let request = LeaseRequest {
        owner: OWNER,
        resource: 0,
        capabilities: (1 << DRIVE) | (1 << STOP),
        duration_ms: config.max_lease_ms,
        limits: [Bounds::ANY; 2],
    };
    let mut lease = rt.acquire(request, 0, &mut d).expect("lease");
    let (mut id, mut renewal) = (0u64, 0u64);
    let renew_every = (config.max_lease_ms / 4).max(1) as u64;
    let stop_threshold = timing.cruise_mm_s.unsigned_abs() as f64 / 100.0;
    let (mut coast_mm, mut last_moving) = (0.0, None);
    let stall_end = match fault {
        Fault::SupervisorStall(ms) => timing.fault_at_ms + ms,
        _ => 0,
    };

    for t in 1..=timing.run_until_ms {
        let moved = d.step();
        let faulted = t >= timing.fault_at_ms;
        if faulted {
            coast_mm += moved;
            if d.velocity.abs() >= stop_threshold {
                last_moving = Some(t);
            }
        }
        if t == timing.fault_at_ms {
            d.first_fallback = None;
            if fault == Fault::EstopPressed {
                rt.emergency_stop(t, &mut d);
            }
        }
        if faulted && t < stall_end {
            continue;
        }
        let flags = match fault {
            Fault::ObstacleDetected if faulted => OBSTACLE,
            Fault::EstopPressed if faulted => ESTOP,
            _ => 0,
        };
        let sensors_alive = !(faulted && fault == Fault::SensorFeedStale);
        if sensors_alive && t % timing.sensor_period_ms == 0 {
            rt.update_state(flags, t, &mut d);
        } else if t % timing.supervisor_period_ms == 0 {
            rt.tick(t, &mut d);
        }
        let controller_alive = !(faulted && fault == Fault::ControllerSilent);
        if controller_alive && t % renew_every == 0 {
            renewal += 1;
            if let Ok(l) = rt.renew(OWNER, lease.id, renewal, config.max_lease_ms, t, &mut d) {
                lease = l;
            }
        }
        if controller_alive && t % timing.command_period_ms == 0 {
            if rt.lease(0).is_none() {
                if let Ok(l) = rt.acquire(request, t, &mut d) {
                    lease = l;
                }
            }
            id += 1;
            let action = Action {
                boot_id: rt.boot_id(),
                requester: OWNER,
                lease_id: lease.id,
                action_id: id,
                sequence: id,
                capability: DRIVE,
                based_on_epoch: rt.epoch(),
                execute_after: t,
                deadline: t + timing.command_period_ms * 2,
                valid_for_ms: timing.command_valid_for_ms,
                parameters: [timing.cruise_mm_s, 0],
            };
            rt.submit(action, OWNER, t, t, &mut d);
        }
    }

    let tick = timing.supervisor_period_ms.max(timing.sensor_period_ms);
    let detection_bound_ms = match fault {
        Fault::EstopPressed => 0,
        Fault::ObstacleDetected => timing.sensor_period_ms,
        Fault::ControllerSilent => timing.command_valid_for_ms as u64 + tick,
        Fault::SensorFeedStale => config.state_ttl_ms as u64 + tick,
        Fault::SupervisorStall(ms) => ms + tick,
    };
    let settle = (model.time_constant_ms * 100f64.ln()).ceil() as u64;
    Outcome {
        fault,
        detected_after_ms: d.first_fallback.map(|at| at - timing.fault_at_ms),
        stopped_after_ms: last_moving.map(|at| at + 1 - timing.fault_at_ms),
        coast_mm,
        detection_bound_ms,
        stop_bound_ms: detection_bound_ms + model.command_delay_ms + settle + 1,
    }
}

pub const FAULTS: [Fault; 5] = [
    Fault::ControllerSilent,
    Fault::EstopPressed,
    Fault::ObstacleDetected,
    Fault::SensorFeedStale,
    Fault::SupervisorStall(200),
];
