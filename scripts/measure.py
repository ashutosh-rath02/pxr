"""Capture reproducible host measurements; no third-party Python packages."""
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1] if len(sys.argv) > 1 else root / "target/release/pxr").resolve()
rustc = shutil.which("rustc") or str(Path.home() / ".cargo/bin/rustc.exe")
output = root / "docs/evidence"
output.mkdir(parents=True, exist_ok=True)
report = {
    "date": "2026-09-30",
    "toolchain": subprocess.check_output([rustc, "--version"], text=True).strip(),
    "platform": platform.platform(),
    "processor": platform.processor(),
    "binary_bytes": binary.stat().st_size,
    "command": "pxr bench 20000",
    "profile": "release, LTO, one codegen unit, panic=abort",
    "clock_note": "Virtual controller tick is held at zero during throughput/latency loops; host Instant measures elapsed time. Timer overhead and scheduling noise are included.",
    "samples": [json.loads(subprocess.check_output([str(binary), "bench", "20000"], text=True)) for _ in range(5)],
}
(output / "host-benchmark.json").write_text(json.dumps(report, indent=2) + "\n")
(output / "demo.jsonl").write_bytes(subprocess.check_output([str(binary), "demo"]))
(output / "replay.jsonl").write_bytes(subprocess.check_output([str(binary), "replay", "examples/demo.pxr"], cwd=root))
print(json.dumps({"toolchain": report["toolchain"], "binary_bytes": report["binary_bytes"],
    "p99_ns": [s["full_p99_ns"] for s in report["samples"]],
    "max_ns": [s["full_max_observed_ns"] for s in report["samples"]],
    "codec_mean_ns": [s["codec_mean_ns"] for s in report["samples"]],
    "throughput": [s["measured_batch_actions_per_second"] for s in report["samples"]]}))
