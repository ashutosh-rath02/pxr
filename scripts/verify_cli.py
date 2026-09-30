"""Independent public-contract checks. Uses only the Python standard library."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib

ROOT = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/release/pxr").resolve()


def run(*args):
    return subprocess.check_output([str(binary), *args], cwd=ROOT)


demo = [json.loads(line) for line in run("demo").splitlines()]
assert [r["status"] for r in demo] == [
    "EXECUTED", "REJECTED_BOUND", "REJECTED_STALE", "REJECTED_AUTHORITY",
    "REJECTED_DUPLICATE", "REJECTED_PRECONDITION", "WATCHDOG_TRIGGERED",
    "REJECTED_EPOCH", "VERIFICATION_FAILED",
]
assert demo[0]["observed_valid"] and demo[0]["dispatched"]
assert not demo[1]["dispatched"] and not demo[1]["observed_valid"]
assert demo[-1]["decision"] == "Failed" and demo[-1]["dispatched"]

trace = run("replay", "examples/demo.pxr")
for _ in range(20):
    assert trace == run("replay", "examples/demo.pxr")
final = json.loads(trace.splitlines()[-1])
assert final["final_state"] == "SafeIdle" and final["velocity"] == [0, 0]
assert final["driver_executions"] == 3

# Built independently from ACTION_ABI.md, not the Rust codec.
body = struct.pack("<4sBBHHH8QI2i", b"PXR0", 1, 0, 92, 1, 0,
                   42, 7, 1, 99, 99, 2, 0, 20, 100, 400, -200)
frame = body + struct.pack("<I", zlib.crc32(body))
assert len(frame) == 92
assert run("frame").strip().decode() == frame.hex()
vector = json.loads((ROOT / "tests/vectors/action-v1.json").read_text())
assert frame.hex() == vector["hex"]

for args in [("missing-command",), ("bench", "0"), ("replay", "missing-file.pxr")]:
    result = subprocess.run([str(binary), *args], capture_output=True, cwd=ROOT)
    assert result.returncode != 0 and b"pxr:" in result.stderr
print("CLI contract verified: scenarios, receipt flags, deterministic traces, golden frame, errors")
