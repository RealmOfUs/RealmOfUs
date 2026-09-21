"""Run reviewed core fixtures and real Blender-to-Godot bundle/lifecycle checks."""

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.compare_grids import compare_grids
from toolchain.build.native import load_core, validate_bundle
from toolchain.build.package_addon import package_addon


def run_godot(script: str, log: Path, marker: str, user_args: list[str]) -> str:
    command = [
        "godot",
        "--headless",
        "--path",
        str(ROOT / "client/app"),
        "--script",
        script,
        "--",
        *user_args,
    ]
    result = subprocess.run(command, capture_output=True, text=True, timeout=120)
    output = result.stdout + result.stderr
    log.write_text(output)
    if result.returncode or marker not in result.stdout or "SCRIPT ERROR:" in output:
        raise RuntimeError(output)
    return output


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--realm",
        action="append",
        type=Path,
        help="Complete generated bundle; repeat to test multiple scenes",
    )
    parser.add_argument(
        "--viewer-realm", type=Path, help="Also exercise streaming lifecycle with this bundle"
    )
    args = parser.parse_args()
    realms = args.realm or [ROOT / "build/s1-2.1/preview/realm.json"]
    output = ROOT / "build/s1-2.1/checks"
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run([sys.executable, str(ROOT / "toolchain/build/verify_spatial.py")], check=True)
    core = load_core()
    for realm in realms:
        validate_bundle(realm, core)
    checks = run_godot(
        "../tests/verify.gd",
        output / "godot-checks.log",
        "S1_CHECKS_PASS",
        ["--realm=" + str(realm.resolve()) for realm in realms],
    )
    if args.viewer_realm:
        run_godot(
            "../tests/lifecycle.gd",
            output / "lifecycle.log",
            "LIFECYCLE_PASS",
            ["--realm=" + str(args.viewer_realm.resolve())],
        )
    comparison = compare_grids(core.read_json(ROOT / "toolchain/fixtures/calibration.json"))
    (output / "grid-comparison.json").write_text(json.dumps(comparison, indent=2) + "\n")
    archive = package_addon(ROOT / "dist/rou_blender.zip")
    print(checks.strip())
    print("ROU_ADDON_PACKAGED", archive)


if __name__ == "__main__":
    main()
