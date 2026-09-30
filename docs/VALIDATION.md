# Release validation

Initial implementation commit: `e4eb8d93fa7ee8ceaea1964bc14c778485711dbd`.
[Public CI run](https://github.com/ashutosh-rath02/pxr/actions/runs/36738972423).

| Check | Result |
|---|---|
| 43 Rust tests, local debug and release | Passed |
| 43 Rust tests, Linux / Windows / macOS CI | Passed |
| Rust 1.85 minimum-version tests | Passed |
| Formatting and Clippy, warnings denied | Passed on all three CI hosts |
| Independent Python CLI / golden frame check | Passed locally and on all three CI hosts |
| C caller linked to Rust archive, Linux CI | Passed |
| C caller linked to Rust archive, Windows / Clang 23.1.2 | Passed locally |
| Cortex-M `no_std` C archive | Built locally and in CI |
| RISC-V `no_std` C archive | Built locally and in CI |
| Public demonstration and reproducible benchmark | Ran successfully |

The tests include 20,000 randomized fault transitions, 100 identical admission
traces, a bounded receipt ring, replay-window eviction, 736 single-bit frame
corruptions and 256,000 arbitrary byte/length combinations. The independent CLI
check compares repeated trace output and constructs the golden frame with Python's
`struct` and `zlib`, independently of the Rust encoder.

Windows C ABI measurement: context 9,616 bytes; C receipt 104 bytes. The context
includes the 9,584-byte Rust runtime and four callback/user pointers.
The local Windows C harness uses LLVM-MinGW's `libunwind.dll` on its toolchain PATH;
this dependency belongs to that C linker setup, not the public MSVC CLI package.

Public CLI binaries come from the passing GitHub Actions host builds. Embedded
archives come from the verified local Rust cross-builds of the same source. Release
bundles include licenses, contracts and SHA-256 checksums. The GitHub release is
marked experimental/prerelease. No hardware or authenticated network integration
was exercised; those are explicitly outside the revised v0 scope.
