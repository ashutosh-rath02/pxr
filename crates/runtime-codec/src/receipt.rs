use crate::{crc32, DecodeError, VERSION};
use pxr_runtime_core::{Decision, Reason, Receipt};

pub const RECEIPT_FRAME_SIZE: usize = 140;

pub fn encode_receipt(r: &Receipt) -> [u8; RECEIPT_FRAME_SIZE] {
    let mut out = [0u8; RECEIPT_FRAME_SIZE];
    out[..4].copy_from_slice(b"PXRR");
    out[4] = VERSION;
    out[6..8].copy_from_slice(&(RECEIPT_FRAME_SIZE as u16).to_le_bytes());
    out[8..16].copy_from_slice(&r.boot_id.to_le_bytes());
    out[16..24].copy_from_slice(&r.principal.to_le_bytes());
    out[24..28].copy_from_slice(&r.safety_flags.to_le_bytes());
    for (i, n) in [
        r.receipt_id,
        r.action_id,
        r.sequence,
        r.lease_id,
        r.epoch,
        r.received_at,
        r.admitted_at,
        r.executed_at,
        r.original_receipt,
    ]
    .iter()
    .enumerate()
    {
        out[32 + i * 8..40 + i * 8].copy_from_slice(&n.to_le_bytes());
    }
    for (i, n) in [r.requested[0], r.requested[1], r.observed[0], r.observed[1]]
        .iter()
        .enumerate()
    {
        out[104 + i * 4..108 + i * 4].copy_from_slice(&n.to_le_bytes());
    }
    out[120..124].copy_from_slice(&r.pre_state.to_le_bytes());
    out[124..128].copy_from_slice(&r.post_state.to_le_bytes());
    out[128..130].copy_from_slice(&r.capability.to_le_bytes());
    out[130..132].copy_from_slice(&(r.reason as u16).to_le_bytes());
    out[132] = r.decision as u8;
    out[133] = r.resource;
    out[134] = r.dispatched as u8;
    out[135] = r.observed_valid as u8;
    let crc = crc32(&out[..136]);
    out[136..].copy_from_slice(&crc.to_le_bytes());
    out
}

pub fn decode_receipt(f: &[u8]) -> Result<Receipt, DecodeError> {
    if f.len() != RECEIPT_FRAME_SIZE {
        return Err(DecodeError::Length);
    }
    if &f[..4] != b"PXRR" {
        return Err(DecodeError::Magic);
    }
    if f[4] != VERSION {
        return Err(DecodeError::Version);
    }
    if f[5] != 0 || f[28..32] != [0; 4] {
        return Err(DecodeError::Reserved);
    }
    if u16::from_le_bytes([f[6], f[7]]) as usize != RECEIPT_FRAME_SIZE {
        return Err(DecodeError::Length);
    }
    let u64_at = |i| u64::from_le_bytes(f[i..i + 8].try_into().unwrap());
    let u32_at = |i| u32::from_le_bytes(f[i..i + 4].try_into().unwrap());
    if crc32(&f[..136]) != u32_at(136) {
        return Err(DecodeError::Checksum);
    }
    let reason = match u16::from_le_bytes([f[130], f[131]]) {
        0 => Reason::Ok,
        1 => Reason::Bound,
        2 => Reason::Stale,
        3 => Reason::Authority,
        4 => Reason::Duplicate,
        5 => Reason::Precondition,
        6 => Reason::OldSequence,
        7 => Reason::Epoch,
        8 => Reason::UnknownCapability,
        9 => Reason::TooEarly,
        10 => Reason::Invalid,
        11 => Reason::Estop,
        12 => Reason::Faulted,
        13 => Reason::Watchdog,
        14 => Reason::LeaseExpired,
        15 => Reason::StreamExpired,
        16 => Reason::Driver,
        17 => Reason::Verification,
        18 => Reason::Clock,
        19 => Reason::Busy,
        20 => Reason::Canceled,
        21 => Reason::InvalidStorm,
        22 => Reason::StateStale,
        23 => Reason::BootMismatch,
        24 => Reason::LeaseGranted,
        25 => Reason::LeaseRenewed,
        26 => Reason::Recovered,
        27 => Reason::Startup,
        _ => return Err(DecodeError::Value),
    };
    let decision = match f[132] {
        0 => Decision::Executed,
        1 => Decision::Rejected,
        2 => Decision::Fallback,
        3 => Decision::Control,
        4 => Decision::Failed,
        _ => return Err(DecodeError::Value),
    };
    if f[134] > 1 || f[135] > 1 {
        return Err(DecodeError::Value);
    }
    Ok(Receipt {
        boot_id: u64_at(8),
        principal: u64_at(16),
        safety_flags: u32_at(24),
        receipt_id: u64_at(32),
        action_id: u64_at(40),
        sequence: u64_at(48),
        lease_id: u64_at(56),
        epoch: u64_at(64),
        received_at: u64_at(72),
        admitted_at: u64_at(80),
        executed_at: u64_at(88),
        original_receipt: u64_at(96),
        requested: [u32_at(104) as i32, u32_at(108) as i32],
        observed: [u32_at(112) as i32, u32_at(116) as i32],
        pre_state: u32_at(120),
        post_state: u32_at(124),
        capability: u16::from_le_bytes([f[128], f[129]]),
        reason,
        decision,
        resource: f[133],
        dispatched: f[134] != 0,
        observed_valid: f[135] != 0,
    })
}
