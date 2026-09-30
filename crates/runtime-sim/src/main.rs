use pxr_runtime_codec::{decode, encode, FRAME_SIZE};
use pxr_runtime_core::{profile::*, *};
use pxr_runtime_sim::*;
use std::{env, hint::black_box, process::ExitCode, time::Instant};

fn label(r: Receipt) -> &'static str {
    if r.decision == Decision::Executed {
        return "EXECUTED";
    }
    match r.reason {
        Reason::Bound => "REJECTED_BOUND",
        Reason::Stale => "REJECTED_STALE",
        Reason::Authority | Reason::LeaseExpired => "REJECTED_AUTHORITY",
        Reason::Duplicate | Reason::OldSequence => "REJECTED_DUPLICATE",
        Reason::Precondition => "REJECTED_PRECONDITION",
        Reason::Watchdog => "WATCHDOG_TRIGGERED",
        Reason::StreamExpired => "STREAM_EXPIRED",
        Reason::Epoch => "REJECTED_EPOCH",
        Reason::Estop => "ESTOP_ACTIVE",
        Reason::Verification => "VERIFICATION_FAILED",
        _ => "REJECTED",
    }
}
fn print_receipt(r: Receipt) {
    println!("{{\"receipt_id\":{},\"action_id\":{},\"sequence\":{},\"capability\":{},\"resource\":{},\"lease_id\":{},\"decision\":\"{:?}\",\"reason\":\"{:?}\",\"status\":\"{}\",\"epoch\":{},\"received_at\":{},\"admitted_at\":{},\"executed_at\":{},\"requested\":[{},{}],\"observed\":[{},{}],\"pre_state\":{},\"post_state\":{},\"original_receipt\":{},\"dispatched\":{},\"observed_valid\":{}}}",
        r.receipt_id, r.action_id, r.sequence, r.capability, r.resource, r.lease_id, r.decision, r.reason, label(r),
        r.epoch, r.received_at, r.admitted_at, r.executed_at, r.requested[0], r.requested[1],
        r.observed[0], r.observed[1], r.pre_state, r.post_state, r.original_receipt, r.dispatched, r.observed_valid);
}

fn demo() -> Result<(), String> {
    let cases = [
        "valid",
        "bound",
        "stale",
        "authority",
        "duplicate",
        "precondition",
        "watchdog",
        "epoch",
        "verification",
    ];
    for case in cases {
        let mut s = Simulation::default();
        let resource = u8::from(case == "duplicate" || case == "precondition");
        if case == "precondition" {
            s.state(ARM_MOVING);
        }
        let lease = s.grant(resource, 500).map_err(|e| format!("{e:?}"))?;
        let mut a = s.action(
            lease,
            if resource == 0 { DRIVE } else { OPEN },
            1,
            if resource == 0 { [400, 200] } else { [0; 2] },
        );
        let expected = match case {
            "valid" => Reason::Ok,
            "bound" => {
                a.parameters[0] = 1001;
                Reason::Bound
            }
            "stale" => {
                s.advance(20);
                Reason::Stale
            }
            "authority" => {
                a.lease_id = 999;
                Reason::Authority
            }
            "duplicate" => {
                s.submit(a);
                Reason::Duplicate
            }
            "precondition" => Reason::Precondition,
            "watchdog" => {
                s.submit(a);
                s.runtime.tick(51, &mut s.driver);
                Reason::Watchdog
            }
            "epoch" => {
                s.state(ARM_MOVING);
                Reason::Epoch
            }
            "verification" => {
                s.driver.wrong_feedback = true;
                Reason::Verification
            }
            _ => unreachable!(),
        };
        let receipt = if case == "watchdog" {
            (0..s.runtime.receipt_count())
                .filter_map(|i| s.runtime.receipt(i))
                .find(|r| r.reason == Reason::Watchdog)
                .ok_or("watchdog did not trigger")?
        } else {
            s.submit(decode(&encode(&a)).map_err(|e| format!("{e:?}"))?)
        };
        if receipt.reason != expected {
            return Err(format!(
                "scenario {case}: expected {expected:?}, got {:?}",
                receipt.reason
            ));
        }
        print_receipt(receipt);
    }
    Ok(())
}

fn stats(mut values: Vec<u128>) -> (u128, u128, u128, u128) {
    values.sort_unstable();
    let n = values.len();
    (
        values.iter().sum::<u128>() / n as u128,
        values[n / 2],
        values[(n * 99 / 100).min(n - 1)],
        values[n - 1],
    )
}
fn bench(n: usize) -> Result<(), String> {
    if !(100..=1_000_000).contains(&n) {
        return Err("iterations must be between 100 and 1000000".into());
    }
    let mut s = Simulation::default();
    let l = s.grant(0, 2000).map_err(|e| format!("{e:?}"))?;
    let mut a = s.action(l, DRIVE, 1, [400, 200]);
    let mut samples = Vec::with_capacity(n);
    let mut duplicate = Vec::with_capacity(n);
    let mut direct = Vec::with_capacity(n);
    // Warm caches before measuring. Wall-clock time does not advance the simulated controller clock.
    for i in 1..=1000 {
        a.action_id = i;
        a.sequence = i;
        black_box(s.submit(black_box(a)));
    }
    for i in 1001..n as u64 + 1001 {
        a.action_id = i;
        a.sequence = i;
        let start = Instant::now();
        let r = black_box(s.submit(black_box(a)));
        samples.push(start.elapsed().as_nanos());
        if r.decision != Decision::Executed {
            return Err(format!("benchmark action rejected: {:?}", r.reason));
        }
        let start = Instant::now();
        black_box(s.submit(black_box(a)));
        duplicate.push(start.elapsed().as_nanos());
        let start = Instant::now();
        black_box(s.driver.observe(black_box(0)).unwrap());
        s.driver
            .execute(black_box(DRIVE), black_box([400, 200]))
            .unwrap();
        black_box(s.driver.observe(black_box(0)).unwrap());
        direct.push(start.elapsed().as_nanos());
    }
    let full = stats(samples);
    let dup = stats(duplicate);
    let baseline = stats(direct);
    let batch_start = Instant::now();
    for i in n as u64 + 1001..2 * n as u64 + 1001 {
        a.action_id = i;
        a.sequence = i;
        black_box(s.submit(black_box(a)));
    }
    let batch_ns = batch_start.elapsed().as_nanos().max(1);
    let mut codec = Vec::with_capacity(n);
    for _ in 0..n {
        let start = Instant::now();
        black_box(decode(&black_box(encode(black_box(&a))))).map_err(|e| format!("{e:?}"))?;
        codec.push(start.elapsed().as_nanos());
    }
    let codec = stats(codec);
    struct BenchDriver(SimDriver);
    impl Driver for BenchDriver {
        fn observe(&mut self, r: u8) -> Result<Observation, DriverError> {
            self.0.observe(r)
        }
        fn execute(&mut self, _: u16, p: [i32; 2]) -> Result<(), DriverError> {
            self.0.execute(DRIVE, p)
        }
        fn fallback(&mut self, r: u8, reason: Reason) -> Result<(), DriverError> {
            self.0.fallback(r, reason)
        }
    }
    let mut caps = [CAPABILITIES[0]; MAX_CAPABILITIES];
    for (i, c) in caps.iter_mut().enumerate() {
        c.id = i as u16 + 1;
    }
    let mut driver = BenchDriver(SimDriver::default());
    let mut runtime = Runtime::new(Config::default(), &caps, 42, 0, 0, &mut driver)
        .map_err(|e| format!("{e:?}"))?;
    let lease = runtime
        .acquire(
            LeaseRequest {
                owner: 7,
                resource: 0,
                capabilities: 1 << 16,
                duration_ms: 2000,
                limits: [Bounds::ANY; 2],
            },
            0,
            &mut driver,
        )
        .map_err(|e| format!("{e:?}"))?;
    a.capability = 16;
    a.lease_id = lease.id;
    a.based_on_epoch = runtime.epoch();
    let mut worst = Vec::with_capacity(n);
    for i in 1..=n as u64 {
        a.action_id = i;
        a.sequence = i;
        let start = Instant::now();
        let r = black_box(runtime.submit(black_box(a), 7, 0, 0, &mut driver));
        worst.push(start.elapsed().as_nanos());
        if r.decision != Decision::Executed {
            return Err("max-capacity benchmark failed".into());
        }
    }
    let worst = stats(worst);
    let mut stop = Simulation::default();
    let l = stop.grant(0, 500).map_err(|e| format!("{e:?}"))?;
    let a = stop.action(l, DRIVE, 1, [400, 0]);
    stop.submit(a);
    while stop.driver.velocity != [0; 2] && stop.now <= 500 {
        stop.advance(10);
    }
    if stop.now != 100 {
        return Err("stream stop deadline changed".into());
    }
    println!("{{\"profile\":\"host-simulation\",\"architecture\":\"{}\",\"os\":\"{}\",\"iterations\":{},\"runtime_bytes\":{},\"receipt_bytes\":{},\"receipt_capacity\":{},\"replay_capacity\":{},\"frame_bytes\":{},\"full_mean_ns\":{},\"full_p50_ns\":{},\"full_p99_ns\":{},\"full_max_observed_ns\":{},\"direct_mean_ns\":{},\"duplicate_mean_ns\":{},\"duplicate_p99_ns\":{},\"measured_batch_actions_per_second\":{},\"codec_mean_ns\":{},\"codec_p99_ns\":{},\"max_capacity_p99_ns\":{},\"max_capacity_max_observed_ns\":{},\"virtual_stream_stop_ms\":{},\"virtual_tick_ms\":10,\"hardware_validated\":false}}",
        env::consts::ARCH, env::consts::OS, n, std::mem::size_of::<Runtime>(), std::mem::size_of::<Receipt>(),
        RECEIPT_CAPACITY, REPLAY_CAPACITY, FRAME_SIZE, full.0, full.1, full.2, full.3, baseline.0, dup.0, dup.2,
        n as u128 * 1_000_000_000u128 / batch_ns, codec.0, codec.2, worst.2, worst.3, stop.now);
    Ok(())
}

/// A deliberately small, reproducible trace language. No wall clock or transport is involved.
fn replay(path: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut s = Simulation::default();
    for (line, raw) in source.lines().enumerate() {
        let words: Vec<_> = raw
            .split('#')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        if words.is_empty() {
            continue;
        }
        let error = |msg: &str| format!("{path}:{}: {msg}", line + 1);
        let n = |i: usize| -> Result<u64, String> {
            words
                .get(i)
                .ok_or_else(|| error("missing argument"))?
                .parse()
                .map_err(|_| error("expected unsigned integer"))
        };
        let signed = |i: usize| -> Result<i32, String> {
            words
                .get(i)
                .ok_or_else(|| error("missing parameter"))?
                .parse()
                .map_err(|_| error("expected i32 parameter"))
        };
        let u8n = |i| u8::try_from(n(i)?).map_err(|_| error("u8 out of range"));
        let u32n = |i| u32::try_from(n(i)?).map_err(|_| error("u32 out of range"));
        match words[0] {
            "lease" if words.len() == 3 => {
                s.grant(u8n(1)?, u32n(2)?)
                    .map_err(|e| error(&format!("{e:?}")))?;
            }
            "advance" if words.len() == 2 => {
                let ms = n(1)?;
                if ms > 60000 {
                    return Err(error("advance limit is 60000 ms"));
                }
                s.advance(ms);
            }
            "state" if words.len() == 2 => s.state(u32n(1)?),
            "estop" if words.len() == 1 => s.runtime.emergency_stop(s.now, &mut s.driver),
            "recover" if words.len() == 1 => s
                .runtime
                .recover_local(s.now, &mut s.driver)
                .map_err(|e| error(&format!("{e:?}")))?,
            "action" if words.len() == 10 => {
                let cap = u16::try_from(n(1)?).map_err(|_| error("capability out of range"))?;
                let resource = s
                    .runtime
                    .capabilities()
                    .find(|c| c.id == cap)
                    .ok_or_else(|| error("unknown capability"))?
                    .resource;
                let l = s.runtime.lease(resource).ok_or_else(|| error("no lease"))?;
                let mut a = s.action(l, cap, n(2)?, [signed(4)?, signed(5)?]);
                a.sequence = n(3)?;
                a.deadline = n(6)?;
                a.valid_for_ms = u32n(7)?;
                a.based_on_epoch = n(8)?;
                a.execute_after = n(9)?;
                print_receipt(s.submit(a));
            }
            _ => {
                return Err(error(
                    "invalid command or argument count; see examples/demo.pxr",
                ))
            }
        }
    }
    println!("{{\"final_state\":\"{:?}\",\"tick\":{},\"epoch\":{},\"velocity\":[{},{}],\"gripper_open\":{},\"driver_executions\":{},\"fallback_calls\":{}}}",
        s.runtime.state(), s.now, s.runtime.epoch(), s.driver.velocity[0], s.driver.velocity[1], s.driver.gripper_open, s.driver.executions, s.driver.fallbacks);
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("demo") if args.len() == 1 => demo(),
        Some("bench") if args.len() <= 2 => bench(args.get(1).map_or(Ok(20000), |s| {
            s.parse().map_err(|_| "invalid iteration count")
        })?),
        Some("replay") if args.len() == 2 => replay(&args[1]),
        Some("frame") if args.len() == 1 => {
            let mut s = Simulation::default();
            let l = s.grant(0, 500).unwrap();
            for b in encode(&s.action(l, DRIVE, 99, [400, -200])) {
                print!("{b:02x}");
            }
            println!();
            Ok(())
        }
        Some("--version") => {
            println!("pxr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None | Some("--help") | Some("help") => {
            println!("PXR — deterministic physical execution runtime\n\n  pxr demo             Run nine checked failure scenarios (JSONL)\n  pxr bench [count]    Measure admission, duplicate and direct driver paths (JSON)\n  pxr replay FILE      Run a deterministic .pxr trace (JSONL)\n  pxr frame            Print a reference action frame as hex\n\nSimulation / research release. See SAFETY_MODEL.md for integration obligations.");
            Ok(())
        }
        _ => Err("unknown command; run pxr --help".into()),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pxr: {e}");
            ExitCode::FAILURE
        }
    }
}
