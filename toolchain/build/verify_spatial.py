"""Run reviewed spatial fixtures in Rust and through the actual Godot extension."""

import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.native import build_native


def main():
    out = ROOT / "build/s1-2.1/spatial-checks"
    out.mkdir(parents=True, exist_ok=True)
    build_native("rou-godot", "rou-ffi")
    commands = {
        "rust": [
            "cargo",
            "test",
            "-p",
            "rou-core",
            "--test",
            "spatial_contract",
        ],
        "godot": [
            "godot",
            "--headless",
            "--path",
            str(ROOT / "client/app"),
            "--script",
            "../tests/spatial.gd",
        ],
    }
    logs = {}
    for name, command in commands.items():
        proc = subprocess.run(
            command,
            cwd=ROOT / "rou" if name == "rust" else ROOT,
            capture_output=True,
            text=True,
            timeout=120,
        )
        logs[name] = proc.stdout + proc.stderr
        (out / (name + ".log")).write_text(logs[name])
        if proc.returncode or (
            name == "godot"
            and ("RUST_SPATIAL_PASS " not in proc.stdout or "SCRIPT ERROR:" in logs[name])
        ):
            raise SystemExit(logs[name])
    fixture = ROOT / "rou/crates/rou-core/tests/fixtures/spatial.json"
    vectors = json.loads(fixture.read_text())
    report = {
        "status": "passed",
        "checked_at": datetime.now(timezone.utc).isoformat(),
        "scope": "Rust core and native Godot ABI; no rendering or source assets",
        "godot": subprocess.check_output(["godot", "--version"], text=True).strip(),
        "cases": {
            key: len(vectors[key]) for key in ["profiles", "positions", "bounds", "catalogs"]
        },
        "godot_assertions": int(logs["godot"].split("RUST_SPATIAL_PASS ")[1].split()[0]),
        "profile_hash": vectors["profile_hash"],
        "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
    }
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print("S1_SPATIAL_VERIFIED", json.dumps(report))


if __name__ == "__main__":
    main()
