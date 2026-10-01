//! Kani bounded model-checking harnesses: `cargo kani -p pxr-runtime-core`.
//! Every input and every driver result is symbolic, so each proof covers all values at once.
use crate::{profile::*, *};

#[derive(Default)]
struct AnyDriver {
    executions: u32,
    fallbacks: u32,
    motor: [i32; 2],
}
impl Driver for AnyDriver {
    fn observe(&mut self, _: u8) -> Result<Observation, DriverError> {
        if kani::any() {
            return Err(DriverError);
        }
        Ok(Observation {
            parameters: if kani::any() {
                self.motor
            } else {
                [kani::any(), kani::any()]
            },
            state: kani::any(),
        })
    }
    fn execute(&mut self, _: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        self.executions += 1;
        if kani::any() {
            return Err(DriverError);
        }
        self.motor = parameters;
        Ok(())
    }
    fn fallback(&mut self, resource: u8, _: Reason) -> Result<(), DriverError> {
        self.fallbacks += 1;
        if resource == 0 {
            self.motor = [0; 2];
        }
        if kani::any() {
            Err(DriverError)
        } else {
            Ok(())
        }
    }
}

fn any_action() -> Action {
    Action {
        boot_id: kani::any(),
        requester: kani::any(),
        lease_id: kani::any(),
        action_id: kani::any(),
        sequence: kani::any(),
        capability: kani::any(),
        based_on_epoch: kani::any(),
        execute_after: kani::any(),
        deadline: kani::any(),
        valid_for_ms: kani::any(),
        parameters: [kani::any(), kani::any()],
    }
}

/// Runtime with one live motor lease whose duration and first-parameter limits are symbolic.
fn armed(d: &mut AnyDriver) -> Option<(Runtime, Lease)> {
    let mut rt = Runtime::new(Config::default(), &CAPABILITIES, 42, 0, 0, d).ok()?;
    let lease = rt
        .acquire(
            LeaseRequest {
                owner: 7,
                resource: 0,
                capabilities: (1 << DRIVE) | (1 << STOP),
                duration_ms: kani::any(),
                limits: [
                    Bounds {
                        min: kani::any(),
                        max: kani::any(),
                    },
                    Bounds::ANY,
                ],
            },
            0,
            d,
        )
        .ok()?;
    Some((rt, lease))
}

#[kani::proof]
#[kani::unwind(10)]
fn submit_dispatches_only_admissible_actions() {
    let mut d = AnyDriver::default();
    let Some((mut rt, lease)) = armed(&mut d) else {
        return;
    };
    let t: u64 = kani::any();
    kani::assume(t <= 100);
    rt.update_state(kani::any(), t, &mut d);
    let flags = rt.flags();
    let action = any_action();
    let (principal, received, now): (u64, u64, u64) = (kani::any(), kani::any(), kani::any());
    let before = d.executions;
    let receipt = rt.submit(action, principal, received, now, &mut d);
    if d.executions == before {
        assert!(receipt.decision != Decision::Executed);
        return;
    }
    let cap = CAPABILITIES
        .iter()
        .find(|c| c.id == action.capability)
        .unwrap();
    assert!(cap.resource == 0 && receipt.dispatched);
    assert!(principal == 7 && action.requester == 7);
    assert!(action.boot_id == 42 && action.lease_id == lease.id);
    assert!(cap.permits(flags) && flags & ESTOP == 0);
    assert!(received <= now && now - received < action.valid_for_ms as u64);
    assert!(action.execute_after <= now && now < action.deadline && now < lease.expires_at);
    assert!(lease.limits[0].contains(action.parameters[0]));
    assert!(cap.bounds[0].contains(action.parameters[0]));
    assert!(cap.bounds[1].contains(action.parameters[1]));
}

#[kani::proof]
#[kani::unwind(10)]
fn emergency_stop_blocks_dispatch_until_local_recovery() {
    let mut d = AnyDriver::default();
    let Some((mut rt, _)) = armed(&mut d) else {
        return;
    };
    rt.emergency_stop(kani::any(), &mut d);
    assert!(rt.state() == RuntimeState::Estopped && rt.lease(0).is_none());
    let before = d.executions;
    rt.update_state(kani::any(), kani::any(), &mut d);
    let _ = rt.acquire(
        LeaseRequest {
            owner: kani::any(),
            resource: kani::any(),
            capabilities: kani::any(),
            duration_ms: kani::any(),
            limits: [Bounds::ANY; 2],
        },
        kani::any(),
        &mut d,
    );
    rt.submit(any_action(), kani::any(), kani::any(), kani::any(), &mut d);
    rt.tick(kani::any(), &mut d);
    assert!(d.executions == before);
    assert!(rt.state() == RuntimeState::Estopped);
}

#[kani::proof]
#[kani::unwind(10)]
fn tick_revokes_expired_or_unsupervised_authority() {
    let mut d = AnyDriver::default();
    let Some((mut rt, lease)) = armed(&mut d) else {
        return;
    };
    let fallbacks = d.fallbacks;
    let now: u64 = kani::any();
    rt.tick(now, &mut d);
    match rt.lease(0) {
        Some(l) => {
            assert!(l.expires_at > now && now < lease.expires_at);
            assert!(rt.state() == RuntimeState::Armed);
        }
        None => assert!(d.fallbacks > fallbacks && d.motor == [0; 2]),
    }
    if now >= lease.expires_at || now > Config::default().watchdog_ms as u64 {
        assert!(rt.lease(0).is_none());
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn receipts_never_wrap_or_lose_order() {
    let mut d = AnyDriver::default();
    let Some((mut rt, lease)) = armed(&mut d) else {
        return;
    };
    let mut action = any_action();
    action.lease_id = lease.id;
    let first = rt.submit(action, 7, kani::any(), kani::any(), &mut d);
    let second = rt.submit(any_action(), kani::any(), kani::any(), kani::any(), &mut d);
    assert!(second.receipt_id > first.receipt_id);
    let n = rt.receipt_count();
    assert!(rt.receipt(n - 1).unwrap().receipt_id == rt.snapshot().next_receipt_id - 1);
    assert!(rt.receipt(n).is_none());
}

#[kani::proof]
#[kani::unwind(10)]
fn raised_estop_signal_blocks_the_next_dispatch() {
    let mut d = AnyDriver::default();
    let Some((mut rt, _)) = armed(&mut d) else {
        return;
    };
    let signal = EstopSignal::new();
    if kani::any() {
        assert!(!rt.poll_estop(&signal, kani::any(), &mut d));
    }
    signal.raise();
    let before = d.executions;
    assert!(rt.poll_estop(&signal, kani::any(), &mut d));
    rt.submit(any_action(), kani::any(), kani::any(), kani::any(), &mut d);
    assert!(d.executions == before);
    assert!(rt.state() == RuntimeState::Estopped);
}
