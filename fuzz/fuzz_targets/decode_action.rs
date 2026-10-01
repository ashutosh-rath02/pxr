#![no_main]
use libfuzzer_sys::fuzz_target;
use pxr_runtime_codec::{crc32, decode, encode, FRAME_SIZE};

fuzz_target!(|data: &[u8]| {
    if let Ok(action) = decode(data) {
        assert_eq!(
            &encode(&action)[..],
            data,
            "decoder accepted a non-canonical frame"
        );
    }
    // Repair the checksum so mutations reach field validation instead of stopping at CRC.
    if data.len() == FRAME_SIZE {
        let mut frame = [0u8; FRAME_SIZE];
        frame.copy_from_slice(data);
        let crc = crc32(&frame[..88]);
        frame[88..].copy_from_slice(&crc.to_le_bytes());
        if let Ok(action) = decode(&frame) {
            assert_eq!(encode(&action), frame);
        }
    }
});
