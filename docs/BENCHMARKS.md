# v0.2 measurements

Measured 30 September 2026 on Windows x86-64, Intel Core i7-1260P, Rust 1.98.1.
Release profile: LTO, one codegen unit, panic abort. Five independent runs of
20,000 samples each. Raw data: [host-benchmark.json](evidence/host-benchmark.json).

| Metric | Observed result |
|---|---:|
| Fixed Rust runtime storage | 11,120 bytes |
| One Rust receipt | 128 bytes |
| Receipt ring payload (64 × 128) | 8,192 bytes, included in runtime storage |
| C context, 64-bit host | 11,152 bytes |
| Replay window | 32 action IDs, plus per-lease sequence marks |
| Action / portable receipt frame | 92 / 140 bytes |
| Full admission/driver/receipt p99 | 100 ns in each of five host runs |
| Maximum observed full-path sample | 35,700 ns (35.7 µs) across all runs |
| Codec encode + decode mean | 859–903 ns |
| Batch action throughput | 20.2–22.3 million/s, virtual-clock simulation |
| Simulated stream stop | Tick 100 ms for a 100 ms TTL, supervisor every 10 ms |
| Windows GNU CLI executable | 454,867 bytes, includes host CLI/standard library |

The JSON includes the direct-driver baseline, duplicate-rejection cost,
full-capacity lookup timings and all individual runs. Variance and outliers are
retained. Timing uses host `Instant`; timer overhead and scheduling interference
are included. Samples near timer resolution are quantized. The controller's
virtual clock remains fixed during latency loops, so these throughput numbers
do not describe a real sensor/control-loop workload.

Core admission timing excludes binary codec work. It includes synchronous
simulated driver observations and verification. The direct baseline performs the
same observation/execute/observation sequence without the runtime. Receipt
insertion cost is included in full-path timing and is not separately isolated.
These results should not be used to infer a speedup over a different host session.

The largest observed sample is **not a proven worst-case execution time**.
The 16-slot lookup sample explores a fuller table; it does not enumerate hardware
interrupt, cache or driver behavior.

## Emulated embedded execution

[CI run 36741849281](https://github.com/ashutosh-rath02/pxr/actions/runs/36741849281)
links freestanding C/Rust firmware and executes it under QEMU 8.2.2.
Raw data: [qemu.json](evidence/qemu.json).

| Target | Linked code | Data | BSS | C context | Observed stack use |
|---|---:|---:|---:|---:|---:|
| Cortex-M3, `thumbv7m-none-eabi` | 21,920 B | 0 B | 16,404 B | 11,128 B | 2,228 B |
| RISC-V, `riscv32imc-unknown-none-elf` | 19,296 B | 0 B | 16,404 B | 11,128 B | 2,148 B |

Code includes the test harness and semihosting support. BSS includes a generously
reserved 16 KiB context buffer; it is not the minimum runtime allocation.
The linker reserves another 32 KiB for stack, separately from BSS. Stack use is
measured by painting this region and scanning after the test sequence; it covers
only the executed paths and excludes real interrupts and peripheral drivers.
ELF file sizes include debug/symbol data and do not equal flash usage.

Cortex-M4F (`thumbv7em-none-eabihf`) also cross-builds as a static C archive.
No physical board is exercised. Board WCET, ISR interference, CPU load and
physical fallback timing remain unmeasured.

## Reproduce

```sh
cargo build --workspace --release
python3 scripts/measure.py target/release/pxr
```

Windows: use `python scripts/measure.py target/release/pxr.exe`.
This overwrites local host evidence; `pxr bench 20000` only prints JSON.
For firmware builds, QEMU commands and target setup, see
[ports/qemu](../ports/qemu/README.md). Public firmware bundles include ELF files,
linker maps, measurements and source/CI provenance.
