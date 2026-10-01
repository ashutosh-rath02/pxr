"""Build freestanding C/Rust firmware and run it on two QEMU instruction sets."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser()
parser.add_argument("--build-only",action="store_true")
args=parser.parse_args()
out=ROOT/"target/qemu"
out.mkdir(parents=True,exist_ok=True)
cargo=shutil.which("cargo") or str(Path.home()/".cargo/bin/cargo.exe")
clang=os.environ.get("CLANG","clang")
size=os.environ.get("LLVM_SIZE","llvm-size")
report=[]
for target,arch,flags,emulator,machine in [
    ("thumbv7m-none-eabi","cortex-m",["--target=arm-none-eabi","-mcpu=cortex-m3","-mthumb"],"qemu-system-arm","lm3s6965evb"),
    ("riscv32imc-unknown-none-elf","riscv32",["--target=riscv32-none-elf","-march=rv32imc_zicsr","-mabi=ilp32","-mno-relax"],"qemu-system-riscv32","virt")]:
    subprocess.run([cargo,"build","--locked","--release","-p","pxr-runtime-c-api","--no-default-features","--target",target],cwd=ROOT,check=True)
    def link(source,name):
        elf=out/(name+".elf")
        command=[clang,*flags,"-Oz","-g","-ffreestanding","-fno-builtin","-fdata-sections","-ffunction-sections",
            "-nostdlib","-fuse-ld=lld","-Wall","-Wextra","-Werror","-Iinclude",source,"ports/qemu/platform.c",
            f"ports/qemu/{arch}.S",f"target/{target}/release/libpxr_runtime_c_api.a",f"-Wl,-T,ports/qemu/{arch}.ld",
            "-Wl,--gc-sections",f"-Wl,-Map,{out/(name+'.map')}","-o",str(elf)]
        subprocess.run(command,cwd=ROOT,check=True)
        return elf,command
    def emulate(elf,extra=()):
        command=[emulator,"-M",machine,"-nographic","-semihosting-config","enable=on,target=native",*extra,"-kernel",str(elf)]
        if arch=="riscv32": command.extend(["-bios","none"])
        result=subprocess.run(command,capture_output=True,text=True,timeout=120)
        return result,result.stdout+result.stderr
    elf,command=link("ports/qemu/firmware.c",arch)
    timing_elf,_=link("ports/qemu/timing.c",arch+"-timing")
    sizes=subprocess.check_output([size,str(elf)],text=True)
    print(sizes)
    fields=sizes.splitlines()[-1].split()
    record={"target":target,"text_bytes":int(fields[0]),"data_bytes":int(fields[1]),"bss_bytes":int(fields[2]),
            "elf_bytes":elf.stat().st_size,"hardware_validated":False,"machine":machine,
            "compiler":subprocess.check_output([clang,"--version"],text=True).splitlines()[0],
            "build_command":command}
    if not args.build_only:
        result,text=emulate(elf)
        print(text)
        if result.returncode!=0 or "PXR_QEMU_PASS" not in text: raise SystemExit("Firmware check failed")
        for key,value in re.findall(r"(\w+)=(\d+)",text): record[key]=int(value)
        # One virtual nanosecond per instruction; ticks are converted using a calibration loop.
        result,text=emulate(timing_elf,["-icount","shift=0"])
        print(text)
        if result.returncode!=0 or "PXR_TIMING_DONE" not in text: raise SystemExit("Timing run failed")
        ops={name:(int(lo),int(hi)) for name,lo,hi in re.findall(r"op=(\w+) min_ticks=(\d+) max_ticks=(\d+)",text)}
        empty=ops.pop("empty")[0]
        calibration=ops.pop("calibration_200000_insns")[0]-empty
        if calibration<=0: raise SystemExit(f"{emulator} timer did not advance under -icount; timing unsupported")
        per_tick=200000/calibration
        record["timing_instructions"]={name:{"min":round((lo-empty)*per_tick),"max":round((hi-empty)*per_tick)}
            for name,(lo,hi) in ops.items()}
        record["timing_method"]=("QEMU -icount shift=0 retired instructions, calibrated against a "
            "200,000-instruction loop; worst-case inputs (full replay history and receipt ring); not cycles")
        record["emulator"]=subprocess.check_output([emulator,"--version"],text=True).splitlines()[0]
    report.append(record)
(out/"measurements.json").write_text(json.dumps(report,indent=2)+"\n")
