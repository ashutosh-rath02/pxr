# PXR

### Deterministic action admission for embedded systems

[![CI](https://github.com/ashutosh-rath02/pxr/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ashutosh-rath02/pxr/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust: 1.85+](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](Cargo.toml)

PXR checks actuator commands against local authority, timing, parameter limits,
and device state before calling a driver. When a command stream or lease expires,
the local controller invokes its configured fallback.

**Rust `no_std` · Fixed memory · C/C++ interface · No external Rust dependencies**

[Try in your browser](https://ashutosh-rath02.github.io/pxr/) · [Quickstart](#quickstart) · [Embedding](#embedding) · [Benchmarks](#benchmarks) · [Downloads](https://github.com/ashutosh-rath02/pxr/releases/tag/v0.2.0)

```text
Planner → authenticated adapter → PXR → device driver → actuator
                                   ↑
                        local sensors, clock, e-stop
```

Use PXR when an AI system, planner, or remote application sends bounded commands
and the device must decide whether each command is still authorized and valid.
The application supplies the transport, authenticated identity, sensors, and drivers.

**Status:** v0.2.0 developer preview. `main` also contains unreleased changes:
an interrupt-safe e-stop signal, a receipt sink, eviction that protects executions,
and a required `pxr_platform_panic` hook for freestanding C builds.
Host tests and Cortex-M/RISC-V emulation pass.
Physical board timing and actuator behavior require device-specific validation.

## Quickstart

**[Open the interactive playground](https://ashutosh-rath02.github.io/pxr/)** to try
nine scenarios without installing anything. It compiles the actual Rust runtime
to WebAssembly: change command parameters, trigger failures, inspect receipts,
and export the results as JSON. All device behavior and time are simulated.

With [Rust 1.85 or newer](https://www.rust-lang.org/tools/install):

```sh
cargo install --git https://github.com/ashutosh-rath02/pxr --tag v0.2.0 --locked pxr-runtime-sim
pxr demo
```

The demo runs nine checked scenarios and prints JSON receipts. It exits with a
failure code if any expected result changes. The reference profile exposes
`motor.drive`, `motor.stop`, `gripper.open`, and `gripper.close`.

| Scenario | Result |
|---|---|
| Valid command, lease, timing, and state | `EXECUTED`, with driver feedback |
| Velocity exceeds the configured limit | `REJECTED_BOUND` |
| Command misses its validity window | `REJECTED_STALE` |
| Action ID is submitted again | `REJECTED_DUPLICATE` |
| Local state blocks the operation | `REJECTED_PRECONDITION` |
| Supervisor detects loss of progress | `WATCHDOG_TRIGGERED`, followed by fallback |

[Prebuilt downloads](https://github.com/ashutosh-rath02/pxr/releases/tag/v0.2.0)
include Linux x86-64, Windows x86-64, and macOS Apple Silicon CLI/SDK bundles.
Check `SHA256SUMS` and the platform requirements on the release page before use.

To explore traces and capabilities from a checkout:

```sh
git clone https://github.com/ashutosh-rath02/pxr.git
cd pxr
cargo run --release -p pxr-runtime-sim -- capabilities
cargo run --release -p pxr-runtime-sim -- replay --audit examples/demo.pxr
```

### Build the browser playground

The [browser adapter](crates/playground/src/lib.rs) calls the same core and
simulated driver as the native examples. It uses `std` and heap storage for the
browser interface; the embedded core keeps its fixed-memory `no_std` contract.
The [static frontend](web) needs no bundler, backend, account, or analytics service.

```sh
rustup target add wasm32-unknown-unknown
python scripts/build_playground.py
node --test scripts/verify_playground.mjs
python -m http.server 8080 --bind 127.0.0.1 --directory dist/playground
```

Open `http://127.0.0.1:8080`. Building requires Python 3.11+; the WASM checks use
Node.js 22+. Share a scenario with a URL such as
[`?scenario=stream`](https://ashutosh-rath02.github.io/pxr/?scenario=stream).

## Execution contract

- **Authority:** exclusive resource leases bind a principal to capabilities and limits.
- **Freshness:** controller-clock deadlines, receive-relative TTL, boot IDs, and state epochs.
- **Replay handling:** per-lease sequence checks and a 32-entry action-ID history precede dispatch.
- **Local supervision:** stream expiry, lease expiry, sensor freshness, and latched e-stop.
  An interrupt-safe `EstopSignal` lets an ISR request e-stop; the next runtime call latches it.
- **Evidence:** receipts record decisions, requested parameters, driver observations, and authority context.
  When the 64-entry ring is full, rejected receipts are evicted first, so a flood of invalid
  frames evicts at most the single oldest non-rejected receipt. `Driver::record_receipt` (C: `pxr_set_receipt_sink`) sees every
  receipt, which lets the platform persist or sign the audit trail.

The core uses fixed capacities: 16 capabilities, 8 resources, 32 replay entries,
and 64 receipts. Action frames are 92 bytes; portable receipt frames are 140 bytes.
Transport framing and authentication belong to the adapter.

## Embedding

Implement `Driver::execute`, `Driver::observe`, and `Driver::fallback`. Register
the device's capabilities, grant authorized leases, submit actions, and service
the supervisor independently of incoming traffic.

| Interface | Starting point |
|---|---|
| Rust | [Runnable embedding example](crates/runtime-core/examples/embedding.rs) |
| C / C++ | [Public header](include/pxr.h) and [linked C example](examples/c-embedding/main.c) |
| Binary formats | [Action and receipt codecs](crates/runtime-codec/src) |
| Bare metal | [Cortex-M and RISC-V firmware harness](ports/qemu) |

```toml
[dependencies]
pxr-runtime-core = { git = "https://github.com/ashutosh-rath02/pxr", tag = "v0.2.0" }
pxr-runtime-codec = { git = "https://github.com/ashutosh-rath02/pxr", tag = "v0.2.0" }
```

```sh
cargo run -p pxr-runtime-core --example embedding
cargo build --release -p pxr-runtime-c-api

# Freestanding Cortex-M archive; RISC-V is also supported.
rustup target add thumbv7em-none-eabihf
cargo build --release -p pxr-runtime-c-api --no-default-features --target thumbv7em-none-eabihf
```

Serialize access to each runtime. Callbacks must be bounded, synchronous, and
non-reentrant. Use a fresh boot ID and one monotonic controller clock. Supply
trusted sensor updates and an independent hardware watchdog. C callers allocate
storage using `pxr_context_size()` and `pxr_context_align()`; inspect the receipt
decision after `pxr_submit`, since a zero return indicates a receipt was produced.
Calls on storage that was never initialized, or whose initialization failed, return `-1`.

Freestanding C archives call `pxr_platform_panic()`, which the firmware must define.
It must force every actuator to its safe state, then reset or halt. It must never return.

## Benchmarks

### Controlled host comparison

The [comparison harness](crates/runtime-sim/examples/compare.rs) runs four paths
against identical inputs and the same driver callbacks. The small guard implements
one capability, bounds, sequencing, a time window, and local flags. PXR additionally
enforces leases, boot/epoch checks, replay history, supervision, and receipts.

| Path | Median batch cost per action, range across five runs |
|---|---:|
| Direct driver + observation check | 22.9–26.5 ns |
| Small handwritten guard + driver | 24.9–28.4 ns |
| PXR with a typed action | 82.1–91.9 ns |
| PXR with frame decoding and CRC | 614.4–711.7 ns |

Measured on Windows 11 x86-64, Intel Core i7-1260P, Rust 1.98.1, release/LTO.
Each run uses 200 batches of 1,024 actions per path after eight warmup batches.
Execution order rotates; inputs and frames are prepared outside timed regions.
Every dispatch count and final driver output is checked.

These are batch averages with virtual controller time fixed at zero. They measure
host processing cost; they do not establish individual-action tail latency, MCU
worst-case timing, or a real control-loop rate. The guard has fewer guarantees.
The frame path includes decoding and CRC verification, and excludes encoding and transport.

[Raw samples, source hashes, and environment](docs/evidence/comparison.json)

```sh
# Python 3.11+; builds the harness and records five runs.
python scripts/compare.py

# Existing per-action host timing and fault demonstration:
cargo run --release -p pxr-runtime-sim -- bench 20000
```

### Embedded footprint

Both test images execute under QEMU with the same `no_std` core and C ABI that ship
in the SDK.

| Test firmware | Linked code | C context | Observed stack use |
|---|---:|---:|---:|
| Cortex-M3 | 22,672 B | 11,336 B | 2,428 B |
| RISC-V RV32IMC | 19,556 B | 11,336 B | 2,372 B |

Code includes the C harness and startup support. The harness reserves 16 KiB for
context storage and 32 KiB for stack; observed stack use covers the exercised paths.
The Rust runtime alone occupies 11,312 bytes on the measured 64-bit host. The firmware
also raises the e-stop signal from a real timer interrupt (SysTick and the RISC-V CLINT)
while the main loop runs, then checks that the next call stops the motor.

### Emulated instruction counts

A separate [timing image](ports/qemu/timing.c) runs under QEMU `-icount shift=0`.
Each call is measured 32 times with the replay history and receipt ring already full,
so every call takes its longest path. The table gives the worst case per call.

| Call | RISC-V RV32IMC (exact) | Cortex-M3 (±80) |
|---|---:|---:|
| `pxr_submit`, executed | 5,388 | 2,640 |
| `pxr_submit`, duplicate | 5,674 | 2,880 |
| `pxr_submit`, out of bounds | 5,137 | 2,400 |
| `pxr_submit`, corrupt CRC | 1,566 | 880 |
| `pxr_tick`, idle | 237 | 240 |
| `pxr_update_state`, flags changed | 348 | 320 |
| `pxr_estop`, two fallbacks | 3,651 | 1,120 |

These are emulated retired instructions, not cycles. Real cores add pipeline stalls,
flash wait states, and bus contention, and QEMU does not model them. On Cortex-M the
counter is SysTick, which QEMU advances once per 80 instructions; RISC-V uses `minstret`.
Both are calibrated against a 200,000-instruction loop.
[Raw counts](docs/evidence/qemu.json)

### Plant-in-the-loop simulation

[`pxr plant`](crates/runtime-sim/src/plant.rs) drives a simulated motor through PXR.
The motor has a 40 ms first-order lag and a 5 ms command delay. A controller sends
800 mm/s every 20 ms, and sensors and the supervisor run every 10 ms. A fault is
injected at t = 1 s, and each result is checked against a bound derived from the
configuration and loop timing.

| Fault | Detected after | Motor stopped after | Coast distance |
|---|---:|---:|---:|
| Controller goes silent | 80 ms | 269 ms | 99.6 mm |
| E-stop pressed | 0 ms | 189 ms | 35.6 mm |
| Obstacle reported | 0 ms | 189 ms | 35.6 mm |
| Sensor feed goes stale | 240 ms | 429 ms | 227.6 mm |
| PXR's thread stalls for 200 ms | 200 ms | 389 ms | 195.6 mm |

With the default configuration, sensor freshness (`state_ttl_ms` = 250) dominates the
stopping distance, so lower it for fast actuators. A stalled PXR thread cannot stop
anything until it resumes, so production systems need an independent hardware watchdog.
"Stopped" means below 1% of cruise speed in this model; it is not a physical measurement.
[Raw results](docs/evidence/plant.jsonl)

### External evaluation targets

| Target | Useful comparison |
|---|---|
| Handwritten firmware guard | Policy overhead, storage, and maintenance effort on the same board; host cost measured above |
| [EdgeEmbed](https://edgeembed.com/docs/getting-started/) | Matched decision policies through its Linux C SDK; account for its different event model |
| [Invariant](https://github.com/clay-good/invariant) | Overlapping command-validation rules; measure cryptographic verification and signed auditing separately |
| [PX4 Offboard](https://docs.px4.io/main/en/flight_modes/offboard) | Controller-loss and fallback behavior under equivalent failure traces |

External product comparisons have **not been measured**. A meaningful comparison
needs the same host or board, policy, driver workload, and measurement boundaries.

## Verification

Core, codec, conformance, and C ABI tests cover admission, lease scope, replay, time/epoch
checks, e-stop recovery, driver failures, and receipt encoding. Coverage includes exhaustive
single-bit corruption of action/receipt frames and a model-based test that checks admission
invariants after every step of long seeded random operation sequences with injected driver faults.
CI runs on Linux, Windows, and macOS, checks Rust 1.85, links a C caller, and
executes firmware on two emulated instruction sets. Additional CI jobs:

- [Kani](crates/runtime-core/src/proofs.rs) bounded proofs. Each harness covers every action
  field, timestamp, sensor flag, and driver result in its scenario. They show that a
  dispatched action passed every admission check, that e-stop blocks dispatch until local
  recovery, that `tick` revokes expired or unsupervised leases, and that receipt IDs stay ordered.
- [cargo-fuzz](fuzz/fuzz_targets) targets for both decoders and for codec-driven runtime operation sequences.
- Miri with strict provenance over the C ABI entry points.
- [cargo-mutants](.cargo/mutants.toml) on the core and codec; every non-equivalent mutant is killed.
- Compile-time layout checks for every C struct, pinned in both `include/pxr.h` and Rust.

The playground adds four Rust
adapter tests, 14 checks against the compiled WebAssembly, and eight browser tests
covering desktop/mobile interaction, receipt export, and runtime-loading failures.
GitHub Pages deploys only after the full CI suite passes.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# Linux host: execute both freestanding firmware images.
sudo apt-get install clang lld llvm qemu-system-arm qemu-system-misc
rustup target add thumbv7m-none-eabi riscv32imc-unknown-none-elf
python3 ports/qemu/build.py
```

## Deployment boundaries

`Executed` means the driver's immediate observation passed the configured checks.
Physical completion depends on the driver's observation contract. Replay state is
volatile. Receipts are unsigned unless the platform signs them in its receipt sink,
and the in-memory ring keeps only the most recent 64.
Duplicate action IDs are detected within the last 32 admitted actions. Older frames
are still fenced by lease ID and sequence. However, a client that reuses an action ID
under a new lease can have it executed again, so retries must be idempotent or use fresh IDs.
Device-specific constraints, authenticated ingress, physical protection, and
measured scheduling budgets remain the integrator's responsibility.

## Contributing

Feedback from firmware and robotics developers is welcome, especially real-device
ports, measured scheduling behavior, and comparisons with existing control code.
Include a reproducer and target/toolchain details in [issues](https://github.com/ashutosh-rath02/pxr/issues).
Report vulnerabilities through [private security reporting](https://github.com/ashutosh-rath02/pxr/security/advisories/new).

[MIT license](LICENSE). Created by Ashutosh Rath.
