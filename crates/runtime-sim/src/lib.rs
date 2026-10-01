//! Simulated observations are evidence about this model, not about physical hardware.
use pxr_runtime_core::{profile::*, *};

pub mod plant;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimDriver {
    pub velocity: [i32; 2],
    pub gripper_open: bool,
    pub executions: u64,
    pub fallbacks: u64,
    pub fail_execute: bool,
    pub fail_observe: bool,
    pub fail_fallback: bool,
    pub wrong_feedback: bool,
}
impl Driver for SimDriver {
    fn observe(&mut self, resource: u8) -> Result<Observation, DriverError> {
        if self.fail_observe {
            return Err(DriverError);
        }
        match resource {
            0 => Ok(Observation {
                parameters: if self.wrong_feedback {
                    [i32::MAX; 2]
                } else {
                    self.velocity
                },
                state: 0,
            }),
            1 => Ok(Observation {
                parameters: [0; 2],
                state: if self.gripper_open { GRIPPER_OPEN } else { 0 },
            }),
            _ => Err(DriverError),
        }
    }
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        self.executions += 1;
        if self.fail_execute {
            return Err(DriverError);
        }
        match capability {
            DRIVE => self.velocity = parameters,
            STOP => self.velocity = [0; 2],
            OPEN => self.gripper_open = true,
            CLOSE => self.gripper_open = false,
            _ => return Err(DriverError),
        }
        Ok(())
    }
    fn fallback(&mut self, resource: u8, _reason: Reason) -> Result<(), DriverError> {
        self.fallbacks += 1;
        // Reference policy: motor zero; gripper holds its position (no dropped payload).
        if resource == 0 {
            self.velocity = [0; 2];
        }
        if self.fail_fallback {
            Err(DriverError)
        } else {
            Ok(())
        }
    }
}

pub struct Simulation {
    pub runtime: Runtime,
    pub driver: SimDriver,
    pub now: u64,
}
impl Default for Simulation {
    fn default() -> Self {
        Self::new(Config::default())
    }
}
impl Simulation {
    pub fn new(config: Config) -> Self {
        let mut driver = SimDriver::default();
        let runtime = Runtime::new(config, &CAPABILITIES, 42, 0, 0, &mut driver).unwrap();
        Self {
            runtime,
            driver,
            now: 0,
        }
    }
    pub fn grant(&mut self, resource: u8, duration_ms: u32) -> Result<Lease, Reason> {
        self.runtime.acquire(
            LeaseRequest {
                owner: 7,
                resource,
                capabilities: if resource == 0 {
                    (1 << DRIVE) | (1 << STOP)
                } else {
                    (1 << OPEN) | (1 << CLOSE)
                },
                duration_ms,
                limits: [Bounds::ANY; 2],
            },
            self.now,
            &mut self.driver,
        )
    }
    pub fn action(&self, lease: Lease, capability: u16, id: u64, parameters: [i32; 2]) -> Action {
        Action {
            boot_id: self.runtime.boot_id(),
            requester: lease.owner,
            lease_id: lease.id,
            action_id: id,
            sequence: id,
            capability,
            based_on_epoch: self.runtime.epoch(),
            execute_after: self.now,
            deadline: self.now + 20,
            valid_for_ms: 100,
            parameters,
        }
    }
    pub fn submit(&mut self, action: Action) -> Receipt {
        self.runtime
            .submit(action, 7, self.now, self.now, &mut self.driver)
    }
    /// Advance virtual time while the local supervisor and sensor source remain alive.
    pub fn advance(&mut self, ms: u64) {
        let end = self.now.checked_add(ms).unwrap();
        while self.now < end {
            self.now = (self.now + 10).min(end);
            self.runtime
                .update_state(self.runtime.flags(), self.now, &mut self.driver);
        }
    }
    pub fn state(&mut self, flags: u32) {
        self.runtime.update_state(flags, self.now, &mut self.driver);
    }
}
