use pxr_runtime_codec::*;
use pxr_runtime_core::{Action, Decision, Reason, Receipt};

const REASONS: [Reason; 28] = [
    Reason::Ok,
    Reason::Bound,
    Reason::Stale,
    Reason::Authority,
    Reason::Duplicate,
    Reason::Precondition,
    Reason::OldSequence,
    Reason::Epoch,
    Reason::UnknownCapability,
    Reason::TooEarly,
    Reason::Invalid,
    Reason::Estop,
    Reason::Faulted,
    Reason::Watchdog,
    Reason::LeaseExpired,
    Reason::StreamExpired,
    Reason::Driver,
    Reason::Verification,
    Reason::Clock,
    Reason::Busy,
    Reason::Canceled,
    Reason::InvalidStorm,
    Reason::StateStale,
    Reason::BootMismatch,
    Reason::LeaseGranted,
    Reason::LeaseRenewed,
    Reason::Recovered,
    Reason::Startup,
];
const DECISIONS: [Decision; 5] = [
    Decision::Executed,
    Decision::Rejected,
    Decision::Fallback,
    Decision::Control,
    Decision::Failed,
];

fn receipt(reason: Reason, decision: Decision) -> Receipt {
    Receipt {
        boot_id: 42,
        principal: 7,
        safety_flags: 3,
        receipt_id: 9,
        action_id: 11,
        sequence: 12,
        capability: 1,
        resource: 0,
        lease_id: 5,
        decision,
        reason,
        epoch: 4,
        received_at: 1,
        admitted_at: 2,
        executed_at: 3,
        requested: [-5, 6],
        observed: [7, -8],
        pre_state: 1,
        post_state: 2,
        original_receipt: 0,
        dispatched: true,
        observed_valid: false,
    }
}

fn reseal(frame: &mut [u8]) {
    let n = frame.len() - 4;
    let crc = crc32(&frame[..n]);
    frame[n..].copy_from_slice(&crc.to_le_bytes());
}

#[test]
fn every_reason_and_decision_round_trips_with_its_wire_value() {
    for (wire, reason) in REASONS.iter().enumerate() {
        for (byte, decision) in DECISIONS.iter().enumerate() {
            let r = receipt(*reason, *decision);
            let frame = encode_receipt(&r);
            assert_eq!(u16::from_le_bytes([frame[130], frame[131]]) as usize, wire);
            assert_eq!(frame[132] as usize, byte);
            assert_eq!(decode_receipt(&frame), Ok(r), "{reason:?} {decision:?}");
        }
    }
}

#[test]
fn each_reserved_byte_is_checked_individually() {
    let action = Action {
        boot_id: 1,
        requester: 2,
        lease_id: 3,
        action_id: 4,
        sequence: 5,
        capability: 1,
        based_on_epoch: 6,
        execute_after: 0,
        deadline: 9,
        valid_for_ms: 10,
        parameters: [1, 2],
    };
    for offset in [5, 10, 11] {
        let mut frame = encode(&action);
        frame[offset] = 1;
        reseal(&mut frame);
        assert_eq!(
            decode(&frame),
            Err(DecodeError::Reserved),
            "action byte {offset}"
        );
    }
    for offset in [5, 28, 29, 30, 31] {
        let mut frame = encode_receipt(&receipt(Reason::Ok, Decision::Executed));
        frame[offset] = 1;
        reseal(&mut frame);
        assert_eq!(
            decode_receipt(&frame),
            Err(DecodeError::Reserved),
            "receipt byte {offset}"
        );
    }
}
