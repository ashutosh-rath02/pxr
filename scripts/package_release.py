"""Download, verify and package one successful CI run from an exact clean checkout.

Requires Python 3.11+ and an authenticated gh CLI. No local build output is used.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_artifact(base, kind, commit, run_id, version):
    manifest = json.loads((base / "artifact-manifest.json").read_text())
    expected = {"kind": kind, "commit": commit, "run_id": run_id, "version": version}
    if any(manifest.get(key) != value for key, value in expected.items()):
        raise ValueError(f"Artifact provenance mismatch: {base.name}")
    if not manifest["files"]:
        raise ValueError(f"Empty artifact: {base.name}")
    files = {}
    for name, expected_digest in manifest["files"].items():
        path = (base / name).resolve()
        if not path.is_relative_to(base.resolve()) or not path.is_file():
            raise ValueError(f"Invalid artifact path: {name}")
        if digest(path) != expected_digest:
            raise ValueError(f"Artifact checksum mismatch: {name}")
        files[name] = path
    return manifest, files


def pack(destination, files, metadata):
    metadata = {**metadata, "packaged_files": {name: digest(path) for path, name in files}}
    extra = json.dumps(metadata, indent=2).encode() + b"\n"
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path, name in files:
                archive.write(path, name)
            archive.writestr("BUILD.json", extra)
    else:
        with tarfile.open(destination, "w:gz") as archive:
            for path, name in files:
                info = archive.gettarinfo(str(path), arcname=name)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mode = 0o755 if name == "pxr" else 0o644
                with path.open("rb") as stream:
                    archive.addfile(info, stream)
            info = tarfile.TarInfo("BUILD.json")
            info.size = len(extra)
            info.mode = 0o644
            archive.addfile(info, io.BytesIO(extra))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ci-run", required=True, type=int)
    args = parser.parse_args()
    run_id = str(args.ci_run)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    commit = command("git", "rev-parse", "HEAD")
    if command("git", "status", "--porcelain"):
        raise SystemExit("Commit all release changes before packaging")
    run = json.loads(command("gh", "run", "view", run_id,
                             "--json", "headSha,conclusion,status,workflowName,url"))
    if (run["status"], run["conclusion"], run["workflowName"], run["headSha"]) != (
            "completed", "success", "CI", commit):
        raise SystemExit("Packaging requires a successful CI run for exactly HEAD")
    cache = ROOT / ".tools/release-ci"
    cache.mkdir(parents=True, exist_ok=True)
    download = Path(tempfile.mkdtemp(prefix=f"{run_id}-", dir=cache))
    subprocess.run(["gh", "run", "download", run_id, "--dir", str(download)], cwd=ROOT, check=True)
    dist = ROOT / "dist" / f"v{version}"
    dist.mkdir(parents=True, exist_ok=True)
    common_paths = list(ROOT.glob("*.md")) + [ROOT / "LICENSE", ROOT / "include/pxr.h"]
    for directory in ("docs", "examples", "ports/qemu"):
        common_paths.extend(p for p in (ROOT / directory).rglob("*") if p.is_file() and "__pycache__" not in p.parts)
    common_paths.append(ROOT / "crates/runtime-core/examples/embedding.rs")
    common = [(path, path.relative_to(ROOT).as_posix()) for path in sorted(common_paths)]
    bundles = []
    for host in ("ubuntu", "macos", "windows"):
        base = download / f"host-{host}-latest"
        manifest, files = validate_artifact(base, "host", commit, run_id, version)
        benchmark = json.loads(files["benchmark.json"].read_text(encoding="utf-8-sig"))
        exe = "pxr.exe" if host == "windows" else "pxr"
        lib = "pxr_runtime_c_api.lib" if host == "windows" else "libpxr_runtime_c_api.a"
        os_name = "linux" if host == "ubuntu" else host
        suffix = "zip" if host == "windows" else "tar.gz"
        path = dist / f"pxr-{version}-{os_name}-{benchmark['architecture']}.{suffix}"
        payload = [(files[f"target/release/{exe}"], exe),
                   (files[f"target/release/{lib}"], f"lib/{lib}"),
                   (files["benchmark.json"], "ci-benchmark.json")]
        pack(path, payload + common, {**manifest, "ci_url": run["url"], "hardware_validated": False})
        bundles.append(path)
    for target in ("thumbv7em-none-eabihf", "riscv32imc-unknown-none-elf"):
        manifest, files = validate_artifact(download / f"embedded-{target}", target, commit, run_id, version)
        path = dist / f"pxr-{version}-{target}.tar.gz"
        archive = files[f"target/{target}/release/libpxr_runtime_c_api.a"]
        pack(path, [(archive, "lib/libpxr_runtime_c_api.a")] + common,
             {**manifest, "ci_url": run["url"], "features": "no-default-features", "hardware_validated": False})
        bundles.append(path)
    manifest, files = validate_artifact(download / "emulated-firmware", "emulated-firmware", commit, run_id, version)
    path = dist / f"pxr-{version}-qemu-firmware.tar.gz"
    pack(path, [(source, name.removeprefix("target/")) for name, source in files.items()] + common,
         {**manifest, "ci_url": run["url"], "hardware_validated": False})
    bundles.append(path)
    (dist / "SHA256SUMS").write_text("".join(f"{digest(path)}  {path.name}\n" for path in bundles), encoding="ascii", newline="\n")
    for path in bundles:
        print(path.relative_to(ROOT), path.stat().st_size)


if __name__ == "__main__":
    main()
