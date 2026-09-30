# PXR v0.1.0 — experimental execution runtime

The initial public release implements the revised PRD's generic v0 scope: a small,
allocation-free Rust runtime that admits bounded motor/gripper actions, supervises
local validity and authority, verifies driver observations, and records receipts.

Includes a `no_std` core, 92-byte codec, caller-allocated C ABI, simulated actuator
and sensor state, deterministic trace runner, checked demos, conformance tests,
embedded cross-builds and reproducible host benchmarks. No external Rust crates
are required.

Start with the README, run `pxr demo`, then inspect ACTION_ABI.md and SAFETY_MODEL.md.
Source is available under the MIT license. Public API/ABI v1 is experimental.

This release establishes software behavior in simulation and portability at
compile time. It does not establish physical safety, board timing, secure network
operation, durable exactly-once effects or hardware attestation. Platform-specific
drivers, authenticated ingress and independent hardware protection are required
before a physical pilot. No robot/framework/cloud integrations are included.
