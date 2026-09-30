# Executable embedded validation

This harness links the production `no_std` C archive with a freestanding C program
and boots it on two emulated instruction sets. No host Rust standard library,
allocator, operating system or external C library is linked into the firmware.

- [QEMU LM3S6965EVB](https://www.qemu.org/docs/master/system/arm/stellaris.html):
  Cortex-M3, 256 KB emulated flash, 64 KB emulated SRAM.
- [QEMU RISC-V virt](https://www.qemu.org/docs/master/system/riscv/virt.html):
  a generic virtual platform; this harness limits its linked image to 128 KB RAM.

```sh
sudo apt-get install clang lld llvm qemu-system-arm qemu-system-misc
rustup target add thumbv7m-none-eabi riscv32imc-unknown-none-elf
python3 ports/qemu/build.py
```

Each image must report `PXR_QEMU_PASS` and exit successfully within 30 seconds.
The test covers typed C encoding, dispatch/readback, duplicate and bounds rejection,
corrupt frames, stream expiry, gripper execution, e-stop clock fencing, local
recovery, snapshot access and portable receipt export. Emulator stderr/stdout is
checked and the measured results are saved in `target/qemu/measurements.json`.

The reset code fills a reserved 32 KB stack with a known pattern. The program scans
it after the test to estimate stack high-water and requires at least 1 KB unused.
This measures the exercised paths, not every possible application call path or
interrupt nesting. C context storage reserves 16 KB and is included in BSS.
`llvm-size` reports linked text, data and BSS; ELF file size includes debug metadata.

The driver reads/writes simulated variables. CPU emulation verifies that target
instructions execute and the C/Rust ABI works without a host OS. It does not
validate electrical peripherals, motors, real-time deadlines, or physical safety.
Semihosting is a test-only host interface and must not be enabled on an untrusted
firmware image. The runtime itself has no semihosting dependency.
