# Changelog

## 0.2.0 — 2026-09-30

- Fix e-stop clock ordering: older state updates cannot authorize recovery after a newer stop.
- Add typed C action encoding/decoding, runtime configuration, snapshots and capability discovery.
- Add boot/principal/safety context to receipts and a validated 140-byte portable receipt codec.
- Add complete CLI audit export, frame inspection and capability listing.
- Execute freestanding Cortex-M3 and RISC-V firmware under QEMU; measure code, RAM sections and stack use.
- Include host C libraries, embedded archives, firmware evidence and verified CI provenance in public bundles.
- Add a runnable Rust example and integration/migration guide; retain action wire format and existing C ABI 1 layouts.

## 0.1.0 — 2026-09-30

Initial experimental release implementing the revised generic v0 PRD.

- Rust allocation-free `no_std` runtime and fixed action codec.
- Static capabilities, exclusive leases, state/epoch predicates and temporal limits.
- Bounded replay protection, independent stream expiry and deterministic fallbacks.
- Driver observation checks with explicit rejection, execution and failure receipts.
- C ABI and linked C example; Cortex-M and RISC-V cross-build support.
- Simulator, checked failure demos, deterministic trace runner and host benchmarks.
- Architecture contracts, conformance tests and feasibility research.
