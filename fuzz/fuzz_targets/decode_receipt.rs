#![no_main]
use libfuzzer_sys::fuzz_target;
use pxr_runtime_codec::{crc32, decode_receipt, encode_receipt, RECEIPT_FRAME_SIZE};

fuzz_target!(|data: &[u8]| {
    if let Ok(receipt) = decode_receipt(data) {
        assert_eq!(
            &encode_receipt(&receipt)[..],
            data,
            "non-canonical receipt accepted"
        );
    }
    if data.len() == RECEIPT_FRAME_SIZE {
        let mut frame = [0u8; RECEIPT_FRAME_SIZE];
        frame.copy_from_slice(data);
        let crc = crc32(&frame[..136]);
        frame[136..].copy_from_slice(&crc.to_le_bytes());
        if let Ok(receipt) = decode_receipt(&frame) {
            assert_eq!(encode_receipt(&receipt), frame);
        }
    }
});
