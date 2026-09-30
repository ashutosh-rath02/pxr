# PXR — Physical Execution Runtime

An open, embedded-first execution boundary between AI-generated actions and physical
actuators. Rust `no_std` core, fixed memory, transport-independent semantics, C ABI.

**v0.2 is an experimental SDK release.** It includes a simulator, C and Rust embedding
examples, and bare-metal firmware tested on Cortex-M and RISC-V under QEMU.
Physical safety and board timing require device-specific validation.
See [the safety model](SAFETY_MODEL.md).

```text
AI / planner -> trusted adapter -> PXR -> existing driver -> actuator
                                   ^
                      local sensors, clock, e-stop
```

## Try it

Install [Rust](https://www.rust-lang.org/tools/install) (1.85 or newer), then:

```sh
git clone https://github.com/ashutosh-rath02/pxr.git
cd pxr
cargo run --release -p pxr-runtime-sim -- demo
cargo run --release -p pxr-runtime-sim -- replay examples/demo.pxr
cargo run --release -p pxr-runtime-sim -- bench 20000
```

Or install the CLI directly from the release tag:

```sh
cargo install --git https://github.com/ashutosh-rath02/pxr --tag v0.2.0 --locked pxr-runtime-sim
pxr demo
```

Prebuilt Linux, Windows and macOS CLI/SDK packages are on the
[releases page](https://github.com/ashutosh-rath02/pxr/releases). Verify the included
SHA-256 checksums, extract the matching OS/architecture package, and run `pxr demo`.
On Windows use `pxr.exe demo`. On macOS, unsigned downloads may require local
Gatekeeper approval; building from source is also supported.

The demo checks nine scenarios and emits JSON receipts: valid execution, bounds,
staleness, missing authority, duplicates, preconditions, watchdog, epoch mismatch,
and failed verification. Its exit code is nonzero if any expected result differs.
`replay` uses an explicit virtual clock, so repeated traces produce identical output.

```sh
pxr capabilities
pxr replay --audit examples/demo.pxr > audit.jsonl
pxr frame
pxr receipt-frame
# Decode either printed hex frame:
pxr inspect HEX
```

## What ships

- Four simulated capabilities: `motor.drive`, `motor.stop`, `gripper.open`, `gripper.close`.
- Static capability registration with integer parameter bounds and state predicates.
- Exclusive resource leases with owner binding, narrower limits and replay-safe renewal.
- Controller-domain deadlines, receive-relative TTL, and independent stream expiry.
- Sequence fencing, bounded duplicate suppression, and boot-session checks.
- Latched e-stop, local recovery, sensor freshness and watchdog supervision.
- Driver feedback verification and fixed receipt history, including uncertain outcomes.
- A 92-byte action codec and 140-byte receipt codec with authority and state context.
- C APIs for encoding, configuration, capability discovery, state snapshots and receipts.
- Working Rust/C embedding examples and freestanding Cortex-M/RISC-V firmware.
- Fault tests, deterministic traces and reproducible host benchmarks.

No third-party Rust packages are required. There is no transport server, AI model,
robotics framework, dynamic policy engine, or dashboard in this v0.

## Embed it

Rust: implement `Driver::{execute, observe, fallback}`, register a bounded capability
table with `Runtime::new`, and call `tick` from the local supervisor. Authenticate
and authorize principals before granting leases. Use a fresh boot ID each restart.

C/C++: build `pxr-runtime-c-api`, include [pxr.h](include/pxr.h), and allocate context
storage using the exported size/alignment. The [C example](examples/c-embedding/main.c)
uses static memory and verifies the complete action-to-fallback path.
Follow the [integration guide](docs/INTEGRATION.md) for the full lifecycle and upgrade notes.

```sh
cargo build --release -p pxr-runtime-c-api
# Linux C integration check:
cc -std=c11 -Wall -Wextra -Werror -Iinclude examples/c-embedding/main.c \
  target/release/libpxr_runtime_c_api.a -ldl -lpthread -lm -o target/c-embedding
./target/c-embedding
```

For a bare-metal archive:

```sh
rustup target add thumbv7em-none-eabihf
cargo build --release -p pxr-runtime-c-api --no-default-features --target thumbv7em-none-eabihf
```

The same crate builds for `riscv32imc-unknown-none-elf`. The
[QEMU firmware](ports/qemu/README.md) links and executes both ARM Cortex-M3 and
RISC-V binaries with simulated drivers, recording flash sections and stack use.
Physical ports supply a clock, trusted sensors, driver, fallback policy,
supervisor scheduling and an independent hardware watchdog.

## Contracts and evidence

| Document | Read it for |
|---|---|
| [Architecture](ARCHITECTURE.md) | Ownership, bounded memory and embedding obligations |
| [Action and receipt ABI](ACTION_ABI.md) | Byte offsets, time domains, replay and receipts |
| [Integration guide](docs/INTEGRATION.md) | Rust/C lifecycle, discovery, audit export and upgrades |
| [QEMU firmware](ports/qemu/README.md) | Executable embedded examples and stack measurements |
| [Capability model](CAPABILITY_MODEL.md) | Profiles, authority and execution classes |
| [Safety model](SAFETY_MODEL.md) | Failure behavior and guarantee boundaries |
| [State machine](RUNTIME_STATE_MACHINE.md) | Epochs, faults, e-stop and recovery |
| [Test plan](V0_TEST_PLAN.md) | Required cases and verification commands |
| [Research](RESEARCH.md) | Feasibility, overlaps and product direction |
| [Benchmarks](docs/BENCHMARKS.md) | Measured evidence and unmeasured targets |
| [Validation](docs/VALIDATION.md) | Passing tests, CI and C integration evidence |
| [Roadmap](ROADMAP.md) | Completed software scope and physical validation gates |

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The public API and ABI are versioned but still experimental. Contributions should
preserve deterministic semantics and static memory. See [CONTRIBUTING.md](CONTRIBUTING.md).

MIT license. Created by Ashutosh Rath.
