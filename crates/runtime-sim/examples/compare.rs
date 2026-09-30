//! Compare policy costs using the same driver and valid motor workload.
//! cargo run --release -p pxr-runtime-sim --example compare
//! The small guard intentionally provides fewer guarantees than PXR; it is not a substitute.
use pxr_runtime_codec::{decode, encode};
use pxr_runtime_core::{profile::*, *};
use pxr_runtime_sim::SimDriver;
use std::{hint::black_box, mem::size_of, time::Instant};

const BATCH: usize = 1024;
const ROUNDS: usize = 200;
const WARMUP: usize = 8;

#[derive(Default)]
struct BenchDriver(SimDriver);
impl Driver for BenchDriver {
    // All four paths use these same non-inlined callbacks.
    #[inline(never)]
    fn observe(&mut self, resource: u8) -> Result<Observation, DriverError> {
        self.0.observe(black_box(resource))
    }
    #[inline(never)]
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        self.0.execute(black_box(capability), black_box(parameters))
    }
    #[inline(never)]
    fn fallback(&mut self, resource: u8, reason: Reason) -> Result<(), DriverError> {
        self.0.fallback(black_box(resource), black_box(reason))
    }
}

#[derive(Default)]
struct SmallGuard {
    last_sequence: u64,
}
impl SmallGuard {
    // A narrow handwritten baseline: one motor capability, bounds, sequence,
    // time window and local flags. No lease, boot/epoch fence, replay ring,
    // supervisor, sensor-age check, fault latch or receipt history.
    fn admit(&mut self, a: &Action, now: u64, received: u64, flags: u32) -> bool {
        if a.capability != DRIVE
            || a.sequence <= self.last_sequence
            || !(-1000..=1000).contains(&a.parameters[0])
            || !(-2000..=2000).contains(&a.parameters[1])
            || a.execute_after >= a.deadline
            || now < a.execute_after
            || now >= a.deadline
            || received > now
            || a.valid_for_ms == 0
            || a.valid_for_ms > 1000
            || !received
                .checked_add(a.valid_for_ms as u64)
                .is_some_and(|expires| now < expires)
            || flags != 0
        {
            return false;
        }
        self.last_sequence = a.sequence;
        true
    }
}

fn direct(driver: &mut BenchDriver, a: Action) -> bool {
    let before = driver.observe(0).unwrap();
    driver.execute(a.capability, a.parameters).unwrap();
    let after = driver.observe(0).unwrap();
    black_box(before);
    black_box(after).parameters == a.parameters
}

#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Guard,
    Typed,
    Frame,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct_driver",
            Self::Guard => "small_guard",
            Self::Typed => "pxr_typed",
            Self::Frame => "pxr_decoded_frame",
        }
    }
}

struct Case {
    mode: Mode,
    driver: BenchDriver,
    guard: SmallGuard,
    runtime: Option<Runtime>,
    samples: Vec<f64>,
    accepted: usize,
}
impl Case {
    fn new(mode: Mode) -> Self {
        let mut driver = BenchDriver::default();
        let runtime = if matches!(mode, Mode::Typed | Mode::Frame) {
            let mut runtime =
                Runtime::new(Config::default(), &CAPABILITIES[..2], 42, 0, 0, &mut driver).unwrap();
            let lease = runtime
                .acquire(
                    LeaseRequest {
                        owner: 7,
                        resource: 0,
                        capabilities: (1 << DRIVE) | (1 << STOP),
                        duration_ms: 2000,
                        limits: [Bounds::ANY; 2],
                    },
                    0,
                    &mut driver,
                )
                .unwrap();
            assert_eq!(lease.id, 1);
            assert_eq!(runtime.epoch(), 2);
            Some(runtime)
        } else {
            None
        };
        Self {
            mode,
            driver,
            runtime,
            guard: SmallGuard::default(),
            samples: Vec::with_capacity(ROUNDS),
            accepted: 0,
        }
    }

    fn step(&mut self, a: Action) -> bool {
        match self.mode {
            Mode::Direct => direct(&mut self.driver, a),
            Mode::Guard => {
                self.guard
                    .admit(&a, black_box(0), black_box(0), black_box(0))
                    && direct(&mut self.driver, a)
            }
            Mode::Typed | Mode::Frame => {
                let receipt = self.runtime.as_mut().unwrap().submit(
                    a,
                    black_box(7),
                    black_box(0),
                    black_box(0),
                    &mut self.driver,
                );
                black_box(receipt).decision == Decision::Executed
            }
        }
    }
}

fn action(id: u64) -> Action {
    Action {
        boot_id: 42,
        requester: 7,
        lease_id: 1,
        action_id: id,
        sequence: id,
        capability: DRIVE,
        based_on_epoch: 2,
        execute_after: 0,
        deadline: 20,
        valid_for_ms: 100,
        parameters: [(id % 501) as i32, -200],
    }
}

fn check_baseline() {
    let mut guard = SmallGuard::default();
    let a = action(1);
    assert!(guard.admit(&a, 0, 0, 0));
    assert!(!guard.admit(&a, 0, 0, 0));
    let mut a = action(2);
    a.parameters[0] = 1001;
    assert!(!guard.admit(&a, 0, 0, 0));
    a.parameters[0] = 400;
    assert!(!guard.admit(&a, 20, 0, 0));
    assert!(!guard.admit(&a, 0, 0, ESTOP));
    assert!(guard.admit(&a, 0, 0, 0));
}

fn main() {
    check_baseline();
    let mut cases = [Mode::Direct, Mode::Guard, Mode::Typed, Mode::Frame].map(Case::new);
    for round in 0..WARMUP + ROUNDS {
        // Identical inputs; generation/encoding is outside every timed region.
        let actions: Vec<_> = (0..BATCH)
            .map(|i| action((round * BATCH + i + 1) as u64))
            .collect();
        let frames: Vec<_> = actions.iter().map(encode).collect();
        // Rotate order to reduce fixed ordering/thermal bias.
        for offset in 0..cases.len() {
            let case = &mut cases[(round + offset) % 4];
            let mut accepted = 0;
            let start = Instant::now();
            if matches!(case.mode, Mode::Frame) {
                for frame in &frames {
                    let a = decode(black_box(frame)).unwrap();
                    accepted += usize::from(black_box(case.step(a)));
                }
            } else {
                for &a in &actions {
                    accepted += usize::from(black_box(case.step(black_box(a))));
                }
            }
            let elapsed = start.elapsed().as_nanos() as f64 / BATCH as f64;
            assert_eq!(accepted, BATCH);
            case.accepted += accepted;
            assert_eq!(case.driver.0.velocity, actions.last().unwrap().parameters);
            if round >= WARMUP {
                case.samples.push(elapsed);
            }
        }
    }
    println!("{{\"schema\":1,\"version\":\"{}\",\"os\":\"{}\",\"architecture\":\"{}\",\"batch_size\":{BATCH},\"measured_batches\":{ROUNDS},\"warmup_batches\":{WARMUP},\"controller_tick_ms\":0,\"runtime_bytes\":{},\"hardware_validated\":false,\"cases\":[",env!("CARGO_PKG_VERSION"),std::env::consts::OS,std::env::consts::ARCH,size_of::<Runtime>());
    for (i, case) in cases.iter_mut().enumerate() {
        assert_eq!(case.accepted, (ROUNDS + WARMUP) * BATCH);
        assert_eq!(case.driver.0.executions as usize, case.accepted);
        if let Some(runtime) = &case.runtime {
            assert_eq!(runtime.state(), RuntimeState::Armed);
            assert_eq!(runtime.receipt_count(), RECEIPT_CAPACITY);
        }
        let samples = format!("{:?}", case.samples);
        case.samples.sort_by(f64::total_cmp);
        println!("{}{{\"name\":\"{}\",\"batch_median_ns_per_action\":{:.3},\"batch_p99_ns_per_action\":{:.3},\"batch_max_ns_per_action\":{:.3},\"verified_executions\":{},\"batch_samples_ns_per_action\":{}}}",if i==0 {""} else {","},case.mode.name(),case.samples[ROUNDS/2],case.samples[ROUNDS*99/100],case.samples[ROUNDS-1],case.accepted,samples);
    }
    println!("]}}");
}
