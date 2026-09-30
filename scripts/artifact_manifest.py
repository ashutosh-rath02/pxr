"""Bind every release binary/archive to its CI source revision and toolchain."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tomllib

root=Path(__file__).resolve().parents[1]
kind=sys.argv[1]
if kind=="host":
    files=[root/"target/release"/name for name in ("pxr","pxr.exe","libpxr_runtime_c_api.a","pxr_runtime_c_api.lib")]
    files=[p for p in files if p.is_file()]+[root/"benchmark.json"]
elif kind=="emulated-firmware":
    files=sorted(p for p in (root/"target/qemu").glob("*") if p.is_file())
else:
    files=[root/"target"/kind/"release/libpxr_runtime_c_api.a"]
manifest={"version":tomllib.loads((root/"Cargo.toml").read_text())["workspace"]["package"]["version"],
    "commit":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),
    "run_id":os.environ.get("GITHUB_RUN_ID"),"kind":kind,"os":platform.system(),"architecture":platform.machine(),
    "rustc":subprocess.check_output(["rustc","-vV"],text=True).strip(),
    "files":{p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in files}}
(root/"artifact-manifest.json").write_text(json.dumps(manifest,indent=2)+"\n")
