//! Browser-only adapter. All admission decisions come from runtime-core.
//! Virtual time advances only on request. This wrapper uses std/heap storage;
//! the embedded core retains its no_std, fixed-capacity implementation.
use pxr_runtime_core::{profile::*, *};
use pxr_runtime_sim::{SimDriver, Simulation};
use std::{cell::RefCell, fmt::Write};

struct Demo {
    sim: Simulation,
    previous: Option<(Action, u64)>,
    next_action: u64,
    result: Reason,
    json: String,
}

impl Demo {
    fn new() -> Self {
        let mut driver = SimDriver::default();
        let runtime =
            Runtime::new(Config::default(), &CAPABILITIES[..2], 42, 0, 0, &mut driver).unwrap();
        let mut sim = Simulation {
            runtime,
            driver,
            now: 0,
        };
        sim.grant(0, 2000).unwrap();
        Self {
            sim,
            previous: None,
            next_action: 1,
            result: Reason::Ok,
            json: String::new(),
        }
    }

    fn advance(&mut self, ms: u32, mode: u32) -> Reason {
        if ms > 5000 || mode > 2 {
            return Reason::Invalid;
        }
        match mode {
            0 => self.sim.advance(ms.into()),
            1 => {
                let end = self.sim.now + u64::from(ms);
                while self.sim.now < end {
                    self.sim.now = (self.sim.now + 10).min(end);
                    self.sim.runtime.tick(self.sim.now, &mut self.sim.driver);
                }
            }
            _ => {
                self.sim.now += u64::from(ms);
                self.sim.runtime.tick(self.sim.now, &mut self.sim.driver);
            }
        }
        Reason::Ok
    }

    fn send(&mut self, velocity: i32, turn: i32, ttl: u32, delay: u32) -> Reason {
        if delay > 5000 || ttl > 1000 {
            return Reason::Invalid;
        }
        let received = self.sim.now;
        let action = Action {
            boot_id: self.sim.runtime.boot_id(),
            requester: 7,
            lease_id: self.sim.runtime.lease(0).map_or(0, |l| l.id),
            action_id: self.next_action,
            sequence: self.next_action,
            capability: DRIVE,
            based_on_epoch: self.sim.runtime.epoch(),
            execute_after: received,
            deadline: received + u64::from(ttl),
            valid_for_ms: ttl,
            parameters: [velocity, turn],
        };
        self.next_action += 1;
        self.previous = Some((action, received));
        self.advance(delay, 0);
        self.sim
            .runtime
            .submit(action, 7, received, self.sim.now, &mut self.sim.driver)
            .reason
    }

    fn snapshot(&mut self) {
        let snapshot = self.sim.runtime.snapshot();
        self.json.clear();
        write!(self.json,
            "{{\"version\":\"{}\",\"virtual_time\":true,\"hardware_validated\":false,\"now\":{},\"state\":\"{:?}\",\"epoch\":{},\"flags\":{},\"lease_expires\":{},\"sensor_age\":{},\"velocity\":{},\"turn\":{},\"executions\":{},\"fallbacks\":{},\"operation_reason\":\"{:?}\",\"next_receipt_id\":{},\"receipts\":[",
            env!("CARGO_PKG_VERSION"), self.sim.now, snapshot.state, snapshot.epoch, snapshot.flags,
            self.sim.runtime.lease(0).map_or(0, |l| l.expires_at), self.sim.now - snapshot.sensor_tick,
            self.sim.driver.velocity[0], self.sim.driver.velocity[1], self.sim.driver.executions,
            self.sim.driver.fallbacks, self.result, snapshot.next_receipt_id).unwrap();
        for i in 0..self.sim.runtime.receipt_count() {
            let r = self.sim.runtime.receipt(i).unwrap();
            if i > 0 {
                self.json.push(',');
            }
            write!(self.json,
                "{{\"id\":{},\"action_id\":{},\"sequence\":{},\"lease_id\":{},\"principal\":{},\"boot_id\":{},\"epoch\":{},\"time\":{},\"received_at\":{},\"executed_at\":{},\"decision\":\"{:?}\",\"reason\":\"{:?}\",\"requested\":{:?},\"observed\":{:?},\"dispatched\":{},\"observed_valid\":{},\"original_receipt\":{},\"capability\":{},\"resource\":{},\"safety_flags\":{},\"pre_state\":{},\"post_state\":{}}}",
                r.receipt_id, r.action_id, r.sequence, r.lease_id, r.principal, r.boot_id, r.epoch,
                r.admitted_at, r.received_at, r.executed_at, r.decision, r.reason, r.requested,
                r.observed, r.dispatched, r.observed_valid, r.original_receipt, r.capability,
                r.resource, r.safety_flags, r.pre_state, r.post_state).unwrap();
        }
        self.json.push_str("]}");
    }
}

thread_local! { static DEMO: RefCell<Demo> = RefCell::new(Demo::new()); }

fn change(f: impl FnOnce(&mut Demo) -> Reason) -> u32 {
    DEMO.with_borrow_mut(|demo| {
        demo.result = f(demo);
        demo.result as u32
    })
}

#[no_mangle]
pub extern "C" fn pxr_demo_reset() {
    DEMO.with_borrow_mut(|d| *d = Demo::new());
}

#[no_mangle]
pub extern "C" fn pxr_demo_send(velocity: i32, turn: i32, ttl: u32, delay: u32) -> u32 {
    change(|d| d.send(velocity, turn, ttl, delay))
}

#[no_mangle]
pub extern "C" fn pxr_demo_repeat() -> u32 {
    change(|d| match d.previous {
        Some((action, received)) => {
            d.sim
                .runtime
                .submit(action, 7, received, d.sim.now, &mut d.sim.driver)
                .reason
        }
        None => Reason::Invalid,
    })
}

/// mode: 0 healthy supervision/sensors, 1 missing sensors, 2 supervisor gap.
#[no_mangle]
pub extern "C" fn pxr_demo_advance(ms: u32, mode: u32) -> u32 {
    change(|d| d.advance(ms, mode))
}

#[no_mangle]
pub extern "C" fn pxr_demo_grant() -> u32 {
    change(|d| d.sim.grant(0, 2000).map_or_else(|r| r, |_| Reason::Ok))
}

#[no_mangle]
pub extern "C" fn pxr_demo_estop() {
    change(|d| {
        d.sim.runtime.emergency_stop(d.sim.now, &mut d.sim.driver);
        Reason::Ok
    });
}

/// Explicit simulated local sensor input, never exposed as a remote control API.
#[no_mangle]
pub extern "C" fn pxr_demo_flags(flags: u32) -> u32 {
    change(|d| {
        if flags & !(ESTOP | OBSTACLE) != 0 {
            return Reason::Invalid;
        }
        d.sim.state(flags);
        Reason::Ok
    })
}

#[no_mangle]
pub extern "C" fn pxr_demo_recover() -> u32 {
    change(|d| {
        d.sim
            .runtime
            .recover_local(d.sim.now, &mut d.sim.driver)
            .map_or_else(|r| r, |_| Reason::Ok)
    })
}

#[no_mangle]
pub extern "C" fn pxr_demo_feedback(wrong: u32) {
    change(|d| {
        d.sim.driver.wrong_feedback = wrong != 0;
        Reason::Ok
    });
}

/// Refresh the snapshot. Its pointer remains valid until the next snapshot/reset.
/// The caller must decode/copy immediately; never retain a view across WASM calls.
#[no_mangle]
pub extern "C" fn pxr_demo_snapshot() -> *const u8 {
    DEMO.with_borrow_mut(|d| {
        d.snapshot();
        d.json.as_ptr()
    })
}

#[no_mangle]
pub extern "C" fn pxr_demo_snapshot_len() -> usize {
    DEMO.with_borrow(|d| d.json.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_input_never_reaches_driver() {
        let mut d = Demo::new();
        assert_eq!(d.send(1001, 0, 100, 0), Reason::Bound);
        assert_eq!(d.send(400, 0, 100, 100), Reason::Stale);
        assert_eq!(d.sim.driver.executions, 0);
    }

    #[test]
    fn delay_services_local_supervision() {
        let mut d = Demo::new();
        assert_eq!(d.send(400, 0, 100, 0), Reason::Ok);
        assert_eq!(d.send(500, 0, 100, 200), Reason::Authority);
        assert_eq!(d.sim.driver.velocity, [0, 0]);
        assert_eq!(d.sim.driver.executions, 1);
        assert_eq!(
            d.sim.runtime.receipt(2).unwrap().reason,
            Reason::StreamExpired
        );
    }

    #[test]
    fn missing_sensors_and_supervisor_have_distinct_causes() {
        for (mode, duration, reason) in [(1, 250, Reason::StateStale), (2, 51, Reason::Watchdog)] {
            let mut d = Demo::new();
            d.send(400, 0, 1000, 0);
            d.advance(duration, mode);
            assert_eq!(d.sim.runtime.state(), RuntimeState::Faulted);
            assert_eq!(d.sim.driver.velocity, [0, 0]);
            assert_eq!(d.sim.runtime.receipt(2).unwrap().reason, reason);
        }
    }

    #[test]
    fn adapter_bounds_work_without_advancing_time() {
        let mut d = Demo::new();
        assert_eq!(d.advance(u32::MAX, 0), Reason::Invalid);
        assert_eq!(d.advance(20, 3), Reason::Invalid);
        assert_eq!(d.send(0, 0, 100, u32::MAX), Reason::Invalid);
        assert_eq!(d.send(0, 0, 1001, 0), Reason::Invalid);
        assert_eq!(d.sim.now, 0);
        assert_eq!(d.sim.driver.executions, 0);
    }
}
