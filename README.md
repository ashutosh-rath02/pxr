# PXR — Physical Execution Runtime

An open, embedded-first execution boundary between AI-generated actions and physical
actuators. Rust `no_std` core, fixed memory, transport-independent semantics, C ABI.

**v0.1 is an experimental simulator release.** It demonstrates command admission and
failure behavior. Physical safety, MCU timing, authenticated networking and hardware
attestation are not established by this release. See [the safety model](SAFETY_MODEL.md).

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
cargo install --git https://github.com/ashutosh-rath02/pxr --tag v0.1.0 pxr-runtime-sim
pxr demo
```

Prebuilt Linux, Windows and macOS CLI packages are on the
[releases page](https://github.com/ashutosh-rath02/pxr/releases). Verify the included
SHA-256 checksums, extract the matching OS/architecture package, and run `pxr demo`.
On Windows use `pxr.exe demo`. On macOS, unsigned downloads may require local
Gatekeeper approval; building from source is also supported.

The demo checks nine scenarios and emits JSON receipts: valid execution, bounds,
staleness, missing authority, duplicates, preconditions, watchdog, epoch mismatch,
and failed verification. Its exit code is nonzero if any expected result differs.
`replay` uses an explicit virtual clock, so repeated traces produce identical output.

## What ships

- Four simulated capabilities: `motor.drive`, `motor.stop`, `gripper.open`, `gripper.close`.
- Static capability registration with integer parameter bounds and state predicates.
- Exclusive resource leases with owner binding, narrower limits and replay-safe renewal.
- Controller-domain deadlines, receive-relative TTL, and independent stream expiry.
- Sequence fencing, bounded duplicate suppression, and boot-session checks.
- Latched e-stop, local recovery, sensor freshness and watchdog supervision.
- Driver feedback verification and fixed receipt history, including uncertain outcomes.
- A 92-byte binary codec, C header, and working C embedding example.
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

The same crate builds for `riscv32imc-unknown-none-elf`. These are compile targets,
not complete STM32/ESP firmware ports. Bring a clock, trusted sensors, driver,
fallback policy, supervisor scheduling and an independent hardware watchdog.

## Contracts and evidence

| Document | Read it for |
|---|---|
| [Architecture](ARCHITECTURE.md) | Ownership, bounded memory and embedding obligations |
| [Action ABI](ACTION_ABI.md) | Byte offsets, time domains, replay and receipts |
| [Capability model](CAPABILITY_MODEL.md) | Profiles, authority and execution classes |
| [Safety model](SAFETY_MODEL.md) | Failure behavior and guarantee boundaries |
| [State machine](RUNTIME_STATE_MACHINE.md) | Epochs, faults, e-stop and recovery |
| [Test plan](V0_TEST_PLAN.md) | Required cases and verification commands |
| [Research](RESEARCH.md) | Feasibility, overlaps and product direction |
| [Benchmarks](docs/BENCHMARKS.md) | Measured evidence and unmeasured targets |
| [Validation](docs/VALIDATION.md) | Passing tests, CI and C integration evidence |
| [Roadmap](ROADMAP.md) | Work needed beyond the v0 simulation release |

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The public API and ABI are versioned but still experimental. Contributions should
preserve deterministic semantics and static memory. See [CONTRIBUTING.md](CONTRIBUTING.md).

MIT license. Created by Ashutosh Rath.
