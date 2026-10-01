//! Version 1 fixed-size wire codec. CRC detects accidental corruption, not malicious tampering.
#![no_std]
#![forbid(unsafe_code)]
use pxr_runtime_core::Action;
mod receipt;
pub use receipt::{decode_receipt, encode_receipt, RECEIPT_FRAME_SIZE};

pub const FRAME_SIZE: usize = 92;
pub const VERSION: u8 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Length,
    Magic,
    Version,
    Reserved,
    Checksum,
    Value,
}

// 4-bit table: 64 bytes of flash instead of 1 KiB, roughly 4x faster than bitwise on MCUs.
const NIBBLE: [u32; 16] = {
    let mut table = [0u32; 16];
    let mut n = 0;
    while n < 16 {
        let mut crc = n as u32;
        let mut bit = 0;
        while bit < 4 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
            bit += 1;
        }
        table[n] = crc;
        n += 1;
    }
    table
};

/// CRC-32/ISO-HDLC, polynomial 0xedb88320, initial/final XOR 0xffffffff.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= *byte as u32;
        crc = (crc >> 4) ^ NIBBLE[(crc & 15) as usize];
        crc = (crc >> 4) ^ NIBBLE[(crc & 15) as usize];
    }
    !crc
}

pub fn encode(action: &Action) -> [u8; FRAME_SIZE] {
    let mut out = [0; FRAME_SIZE];
    out[..4].copy_from_slice(b"PXR0");
    out[4] = VERSION;
    out[6..8].copy_from_slice(&(FRAME_SIZE as u16).to_le_bytes());
    out[8..10].copy_from_slice(&action.capability.to_le_bytes());
    let fields = [
        action.boot_id,
        action.requester,
        action.lease_id,
        action.action_id,
        action.sequence,
        action.based_on_epoch,
        action.execute_after,
        action.deadline,
    ];
    for (n, value) in fields.iter().enumerate() {
        out[12 + n * 8..20 + n * 8].copy_from_slice(&value.to_le_bytes());
    }
    out[76..80].copy_from_slice(&action.valid_for_ms.to_le_bytes());
    out[80..84].copy_from_slice(&action.parameters[0].to_le_bytes());
    out[84..88].copy_from_slice(&action.parameters[1].to_le_bytes());
    let crc = crc32(&out[..88]);
    out[88..].copy_from_slice(&crc.to_le_bytes());
    out
}

pub fn decode(frame: &[u8]) -> Result<Action, DecodeError> {
    if frame.len() != FRAME_SIZE {
        return Err(DecodeError::Length);
    }
    if &frame[..4] != b"PXR0" {
        return Err(DecodeError::Magic);
    }
    if frame[4] != VERSION {
        return Err(DecodeError::Version);
    }
    if frame[5] != 0 || frame[10] != 0 || frame[11] != 0 {
        return Err(DecodeError::Reserved);
    }
    if u16::from_le_bytes([frame[6], frame[7]]) as usize != FRAME_SIZE {
        return Err(DecodeError::Length);
    }
    if crc32(&frame[..88]) != u32::from_le_bytes(frame[88..92].try_into().unwrap()) {
        return Err(DecodeError::Checksum);
    }
    let u64_at = |n| u64::from_le_bytes(frame[n..n + 8].try_into().unwrap());
    Ok(Action {
        capability: u16::from_le_bytes([frame[8], frame[9]]),
        boot_id: u64_at(12),
        requester: u64_at(20),
        lease_id: u64_at(28),
        action_id: u64_at(36),
        sequence: u64_at(44),
        based_on_epoch: u64_at(52),
        execute_after: u64_at(60),
        deadline: u64_at(68),
        valid_for_ms: u32::from_le_bytes(frame[76..80].try_into().unwrap()),
        parameters: [
            i32::from_le_bytes(frame[80..84].try_into().unwrap()),
            i32::from_le_bytes(frame[84..88].try_into().unwrap()),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Action {
        Action {
            boot_id: 42,
            requester: 7,
            lease_id: 1,
            action_id: 99,
            sequence: 1,
            capability: 1,
            based_on_epoch: 2,
            execute_after: 0,
            deadline: 20,
            valid_for_ms: 100,
            parameters: [400, -200],
        }
    }
    #[test]
    fn known_crc() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }
    #[test]
    fn round_trip() {
        assert_eq!(decode(&encode(&sample())), Ok(sample()));
    }
    #[test]
    fn every_single_bit_corruption_is_rejected() {
        let frame = encode(&sample());
        for i in 0..FRAME_SIZE * 8 {
            let mut corrupt = frame;
            corrupt[i / 8] ^= 1 << (i % 8);
            assert!(decode(&corrupt).is_err());
        }
    }
    #[test]
    fn arbitrary_lengths_and_bytes_never_panic() {
        let mut bytes = [0u8; 256];
        let mut seed = 1u32;
        for _ in 0..1000 {
            for b in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *b = seed as u8;
            }
            for len in 0..bytes.len() {
                assert!(decode(&bytes[..len]).is_err());
            }
        }
    }
    #[test]
    fn reserved_fields_are_not_silently_accepted() {
        let mut frame = encode(&sample());
        frame[5] = 1;
        let crc = crc32(&frame[..88]);
        frame[88..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(decode(&frame), Err(DecodeError::Reserved));
    }
}
