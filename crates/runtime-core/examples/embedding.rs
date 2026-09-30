//! Run with: cargo run -p pxr-runtime-core --example embedding
use pxr_runtime_core::{profile::*, *};

#[derive(Default)]
struct Motor {
    velocity: [i32; 2],
}
impl Driver for Motor {
    fn observe(&mut self, _: u8) -> Result<Observation, DriverError> {
        Ok(Observation {
            parameters: self.velocity,
            state: 0,
        })
    }
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        match capability {
            DRIVE | STOP => {
                self.velocity = parameters;
                Ok(())
            }
            _ => Err(DriverError),
        }
    }
    fn fallback(&mut self, _: u8, _: Reason) -> Result<(), DriverError> {
        self.velocity = [0; 2];
        Ok(())
    }
}
fn main() {
    let mut motor = Motor::default();
    let mut runtime =
        Runtime::new(Config::default(), &CAPABILITIES[..2], 42, 0, 0, &mut motor).unwrap();
    // Principal 7 is already authenticated/authorized by the embedding application.
    let lease = runtime
        .acquire(
            LeaseRequest {
                owner: 7,
                resource: 0,
                capabilities: (1 << DRIVE) | (1 << STOP),
                duration_ms: 500,
                limits: [
                    Bounds {
                        min: -500,
                        max: 500,
                    },
                    Bounds {
                        min: -1000,
                        max: 1000,
                    },
                ],
            },
            0,
            &mut motor,
        )
        .unwrap();
    let receipt = runtime.submit(
        Action {
            boot_id: 42,
            requester: 7,
            lease_id: lease.id,
            action_id: 1,
            sequence: 1,
            capability: DRIVE,
            based_on_epoch: runtime.epoch(),
            execute_after: 0,
            deadline: 20,
            valid_for_ms: 100,
            parameters: [400, 0],
        },
        7,
        0,
        0,
        &mut motor,
    );
    assert_eq!(receipt.decision, Decision::Executed);
    // Local supervisor keeps running when the remote command stream disappears.
    for now in (10..=100).step_by(10) {
        runtime.tick(now, &mut motor);
    }
    assert_eq!(motor.velocity, [0; 2]);
    assert_eq!(runtime.state(), RuntimeState::SafeIdle);
    println!(
        "Executed once, then locally stopped at stream expiry; {} receipts retained",
        runtime.receipt_count()
    );
}
