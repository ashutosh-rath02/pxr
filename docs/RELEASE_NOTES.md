# PXR v0.2.0 — embedded execution SDK

PXR implements the revised PRD's generic v0 execution boundary: bounded actions,
temporary authority, time/state validation, replay protection, local fallback,
driver observation checks and execution receipts. This release completes the
embedding SDK and adds executed embedded evidence.

## Changes

- C APIs now encode/decode actions, configure runtime limits, enumerate capabilities,
  read controller snapshots and export portable receipts.
- A 140-byte receipt format preserves boot ID, authenticated principal, safety flags,
  action outcome and observation context. The 92-byte action format is unchanged.
- `pxr capabilities`, `pxr inspect`, `pxr receipt-frame` and `replay --audit FILE`
  support integration and complete trace export.
- Freestanding Cortex-M3 and RISC-V firmware runs under QEMU in CI, with code,
  static memory and stack measurements.
- E-stop now fences its timestamp so older sensor updates cannot authorize recovery.
  Applications on 0.1 should upgrade and use a single monotonic clock.
- Rust/C examples, integration guidance and CI-verified SDK bundles are included.

47 Rust tests pass on Linux, Windows and macOS, including the Rust 1.85 minimum.
Independent Python codec checks, C linking, embedded cross-builds and both QEMU
firmware checks pass. See [validation](https://github.com/ashutosh-rath02/pxr/blob/v0.2.0/docs/VALIDATION.md).

## Install

Download the matching CLI/SDK bundle, verify `SHA256SUMS`, extract, then run
`pxr demo` (`pxr.exe demo` on Windows). Host bundles include a C static library
under `lib/`. The Windows library uses MSVC; match the linker/toolchain when embedding.
Embedded archive bundles target Cortex-M4F and RISC-V. The QEMU bundle contains
executable Cortex-M3/RISC-V firmware and its measured evidence.

```sh
cargo install --git https://github.com/ashutosh-rath02/pxr --tag v0.2.0 --locked pxr-runtime-sim
pxr demo
```

Existing action frames and C ABI 1 signatures/layouts remain compatible.
Re-query context size after upgrading: receipt context adds storage.
Rust `Receipt` has new fields; CLI JSON adds the same metadata.
The [integration guide](https://github.com/ashutosh-rath02/pxr/blob/v0.2.0/docs/INTEGRATION.md)
covers migration.

MIT licensed, with no external Rust dependencies. This is an experimental public
SDK, validated in simulation and emulation. Physical safety, real-board timing,
authenticated transport and independent hardware protection require platform
integration and measurement.
