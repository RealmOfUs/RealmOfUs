"""Run reviewed spatial fixtures in Rust and through the actual Godot extension."""

import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.compare_grids import compare_grids
from toolchain.build.native import ffi_library, godot_extension


def main():
    out = ROOT / "build/s1-2.1/spatial-checks"
    out.mkdir(parents=True, exist_ok=True)
    godot_extension()
    ffi_library()
    commands = {
        "rust": [
            "cargo",
            "test",
            "-p",
            "rou-core",
            "--test",
            "spatial_contract",
        ],
        "viewer": ["cargo", "test", "-p", "rou-core", "--test", "viewer_contract"],
        "streaming": [
            "godot",
            "--headless",
            "--path",
            str(ROOT / "client/app"),
            "--script",
            "../tests/streaming_contract.gd",
            "--",
            "--radius=16,64,4000",
        ],
        "godot": [
            "godot",
            "--headless",
            "--path",
            str(ROOT / "client/app"),
            "--script",
            "../tests/spatial.gd",
        ],
        "fault": [
            "godot",
            "--headless",
            "--path",
            str(ROOT / "client/app"),
            "--script",
            "../tests/loader_fault.gd",
        ],
    }
    logs = {}
    for name, command in commands.items():
        proc = subprocess.run(
            command,
            cwd=ROOT / "rou" if name in ["rust", "viewer"] else ROOT,
            capture_output=True,
            text=True,
            timeout=120,
        )
        logs[name] = proc.stdout + proc.stderr
        (out / (name + ".log")).write_text(logs[name])
        if name == "fault":
            # This isolated negative probe deliberately emits exactly one script error.
            if (
                proc.returncode
                or "LOADER_FAULT_RECOVERED" not in proc.stdout
                or logs[name].count("ERROR:") != 1
                or "injected_worker_failure" not in proc.stderr
            ):
                raise SystemExit(logs[name])
            continue
        if (
            proc.returncode
            or "ERROR:" in logs[name]
            or (name == "streaming" and "STREAMING_CONTRACT_PASS " not in proc.stdout)
            or (name == "godot" and "RUST_SPATIAL_PASS " not in proc.stdout)
        ):
            raise SystemExit(logs[name])
    fixture = ROOT / "rou/crates/rou-core/tests/fixtures/spatial.json"
    vectors = json.loads(fixture.read_text())
    comparison = compare_grids(
        json.loads((ROOT / "toolchain/fixtures/calibration.json").read_text())
    )
    (out / "grid-comparison.json").write_text(json.dumps(comparison, indent=2) + "\n")
    report = {
        "status": "passed",
        "checked_at": datetime.now(timezone.utc).isoformat(),
        "scope": "Rust spatial/viewer rules, native Godot ABI and headless dense-GLB streaming",
        "godot": subprocess.check_output(["godot", "--version"], text=True).strip(),
        "cases": {
            key: len(vectors[key]) for key in ["profiles", "positions", "bounds", "catalogs"]
        },
        "godot_assertions": int(logs["godot"].split("RUST_SPATIAL_PASS ")[1].split()[0]),
        "viewer_contract_tests": int(
            re.search(r"test result: ok\. (\d+) passed", logs["viewer"])[1]
        ),
        "streaming_assertions": int(
            logs["streaming"].split("STREAMING_CONTRACT_PASS ")[1].split()[0]
        ),
        "grid_comparison_views": sum(len(grid["views_48m_radius"]) for grid in comparison),
        "injected_worker_error_recovered": True,
        "profile_hash": vectors["profile_hash"],
        "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
    }
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print("S1_SPATIAL_VERIFIED", json.dumps(report))


if __name__ == "__main__":
    main()
