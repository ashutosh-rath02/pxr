"""Build and run the controlled driver/guard/PXR comparison. Python standard library only."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--output", type=Path, default=ROOT / "docs/evidence/comparison.json")
    args = parser.parse_args()
    if not 1 <= args.runs <= 20:
        parser.error("--runs must be between 1 and 20")
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo.exe")
    rustc = shutil.which("rustc") or str(Path.home() / ".cargo/bin/rustc.exe")
    subprocess.run([cargo, "build", "--locked", "--release", "-p", "pxr-runtime-sim", "--example", "compare"], cwd=ROOT, check=True)
    binary = ROOT / "target/release/examples" / ("compare.exe" if platform.system() == "Windows" else "compare")
    paths = ["Cargo.toml", "Cargo.lock", "crates/runtime-sim/examples/compare.rs", "scripts/compare.py"]
    for directory in ("crates/runtime-core/src", "crates/runtime-codec/src", "crates/runtime-sim/src"):
        paths.append((Path(directory).parent / "Cargo.toml").as_posix())
        paths.extend(p.relative_to(ROOT).as_posix() for p in (ROOT / directory).rglob("*.rs"))
    report = {
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "version": tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"],
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "benchmark_source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain", "--", *paths], cwd=ROOT)),
        "source_sha256": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in sorted(paths)},
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "toolchain": subprocess.check_output([rustc, "-vV"], text=True).strip(),
        "platform": platform.platform(),
        "processor": platform.processor(),
        "method": "200 rotating-order batches of 1024 valid motor actions per path after 8 warmup batches. Identical non-inlined driver callbacks. Inputs and frames prepared outside timing. Virtual controller time fixed at zero. Every execution and final output checked.",
        "scope": "Batch averages on one host; not individual-action latency, WCET, real-time control throughput, or an external-product ranking. Small guard has fewer guarantees. Frame path includes decode/CRC but excludes encoding and transport.",
        "runs": [json.loads(subprocess.check_output([str(binary)], cwd=ROOT, text=True)) for _ in range(args.runs)],
    }
    for run in report["runs"]:
        assert len(run["cases"]) == 4
        for case in run["cases"]:
            assert case["verified_executions"] == (run["measured_batches"] + run["warmup_batches"]) * run["batch_size"]
            assert len(case["batch_samples_ns_per_action"]) == run["measured_batches"]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for name in ("direct_driver", "small_guard", "pxr_typed", "pxr_decoded_frame"):
        medians = [next(c["batch_median_ns_per_action"] for c in run["cases"] if c["name"] == name) for run in report["runs"]]
        print(f"{name}: median batch cost {min(medians):.1f} to {max(medians):.1f} ns/action across {args.runs} runs")
    print(f"Raw results: {args.output}")


if __name__ == "__main__":
    main()
