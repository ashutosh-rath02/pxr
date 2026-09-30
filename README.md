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

**Status:** v0.2.0 developer preview. Host tests and Cortex-M/RISC-V emulation pass.
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
- **Replay handling:** action-ID history and per-lease sequence checks precede dispatch.
- **Local supervision:** stream expiry, lease expiry, sensor freshness, and latched e-stop.
- **Evidence:** receipts record decisions, requested parameters, driver observations, and authority context.

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

## Benchmarks

### Controlled host comparison

The [comparison harness](crates/runtime-sim/examples/compare.rs) runs four paths
against identical inputs and the same driver callbacks. The small guard implements
one capability, bounds, sequencing, a time window, and local flags. PXR additionally
enforces leases, boot/epoch checks, replay history, supervision, and receipts.

| Path | Median batch cost per action, range across five runs |
|---|---:|
| Direct driver + observation check | 19.7–21.9 ns |
| Small handwritten guard + driver | 21.5–23.8 ns |
| PXR with a typed action | 70.5–79.0 ns |
| PXR with frame decoding and CRC | 583.3–661.6 ns |

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
| Cortex-M3 | 21,920 B | 11,128 B | 2,228 B |
| RISC-V RV32IMC | 19,296 B | 11,128 B | 2,148 B |

Code includes the C harness and startup support. The harness reserves 16 KiB for
context storage and 32 KiB for stack; observed stack use covers the exercised paths.
The Rust runtime alone occupies 11,120 bytes on the measured 64-bit host.
[Raw firmware measurements](docs/evidence/qemu.json)

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

47 core/codec/conformance tests cover admission, lease scope, replay, time/epoch checks, e-stop
recovery, driver failures, and receipt encoding. Coverage includes randomized
fault transitions and exhaustive single-bit corruption of action/receipt frames.
CI runs on Linux, Windows, and macOS, checks Rust 1.85, links a C caller, and
executes firmware on two emulated instruction sets. The playground adds four Rust
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
volatile; receipts are unsigned and their ring buffer overwrites old entries.
Device-specific constraints, authenticated ingress, physical protection, and
measured scheduling budgets remain the integrator's responsibility.

## Contributing

Feedback from firmware and robotics developers is welcome, especially real-device
ports, measured scheduling behavior, and comparisons with existing control code.
Include a reproducer and target/toolchain details in [issues](https://github.com/ashutosh-rath02/pxr/issues).
Report vulnerabilities through [private security reporting](https://github.com/ashutosh-rath02/pxr/security/advisories/new).

[MIT license](LICENSE). Created by Ashutosh Rath.
