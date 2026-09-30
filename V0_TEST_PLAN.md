# V0 test and release plan

## Required scenarios

| PRD scenario | Automated evidence |
|---|---|
| Valid capability, authority, parameters, state, time | Verified motor/gripper execution; CLI demo |
| Bound violation | Static and narrowed-lease bounds; no dispatch |
| Stale action | Absolute deadline, receive TTL, exact equality |
| Missing/expired lease | Admission rejection; expiration before renewal |
| Duplicate/replay | Discrete dispatch once; mutated payload; replay eviction; sequence order |
| State precondition | Local predicate rejection; epoch mismatch; active unsafe state fallback |
| Controller disappears | Stream expiry, lease expiry, watchdog service gap |

Additional coverage: lease-owner spoofing, capability/resource scope, e-stop latch
and recovery, stale sensors, invalid storms, clock rollback, boot-session mismatch,
driver failure, verification failure, fallback failure, startup configuration,
overflow, bounded receipts, reproducible traces, and randomized failure sequences.

Codec tests cover a reference CRC, exact lengths, reserved fields, all 736 single-bit
frame mutations, round trips, and 256,000 arbitrary byte/length combinations.
The C embedding harness links a real C caller with the Rust archive and exercises
execution, replay rejection, corrupt frames, stream fallback and local recovery.

## Commands

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
cargo run --release -p pxr-runtime-sim -- demo
cargo run --release -p pxr-runtime-sim -- replay examples/demo.pxr
cargo run --release -p pxr-runtime-sim -- bench 20000
rustup target add thumbv7em-none-eabihf riscv32imc-unknown-none-elf
cargo build --release -p pxr-runtime-c-api --no-default-features --target thumbv7em-none-eabihf
cargo build --release -p pxr-runtime-c-api --no-default-features --target riscv32imc-unknown-none-elf
```

## Evidence interpretation

Publish platform/toolchain details and raw benchmark JSON. Report mean, p50, p99,
maximum observed latency, a direct driver baseline, duplicate rejection cost,
fixed runtime size, receipt size/capacity, frame size and virtual watchdog timing.
Throughput is measured over a batch without per-action timer calls; it remains a
host simulation rate. Maximum observed latency is not WCET. The benchmark also
reports codec cost and the last-capability lookup with all 16 slots populated.
The full path includes policy, replay, driver observations and receipt insertion.
Receipt storage overhead is measured as size × capacity; isolated insertion
latency is not separated from the admission measurement.

Cross-compiled static archives establish that the core needs neither std nor a
specific CPU. Archive file size contains object/symbol metadata and is not firmware
flash usage. Board RAM, stack high-water, linked flash size, ISR interference,
physical stopping distance, and physical verification are not measured in this v0.

Release only after tests, CLI demonstrations, C integration, and cross-builds pass.
Publish as an experimental prerelease with evidence and limitations included.
