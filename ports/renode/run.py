"""Run the QEMU test and timing images on Renode's STM32F4 Discovery board model.

The same firmware.c/timing.c sources are linked for an STM32F407 memory map and print
through USART2 instead of semihosting. Renode models the board's peripherals, NVIC and
SysTick; it does not model flash wait states, caches or cycle-accurate pipelines.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "target/renode"
TARGET = "thumbv7em-none-eabi"
cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo.exe")
clang = os.environ.get("CLANG", "clang")
renode = os.environ.get("RENODE", "renode")


def link(source, name):
    elf = OUT / (name + ".elf")
    subprocess.run([clang, "--target=arm-none-eabi", "-mcpu=cortex-m4", "-mthumb", "-mfloat-abi=soft",
        "-Oz", "-g", "-ffreestanding", "-fno-builtin", "-fdata-sections", "-ffunction-sections",
        "-nostdlib", "-fuse-ld=lld", "-Wall", "-Wextra", "-Werror", "-DPXR_UART_BASE=0x40004400u",
        "-Iinclude", "-Iports/qemu", source, "ports/qemu/platform.c", "ports/qemu/cortex-m.S",
        f"target/{TARGET}/release/libpxr_runtime_c_api.a", "-Wl,-T,ports/renode/stm32f4.ld",
        "-Wl,--gc-sections", "-o", str(elf)], cwd=ROOT, check=True)
    return elf


def emulate(elf, seconds):
    # The Renode monitor cannot tokenize paths containing spaces, so stage files in temp.
    with tempfile.TemporaryDirectory() as tmp:
        if " " in tmp:
            raise SystemExit(f"Renode needs a temp directory without spaces; set TMP (got {tmp})")
        staged = Path(tmp) / elf.name
        shutil.copyfile(elf, staged)
        log = Path(tmp) / "uart.txt"
        script = Path(tmp) / "run.resc"
        script.write_text("\n".join([
            "mach create \"stm32f4\"",
            "machine LoadPlatformDescription @platforms/boards/stm32f4_discovery-kit.repl",
            f"sysbus LoadELF @{staged.as_posix()}",
            "cpu VectorTableOffset 0x08000000",
            f"usart2 CreateFileBackend @{log.as_posix()} true",
            f"emulation RunFor \"{seconds}\"",
        ]) + "\n")
        result = subprocess.run([renode, "--console", "--disable-gui", "-e",
                                 f"include @{script.as_posix()}; quit"],
                                capture_output=True, text=True, timeout=900)
        text = log.read_text(errors="replace") if log.exists() else ""
        if not text:
            print(result.stdout[-3000:])
        return text


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    subprocess.run([cargo, "build", "--locked", "--release", "-p", "pxr-runtime-c-api",
                    "--no-default-features", "--target", TARGET], cwd=ROOT, check=True)
    record = {"target": TARGET, "board": "Renode stm32f4_discovery-kit (STM32F407, Cortex-M4)",
              "hardware_validated": False}
    text = emulate(link("ports/qemu/firmware.c", "firmware"), 2)
    print(text)
    if "PXR_QEMU_PASS" not in text or "PXR_EXIT code=0" not in text:
        raise SystemExit("Renode firmware check failed")
    passed = next(line for line in text.splitlines() if line.startswith("PXR_QEMU_PASS"))
    for key, value in re.findall(r"(\w+)=(\d+)", passed):
        record[key] = int(value)
    text = emulate(link("ports/qemu/timing.c", "timing"), 5)
    print(text)
    if "PXR_TIMING_DONE" not in text:
        raise SystemExit("Renode timing run failed")
    ops = {n: (int(lo), int(hi)) for n, lo, hi in re.findall(r"op=(\w+) min_ticks=(\d+) max_ticks=(\d+)", text)}
    empty = ops.pop("empty")[0]
    calibration = ops.pop("calibration_200000_insns")[0] - empty
    if calibration <= 0:
        raise SystemExit("Renode SysTick did not advance; timing unsupported")
    scale = 200000 / calibration
    record["timing_instructions"] = {n: {"min": round((lo - empty) * scale), "max": round((hi - empty) * scale)}
                                     for n, (lo, hi) in ops.items()}
    record["timing_method"] = ("Renode SysTick ticks scaled by a 200,000-instruction calibration loop; "
                               "virtual time, not cycles")
    record["emulator"] = subprocess.run([renode, "--version"], capture_output=True, text=True).stdout.strip()
    (OUT / "measurements.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
