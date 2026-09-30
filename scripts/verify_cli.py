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
assert all(r["boot_id"] == 42 for r in demo)
assert demo[0]["principal"] == 7

trace = run("replay", "examples/demo.pxr")
for _ in range(20):
    assert trace == run("replay", "examples/demo.pxr")
final = json.loads(trace.splitlines()[-1])
assert final["final_state"] == "SafeIdle" and final["velocity"] == [0, 0]
assert final["driver_executions"] == 3

# Independent reference for the fixed action ABI v1 layout.
body = struct.pack("<4sBBHHH8QI2i", b"PXR0", 1, 0, 92, 1, 0,
                   42, 7, 1, 99, 99, 2, 0, 20, 100, 400, -200)
frame = body + struct.pack("<I", zlib.crc32(body))
assert len(frame) == 92
assert run("frame").strip().decode() == frame.hex()
vector = json.loads((ROOT / "tests/vectors/action-v1.json").read_text())
assert frame.hex() == vector["hex"]
assert json.loads(run("inspect",frame.hex()))["parameters"] == [400,-200]
receipt=bytes.fromhex(run("receipt-frame").strip().decode())
assert len(receipt)==140 and receipt[:4]==b"PXRR"
assert zlib.crc32(receipt[:136])==struct.unpack_from("<I",receipt,136)[0]
assert struct.unpack_from("<QQ",receipt,8)==(42,7)
assert struct.unpack_from("<Q",receipt,32)[0]==2
assert struct.unpack_from("<ii",receipt,112)==(400,-200)
assert json.loads(run("inspect",receipt.hex()))["decision"]=="Executed"
# Construct every receipt field independently, including reserved bytes and flags.
receipt_body = struct.pack("<4sBBHQQII9Q4iIIHH4B", b"PXRR",1,0,140,42,7,0,0,
                           2,99,99,1,2,0,0,0,0,400,-200,400,-200,0,0,1,0,0,0,1,1)
assert receipt == receipt_body + struct.pack("<I",zlib.crc32(receipt_body))
caps=[json.loads(line) for line in run("capabilities").splitlines()]
assert [c["id"] for c in caps]==[1,2,3,4]
audit=[json.loads(line) for line in run("replay","--audit","examples/demo.pxr").splitlines()]
assert [r["receipt_id"] for r in audit[:-1]]==list(range(1,len(audit)))
assert any(r.get("reason")=="StreamExpired" for r in audit)
assert all(r["status"] == "LEASE_GRANTED" for r in audit[:-1] if r["reason"] == "LeaseGranted")
assert all(r["status"] == "STREAM_EXPIRED" for r in audit[:-1] if r["reason"] == "StreamExpired")
assert run("replay","--audit","examples/demo.pxr") == run("replay","--audit","examples/demo.pxr")

for args in [("missing-command",), ("bench", "0"), ("replay", "missing-file.pxr")]:
    result = subprocess.run([str(binary), *args], capture_output=True, cwd=ROOT)
    assert result.returncode != 0 and b"pxr:" in result.stderr
print("CLI contract verified: scenarios, receipt flags, deterministic traces, golden frame, errors")
