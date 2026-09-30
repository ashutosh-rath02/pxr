# Release validation

v0.2 runtime implementation: `63f7fcef68db111826193fc9e452ebaed25aab35`.
[Passing CI and firmware execution](https://github.com/ashutosh-rath02/pxr/actions/runs/36741849281).
Release bundles also carry the exact final source revision and CI run in `BUILD.json`.

| Check | Result |
|---|---|
| 47 Rust tests, local debug and release | Passed |
| 47 Rust tests, Linux / Windows / macOS CI | Passed |
| Rust 1.85 minimum-version tests | Passed |
| Formatting and Clippy, warnings denied | Passed on all three CI hosts |
| Independent Python action/receipt codec and CLI checks | Passed locally and on all three CI hosts |
| C caller linked to Rust archive, Linux CI | Passed |
| C caller linked to Rust archive, Windows / Clang 23.1.2 | Passed locally |
| Cortex-M4F and RISC-V `no_std` C archives | Built locally and in CI |
| Cortex-M3 freestanding C/Rust firmware | Executed under QEMU |
| RISC-V freestanding C/Rust firmware | Executed under QEMU |

Tests include 20,000 randomized fault transitions, 100 identical admission traces,
replay-window eviction, a bounded receipt ring, 736 single-bit action corruptions,
1,120 single-bit receipt corruptions and 256,000 arbitrary action byte/length
combinations. Python independently checks CRCs, offsets and repeatable audit output.

The e-stop regression tests prove that older sensor updates and recovery ticks
cannot undo a newer e-stop. An e-stop with an older tick still attempts fallback
and preserves the latest clock fence.

The C harness checks typed encoding against an independently assembled frame,
decoding, configuration, capability discovery, snapshots and receipt export.
Windows C measurements: context 11,152 bytes; legacy C receipt 104 bytes.
The runtime contains 11,120 bytes including a 64-entry ring of 128-byte Rust receipts.

The QEMU harness runs without an OS or allocator. It checks valid dispatch,
duplicate/bounds/corruption rejection, stream fallback, gripper execution, e-stop
and recovery ordering, snapshot and receipt export. Stack painting reports the
maximum observed use on these paths. Raw [firmware measurements](evidence/qemu.json)
and [reproduction instructions](../ports/qemu/README.md) are included.

Every public CLI and static library comes from an exact successful CI revision.
The packager checks each artifact's source commit, run ID, version and SHA-256
before creating bundles. `BUILD.json` records toolchain information and hashes of
packaged files; `SHA256SUMS` covers the archives. These provide traceability and
corruption checks, not cryptographic attestation of physical execution.

The release is experimental. Physical controllers, authenticated ingress, RTOS
scheduling and physical stop timing remain device-specific validation work.
