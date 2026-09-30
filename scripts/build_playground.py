"""Build the static browser playground. Requires Rust's wasm32 target; no npm build."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo.exe")
    subprocess.run([cargo, "build", "--locked", "--release", "-p", "pxr-playground",
                    "--target", "wasm32-unknown-unknown"], cwd=ROOT, check=True)
    destination = ROOT / "dist/playground"
    destination.mkdir(parents=True, exist_ok=True)
    for path in (ROOT / "web").iterdir():
        if path.is_file():
            shutil.copyfile(path, destination / path.name)
    shutil.copyfile(ROOT / "target/wasm32-unknown-unknown/release/pxr_playground.wasm",
                    destination / "pxr_playground.wasm")
    inputs = ["Cargo.toml", "Cargo.lock", "web", "crates/playground", "crates/runtime-core",
              "crates/runtime-sim", "crates/runtime-codec", "scripts/build_playground.py"]
    metadata = {
        "version": tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"],
        "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain", "--", *inputs], cwd=ROOT)),
        "run_id": os.environ.get("GITHUB_RUN_ID"),
        "files": {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                  for path in sorted(destination.iterdir()) if path.is_file() and path.name != "build.json"},
    }
    (destination / "build.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"Built {destination}; serve with: python -m http.server 8080 --directory dist/playground")


if __name__ == "__main__":
    main()
