use pxr_runtime_core::{profile::*, *};
use pxr_runtime_sim::*;

fn driving() -> (Simulation, Lease, Action) {
    let mut s = Simulation::default();
    let l = s.grant(0, 500).unwrap();
    let a = s.action(l, DRIVE, 1, [400, 200]);
    (s, l, a)
}
#[test]
fn valid_drive_is_observed_and_receipted() {
    let (mut s, _, a) = driving();
    let r = s.submit(a);
    assert_eq!(r.decision, Decision::Executed);
    assert_eq!(r.observed, [400, 200]);
    assert_eq!(s.driver.executions, 1);
    assert_eq!(s.runtime.state(), RuntimeState::Armed);
}
#[test]
fn bound_rejection_never_calls_driver() {
    let (mut s, _, mut a) = driving();
    a.parameters[0] = 1001;
    assert_eq!(s.submit(a).reason, Reason::Bound);
    assert_eq!(s.driver.executions, 0);
}
#[test]
fn narrowed_lease_limits_are_enforced() {
    let mut s = Simulation::default();
    let l = s
        .runtime
        .acquire(
            LeaseRequest {
                owner: 7,
                resource: 0,
                capabilities: 1 << DRIVE,
                duration_ms: 100,
                limits: [
                    Bounds {
                        min: -100,
                        max: 100,
                    },
                    Bounds::ANY,
                ],
            },
            0,
            &mut s.driver,
        )
        .unwrap();
    let a = s.action(l, DRIVE, 1, [101, 0]);
    assert_eq!(s.submit(a).reason, Reason::Bound);
}
#[test]
fn absolute_controller_deadline_catches_delayed_first_packet() {
    let (mut s, _, a) = driving();
    s.advance(20);
    assert_eq!(s.submit(a).reason, Reason::Stale);
    assert_eq!(s.driver.executions, 0);
}
#[test]
fn receive_relative_ttl_catches_queue_delay() {
    let (mut s, _, mut a) = driving();
    a.deadline = 200;
    a.valid_for_ms = 10;
    s.advance(10);
    let r = s.runtime.submit(a, 7, 0, s.now, &mut s.driver);
    assert_eq!(r.reason, Reason::Stale);
}
#[test]
fn execute_after_is_not_a_hidden_queue() {
    let (mut s, _, mut a) = driving();
    a.execute_after = 10;
    assert_eq!(s.submit(a).reason, Reason::TooEarly);
    s.advance(10);
    assert_eq!(s.submit(a).decision, Decision::Executed);
}
#[test]
fn missing_lease_is_rejected() {
    let (mut s, _, mut a) = driving();
    a.lease_id = 999;
    assert_eq!(s.submit(a).reason, Reason::Authority);
}
#[test]
fn principal_cannot_be_forged_by_action_field() {
    let (mut s, _, a) = driving();
    assert_eq!(
        s.runtime.submit(a, 8, 0, 0, &mut s.driver).reason,
        Reason::Authority
    );
}
#[test]
fn expired_lease_is_not_renewable() {
    let mut s = Simulation::default();
    let l = s.grant(0, 20).unwrap();
    s.advance(20);
    assert_eq!(
        s.runtime.renew(7, l.id, 1, 100, s.now, &mut s.driver),
        Err(Reason::Authority)
    );
}
#[test]
fn duplicate_discrete_is_never_executed_twice() {
    let mut s = Simulation::default();
    let l = s.grant(1, 500).unwrap();
    let a = s.action(l, OPEN, 1, [0; 2]);
    let first = s.submit(a);
    let second = s.submit(a);
    assert_eq!(first.decision, Decision::Executed);
    assert_eq!(second.reason, Reason::Duplicate);
    assert_eq!(second.original_receipt, first.receipt_id);
    assert_eq!(s.driver.executions, 1);
}
#[test]
fn changing_payload_does_not_bypass_duplicate_protection() {
    let (mut s, _, mut a) = driving();
    s.submit(a);
    a.parameters = [-400, 0];
    a.sequence = 2;
    assert_eq!(s.submit(a).reason, Reason::Duplicate);
    assert_eq!(s.driver.velocity, [400, 200]);
}
#[test]
fn reordered_sequence_is_rejected() {
    let (mut s, _, mut a) = driving();
    a.sequence = 2;
    s.submit(a);
    a.action_id = 2;
    a.sequence = 1;
    assert_eq!(s.submit(a).reason, Reason::OldSequence);
    assert_eq!(s.driver.executions, 1);
}
#[test]
fn replay_after_cache_eviction_is_still_rejected_by_sequence() {
    let (mut s, _, mut a) = driving();
    let first = a;
    for n in 1..=REPLAY_CAPACITY as u64 + 1 {
        a.action_id = n;
        a.sequence = n;
        assert_eq!(s.submit(a).decision, Decision::Executed);
    }
    assert_eq!(s.submit(first).reason, Reason::OldSequence);
}
#[test]
fn precondition_checks_local_state() {
    let mut s = Simulation::default();
    s.state(ARM_MOVING);
    let l = s.grant(1, 100).unwrap();
    let a = s.action(l, OPEN, 1, [0; 2]);
    assert_eq!(s.submit(a).reason, Reason::Precondition);
}
#[test]
fn changed_epoch_rejects_plan_even_when_predicates_pass() {
    let (mut s, _, a) = driving();
    s.state(ARM_MOVING);
    assert_eq!(s.submit(a).reason, Reason::Epoch);
}
#[test]
fn unsafe_state_stops_already_active_motor_immediately() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.state(OBSTACLE);
    assert_eq!(s.driver.velocity, [0; 2]);
    assert!(s.runtime.lease(0).is_none());
}
#[test]
fn controller_loss_triggers_stream_fallback_within_one_tick() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.advance(99);
    assert_ne!(s.driver.velocity, [0; 2]);
    s.advance(1);
    assert_eq!(s.driver.velocity, [0; 2]);
    assert_eq!(
        s.runtime
            .receipt(s.runtime.receipt_count() - 1)
            .unwrap()
            .reason,
        Reason::StreamExpired
    );
}
#[test]
fn renewal_does_not_extend_old_stream_setpoint() {
    let (mut s, l, a) = driving();
    s.submit(a);
    s.advance(90);
    s.runtime
        .renew(7, l.id, 1, 500, s.now, &mut s.driver)
        .unwrap();
    s.advance(10);
    assert_eq!(s.driver.velocity, [0; 2]);
    assert!(s.runtime.lease(0).is_none());
}
#[test]
fn renewal_replay_cannot_extend_lease() {
    let (mut s, l, _) = driving();
    s.runtime.renew(7, l.id, 1, 500, 0, &mut s.driver).unwrap();
    s.advance(10);
    assert_eq!(
        s.runtime.renew(7, l.id, 1, 500, 10, &mut s.driver),
        Err(Reason::OldSequence)
    );
    assert_eq!(s.runtime.lease(0).unwrap().expires_at, 500);
}
#[test]
fn supervisor_gap_faults_and_stops() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.now = 51;
    s.runtime.tick(s.now, &mut s.driver);
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert_eq!(s.driver.velocity, [0; 2]);
    assert!((0..s.runtime.receipt_count())
        .any(|i| s.runtime.receipt(i).unwrap().reason == Reason::Watchdog));
}
#[test]
fn stale_sensor_feed_cannot_be_masked_by_lease_heartbeats() {
    let (mut s, _, _) = driving();
    for t in (10..=250).step_by(10) {
        s.runtime.tick(t, &mut s.driver);
    }
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert!((0..s.runtime.receipt_count())
        .any(|i| s.runtime.receipt(i).unwrap().reason == Reason::StateStale));
}
#[test]
fn invalid_storm_revokes_authority() {
    let mut s = Simulation::new(Config {
        invalid_limit: 3,
        ..Config::default()
    });
    let l = s.grant(0, 500).unwrap();
    let mut a = s.action(l, DRIVE, 1, [400, 0]);
    s.submit(a);
    a.action_id = 2;
    a.sequence = 2;
    a.parameters = [i32::MAX, 0];
    for _ in 0..3 {
        s.submit(a);
    }
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert_eq!(s.driver.velocity, [0; 2]);
}
#[test]
fn estop_is_latched_until_explicit_local_recovery() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.runtime.emergency_stop(0, &mut s.driver);
    assert_eq!(s.submit(a).reason, Reason::Estop);
    assert_eq!(
        s.runtime.recover_local(0, &mut s.driver),
        Err(Reason::Estop)
    );
    s.state(0);
    assert_eq!(s.runtime.state(), RuntimeState::Estopped);
    s.runtime.recover_local(0, &mut s.driver).unwrap();
    assert_eq!(s.runtime.state(), RuntimeState::SafeIdle);
    assert!(s.runtime.lease(0).is_none());
}
#[test]
fn clock_rollback_faults() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.advance(10);
    s.runtime.tick(9, &mut s.driver);
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert_eq!(s.driver.velocity, [0; 2]);
}
#[test]
fn boot_nonce_rejects_packets_from_previous_runtime() {
    let (mut s, _, mut a) = driving();
    a.boot_id = 41;
    assert_eq!(s.submit(a).reason, Reason::BootMismatch);
}
#[test]
fn owner_cannot_take_over_live_resource() {
    let (mut s, _, _) = driving();
    assert_eq!(s.grant(0, 500), Err(Reason::Busy));
}
#[test]
fn capability_scope_and_resource_are_enforced() {
    let (mut s, _, mut a) = driving();
    a.capability = OPEN;
    assert_eq!(s.submit(a).reason, Reason::Authority);
}
#[test]
fn driver_failure_revokes_all_authority() {
    let (mut s, _, a) = driving();
    s.driver.fail_execute = true;
    assert_eq!(s.submit(a).reason, Reason::Driver);
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert_eq!(s.driver.velocity, [0; 2]);
    assert!(s.runtime.lease(0).is_none());
}
#[test]
fn verification_failure_is_not_reported_executed() {
    let (mut s, _, a) = driving();
    s.driver.wrong_feedback = true;
    let r = s.submit(a);
    assert_eq!(r.reason, Reason::Verification);
    assert_eq!(r.decision, Decision::Failed);
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert_eq!(s.driver.velocity, [0; 2]);
}
#[test]
fn fallback_failure_is_latched() {
    let (mut s, l, a) = driving();
    s.submit(a);
    s.driver.fail_fallback = true;
    assert_eq!(
        s.runtime.cancel(7, l.id, 0, &mut s.driver),
        Err(Reason::Driver)
    );
    assert_eq!(s.runtime.state(), RuntimeState::Faulted);
    assert_eq!(
        s.runtime.recover_local(0, &mut s.driver),
        Err(Reason::Driver)
    );
}
#[test]
fn stop_and_cancel_work() {
    let (mut s, l, a) = driving();
    s.submit(a);
    let stop = s.action(l, STOP, 2, [0; 2]);
    assert_eq!(s.submit(stop).decision, Decision::Executed);
    assert_eq!(s.driver.velocity, [0; 2]);
    s.runtime.cancel(7, l.id, 0, &mut s.driver).unwrap();
    assert_eq!(s.runtime.state(), RuntimeState::SafeIdle);
}
#[test]
fn gripper_fallback_holds_payload() {
    let mut s = Simulation::default();
    let l = s.grant(1, 20).unwrap();
    let a = s.action(l, OPEN, 1, [0; 2]);
    s.submit(a);
    s.advance(20);
    assert!(s.driver.gripper_open);
    assert!(s.runtime.lease(1).is_none());
}
#[test]
fn receipt_ring_is_bounded_and_ordered() {
    let (mut s, _, mut a) = driving();
    for n in 1..100 {
        a.action_id = n;
        a.sequence = n;
        s.submit(a);
    }
    assert_eq!(s.runtime.receipt_count(), RECEIPT_CAPACITY);
    for i in 1..RECEIPT_CAPACITY {
        assert_eq!(
            s.runtime.receipt(i).unwrap().receipt_id,
            s.runtime.receipt(i - 1).unwrap().receipt_id + 1
        );
    }
    assert!(s.runtime.receipt(RECEIPT_CAPACITY).is_none());
}
#[test]
fn configuration_is_validated_before_driver_calls() {
    let mut d = SimDriver::default();
    let mut caps = CAPABILITIES;
    caps[1].id = DRIVE;
    assert!(matches!(
        Runtime::new(Config::default(), &caps, 42, 0, 0, &mut d),
        Err(ConfigError::DuplicateCapability)
    ));
    assert_eq!(d.fallbacks, 0);
}
#[test]
fn malformed_time_does_not_overflow_or_dispatch() {
    let (mut s, _, mut a) = driving();
    a.execute_after = u64::MAX;
    assert_eq!(s.submit(a).reason, Reason::Invalid);
    assert_eq!(
        s.runtime.submit(a, 7, u64::MAX, 0, &mut s.driver).reason,
        Reason::Invalid
    );
}
#[test]
fn deterministic_trace_repeats_byte_for_byte() {
    fn trace() -> Vec<Receipt> {
        let (mut s, _, a) = driving();
        s.submit(a);
        s.submit(a);
        s.advance(100);
        (0..s.runtime.receipt_count())
            .map(|i| s.runtime.receipt(i).unwrap())
            .collect()
    }
    let baseline = trace();
    for _ in 0..100 {
        assert_eq!(trace(), baseline);
    }
}
#[test]
fn fixed_runtime_storage_fits_working_memory_target() {
    assert!(std::mem::size_of::<Runtime>() < 32768);
}

#[test]
fn estop_fences_old_sensor_updates_and_recovery() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.runtime.emergency_stop(100, &mut s.driver);
    assert_eq!(s.runtime.snapshot().now, 100);
    s.runtime.update_state(0, 99, &mut s.driver);
    assert_ne!(s.runtime.flags() & ESTOP, 0);
    assert!(s.runtime.recover_local(99, &mut s.driver).is_err());
    s.runtime.update_state(0, 100, &mut s.driver);
    s.runtime.recover_local(100, &mut s.driver).unwrap();
}

#[test]
fn regressed_estop_still_stops_and_preserves_clock_fence() {
    let (mut s, _, a) = driving();
    s.submit(a);
    s.advance(10);
    s.runtime.emergency_stop(5, &mut s.driver);
    assert_eq!(s.driver.velocity, [0; 2]);
    assert_eq!(s.runtime.snapshot().now, 10);
    assert_eq!(s.runtime.state(), RuntimeState::Estopped);
}

#[test]
fn exported_receipts_preserve_authority_and_corruption_is_detected() {
    use pxr_runtime_codec::{decode_receipt, encode_receipt, RECEIPT_FRAME_SIZE};
    let (mut s, _, a) = driving();
    let receipt = s.submit(a);
    assert_eq!(receipt.boot_id, 42);
    assert_eq!(receipt.principal, 7);
    let frame = encode_receipt(&receipt);
    assert_eq!(decode_receipt(&frame), Ok(receipt));
    for bit in 0..RECEIPT_FRAME_SIZE * 8 {
        let mut bad = frame;
        bad[bit / 8] ^= 1 << (bit % 8);
        assert!(decode_receipt(&bad).is_err());
    }
    s.advance(100);
    let fallback = s.runtime.receipt(s.runtime.receipt_count() - 1).unwrap();
    assert_eq!(fallback.principal, 7);
    assert_eq!(fallback.lease_id, 1);
}

#[test]
fn receipt_decoder_rejects_unknown_enums_even_with_valid_crc() {
    use pxr_runtime_codec::*;
    let (mut s, _, a) = driving();
    let frame = encode_receipt(&s.submit(a));
    for offset in [130, 132, 134, 135] {
        let mut bad = frame;
        bad[offset] = 255;
        let crc = crc32(&bad[..136]);
        bad[136..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(decode_receipt(&bad), Err(DecodeError::Value));
    }
    for n in 0..140 {
        assert_eq!(decode_receipt(&frame[..n]), Err(DecodeError::Length));
    }
}

#[test]
fn randomized_fault_sequences_preserve_motion_invariants() {
    let mut s = Simulation::default();
    let mut seed = 0x5eedu32;
    let mut id = 0u64;
    for _ in 0..20000 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        match seed % 8 {
            0 => s.advance((seed % 20) as u64),
            1 => {
                s.state(if seed & 256 == 0 { OBSTACLE } else { 0 });
            }
            2 => {
                s.runtime.emergency_stop(s.now, &mut s.driver);
            }
            3 => {
                s.state(0);
                let _ = s.runtime.recover_local(s.now, &mut s.driver);
            }
            4 => {
                let _ = s.grant(0, 100);
            }
            _ => {
                if let Some(l) = s.runtime.lease(0) {
                    id += 1;
                    let mut a = s.action(l, DRIVE, id, [(seed % 2401) as i32 - 1200, 0]);
                    if seed & 64 != 0 {
                        a.sequence = 1;
                    }
                    s.submit(a);
                }
            }
        }
        assert!(s.driver.velocity[0].abs() <= 1000);
        if s.driver.velocity != [0; 2] {
            assert_eq!(s.runtime.state(), RuntimeState::Armed);
            assert!(s.runtime.lease(0).is_some_and(|l| l.expires_at > s.now));
            assert_eq!(
                s.runtime.flags() & (ESTOP | OBSTACLE | BATTERY_CRITICAL | MOTOR_FAULT),
                0
            );
        }
    }
}
