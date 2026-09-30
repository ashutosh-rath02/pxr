"""Package verified CI binaries and embedded archives. Run from a clean release checkout."""
import hashlib
import io
import json
import argparse
from pathlib import Path
import subprocess
import tarfile
import zipfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"
DIST.mkdir(exist_ok=True)
parser = argparse.ArgumentParser()
parser.add_argument("--ci-run", required=True, help="Passing CI run that produced the downloaded host artifacts")
args = parser.parse_args()
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
    raise SystemExit("Commit all release changes before packaging")
run = json.loads(subprocess.check_output(["gh", "run", "view", args.ci_run,
    "--json", "headSha,conclusion"], cwd=ROOT, text=True))
if run["conclusion"] != "success":
    raise SystemExit("The source CI run must have passed")
if subprocess.check_output(["git", "diff", run["headSha"], "HEAD", "--", "crates", "include", "Cargo.toml", "Cargo.lock"], cwd=ROOT):
    raise SystemExit("Runtime source differs from the CI binaries; rebuild and download fresh artifacts")
common = [(p, p.relative_to(ROOT).as_posix()) for p in ROOT.glob("*.md")]
common += [(ROOT / "LICENSE", "LICENSE"), (ROOT / "include/pxr.h", "include/pxr.h"),
           (ROOT / "examples/demo.pxr", "examples/demo.pxr")]
common += [(p, p.relative_to(ROOT).as_posix()) for p in (ROOT / "docs").glob("*.md")]
common += [(p, p.relative_to(ROOT).as_posix()) for p in (ROOT / "docs/evidence").glob("*") if p.is_file()]


def pack(name, files, metadata, windows=False):
    suffix = ".zip" if windows else ".tar.gz"
    destination = DIST / (name + suffix)
    extra = json.dumps(metadata, indent=2).encode() + b"\n"
    if windows:
        with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as z:
            for path, arc in files + common:
                z.write(path, arc)
            z.writestr("BUILD.json", extra)
    else:
        with tarfile.open(destination, "w:gz") as tar:
            for path, arc in files + common:
                info = tar.gettarinfo(str(path), arcname=arc)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mode = 0o755 if arc == "pxr" else 0o644
                with path.open("rb") as f:
                    tar.addfile(info, f)
            info = tarfile.TarInfo("BUILD.json")
            info.size = len(extra)
            info.mode = 0o644
            tar.addfile(info, io.BytesIO(extra))
    return destination


bundles = []
for host in ("linux", "macos", "windows"):
    base = ROOT / ".tools" / ("ci-" + host)
    benchmark = json.loads((base / "benchmark.json").read_text(encoding="utf-8-sig"))
    exe = "pxr.exe" if host == "windows" else "pxr"
    binary = base / "target/release" / exe
    metadata = {"version": VERSION, "package_commit": commit,
                "binary_source_commit": run["headSha"],
                "ci_run": args.ci_run, "host": host, "architecture": benchmark["architecture"]}
    bundles.append(pack(f"pxr-{VERSION}-{host}-{benchmark['architecture']}",
                        [(binary, exe), (base / "benchmark.json", "ci-benchmark.json")],
                        metadata, host == "windows"))

for target in ("thumbv7em-none-eabihf", "riscv32imc-unknown-none-elf"):
    archive = ROOT / "target" / target / "release/libpxr_runtime_c_api.a"
    bundles.append(pack(f"pxr-{VERSION}-{target}", [(archive, "libpxr_runtime_c_api.a")],
                        {"version": VERSION, "package_commit": commit, "source_commit": run["headSha"], "target": target,
                         "toolchain": "Rust 1.98.1", "features": "no-default-features",
                         "hardware_validated": False}))

(DIST / "SHA256SUMS").write_text("".join(
    f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in bundles), encoding="ascii")
for p in bundles:
    print(p.name, p.stat().st_size)
