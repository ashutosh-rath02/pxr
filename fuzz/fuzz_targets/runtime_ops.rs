#![no_main]
//! Byte-driven operation sequences against the reference profile, checking safety invariants
//! after every step. Frames go through the real codec so decoding and admission are both covered.
use libfuzzer_sys::fuzz_target;
use pxr_runtime_codec::{crc32, decode, encode};
use pxr_runtime_core::{profile::*, *};
use pxr_runtime_sim::Simulation;

struct Bytes<'a>(&'a [u8]);
impl Bytes<'_> {
    fn u8(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes([
            self.u8()?,
            self.u8()?,
            self.u8()?,
            self.u8()?,
        ]))
    }
}

fuzz_target!(|data: &[u8]| {
    let mut input = Bytes(data);
    let mut s = Simulation::default();
    let mut executed: Vec<(u64, u64)> = Vec::new();
    let mut next_id = 0u64;
    while let Some(op) = input.u8() {
        match op % 10 {
            0 | 1 => {
                let Some(step) = input.u8() else { break };
                s.now += u64::from(step % 64);
                s.runtime.tick(s.now, &mut s.driver);
            }
            2 => {
                let Some(flags) = input.u8() else { break };
                s.state(u32::from(flags) & 0x1f);
            }
            3 => {
                let Some(r) = input.u8() else { break };
                let _ = s.grant(r % 3, 50 + u32::from(r) * 8);
            }
            4 => s.runtime.emergency_stop(s.now, &mut s.driver),
            5 => {
                s.state(0);
                let _ = s.runtime.recover_local(s.now, &mut s.driver);
            }
            _ => {
                let (Some(sel), Some(p0), Some(p1), Some(tweak)) =
                    (input.u8(), input.i32(), input.i32(), input.u8())
                else {
                    break;
                };
                let cap = [DRIVE, STOP, OPEN, CLOSE, 0, 9][usize::from(sel % 6)];
                let resource = if cap == DRIVE || cap == STOP { 0 } else { 1 };
                let Some(lease) = s.runtime.lease(resource) else {
                    continue;
                };
                next_id += 1;
                let mut action = s.action(lease, cap, next_id, [p0, p1]);
                action.sequence = lease.last_sequence + 1;
                if tweak & 1 != 0 {
                    action.action_id = u64::from(sel);
                }
                if tweak & 2 != 0 {
                    action.sequence = u64::from(tweak >> 2);
                }
                if tweak & 4 != 0 {
                    action.based_on_epoch ^= 1;
                }
                let mut frame = encode(&action);
                // Arbitrary byte overwrite with checksum repair: decoding must stay sound.
                if tweak & 8 != 0 {
                    frame[usize::from(sel) % 88] = tweak;
                    let crc = crc32(&frame[..88]);
                    frame[88..].copy_from_slice(&crc.to_le_bytes());
                }
                let Ok(action) = decode(&frame) else { continue };
                let before = s.driver.executions;
                let receipt = s.submit(action);
                if s.driver.executions > before {
                    let key = (action.lease_id, action.action_id);
                    assert!(!executed.contains(&key), "frame dispatched twice");
                    executed.push(key);
                    assert!(receipt.dispatched);
                    assert!(CAPABILITIES
                        .iter()
                        .find(|c| c.id == action.capability)
                        .is_some_and(|c| c
                            .bounds
                            .iter()
                            .zip(action.parameters)
                            .all(|(b, p)| b.contains(p))));
                } else {
                    assert_ne!(receipt.decision, Decision::Executed);
                }
            }
        }
        assert!(s.driver.velocity[0].abs() <= 1000 && s.driver.velocity[1].abs() <= 2000);
        if s.driver.velocity != [0; 2] {
            assert_eq!(s.runtime.state(), RuntimeState::Armed);
            assert!(s.runtime.lease(0).is_some_and(|l| l.expires_at > s.now));
            assert_eq!(
                s.runtime.flags() & (ESTOP | OBSTACLE | BATTERY_CRITICAL | MOTOR_FAULT),
                0
            );
        }
        if s.runtime.flags() & ESTOP != 0 {
            assert_eq!(s.runtime.state(), RuntimeState::Estopped);
        }
    }
});
