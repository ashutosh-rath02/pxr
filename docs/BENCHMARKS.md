# v0.1 measurements

Measured 30 September 2026 on Windows x86-64, Intel Core i7-1260P, Rust 1.98.1.
Release profile: LTO, one codegen unit, panic abort. Five independent runs of
20,000 samples each. Raw data: [host-benchmark.json](evidence/host-benchmark.json).

| Metric | Observed result |
|---|---:|
| Fixed Rust runtime storage | 9,584 bytes |
| One Rust receipt | 104 bytes |
| Receipt ring payload (64 × 104) | 6,656 bytes, included in runtime storage |
| Replay window | 32 action IDs, plus per-lease sequence marks |
| Action frame | 92 bytes |
| Full admission/driver/receipt p99 | 200–300 ns across five host runs |
| Maximum observed full-path sample | 83,000 ns (83 µs) across all runs |
| Codec encode + decode mean | 1,280–1,742 ns |
| Batch action throughput | 8.2–18.7 million/s, virtual-clock simulation |
| Simulated stream stop | Tick 100 ms for a 100 ms TTL, supervisor every 10 ms |
| Windows CLI executable | 445,634 bytes, includes host CLI/standard library |

The JSON includes the direct-driver baseline, duplicate-rejection cost,
full-capacity lookup timings and all individual runs. Variance and outliers are
retained. Timing uses host `Instant`; timer overhead and scheduling interference
are included. The controller's virtual clock remains fixed during latency loops,
so these throughput numbers do not describe a real sensor/control-loop workload.

The core admission timing excludes binary codec work. It includes synchronous
simulated driver observations and verification. The direct baseline is the same
simulated observation/execute/observation sequence without policy, authority,
replay, supervision or receipts. Small timing differences near timer resolution
should not be treated as precise overhead estimates.

The largest observed sample is **not** a proven worst-case execution time.
The 16-slot lookup sample explores a fuller table; it does not enumerate all
possible hardware interrupt, cache or driver behavior. Per-receipt insertion
time is included in full-path timing, not separately isolated.

## Reproduce

```sh
cargo build --workspace --release
python3 scripts/measure.py target/release/pxr
```

Windows: use `python scripts/measure.py target/release/pxr.exe`.
This overwrites the local evidence files with results from the current machine.
The CLI's `bench` command alone prints JSON without modifying files.

## Embedded evidence

Core, codec and the C ABI static archive compile without `std` for:

- `thumbv7em-none-eabihf` (Cortex-M with hardware floating-point ABI).
- `riscv32imc-unknown-none-elf` (32-bit RISC-V IMC).

The source is unchanged between targets. No physical board is exercised.
Static runtime size excludes call stack and platform state. The C context also
stores callbacks; `pxr_context_size()` is the authoritative allocation requirement.
An archive's file size is not linked flash usage. Actual MCU flash, stack high-water,
RAM placement, WCET, CPU load and physical fallback timing remain unmeasured.
