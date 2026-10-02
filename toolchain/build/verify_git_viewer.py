"""Run an exported macOS viewer with no authoring executables on PATH."""

import argparse
import json
import os
import plistlib
import subprocess
import tempfile
import time
from pathlib import Path


def verify(
    app: Path,
    realm: Path,
    output: Path,
    seconds: int,
    activate: bool = False,
    at: str | None = None,
) -> dict:
    app = app.resolve()
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    realm = realm.resolve()
    if realm.is_dir():
        realm = realm / "realm.json"
    version = json.loads(realm.with_name("version.json").read_text())
    if not version.get("commit_oid"):
        raise ValueError("The acceptance bundle must contain a fixed Git version")
    with (app / "Contents/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
    source_files = list(app.rglob("*.blend"))
    git_directories = [path for path in app.rglob(".git")]
    if source_files or git_directories or (app / "Contents/Resources/preview").exists():
        raise ValueError(
            "The standalone viewer must not contain authoring projects, Git data or an embedded world"
        )
    report_path = output / "runtime-report.json"
    screenshot = output / "viewer.png"
    command = [
        str(binary),
        "--",
        f"--benchmark={seconds}",
        "--static",
        "--no-recent",
        f"--realm={realm}",
        "--capture-after=4",
        f"--report={report_path}",
        f"--screenshot={screenshot}",
    ]
    if at:
        command.append("--at=" + at)
    environment = os.environ.copy()
    with tempfile.TemporaryDirectory(prefix="rou-viewer-empty-path-") as empty_path:
        environment["PATH"] = empty_path
        with (output / "runtime.log").open("w") as log:
            process = subprocess.Popen(
                command, env=environment, stdout=log, stderr=subprocess.STDOUT
            )
            activation = None
            try:
                if activate:
                    time.sleep(1)
                    script = (
                        'ObjC.import("AppKit"); '
                        f"$.NSRunningApplication.runningApplicationWithProcessIdentifier({process.pid})"
                        ".activateWithOptions(3);"
                    )
                    activation = subprocess.run(
                        ["/usr/bin/osascript", "-l", "JavaScript", "-e", script],
                        capture_output=True,
                        text=True,
                        check=False,
                        timeout=10,
                    )
                returncode = process.wait(timeout=seconds + 90)
            except BaseException:
                process.kill()
                process.wait()
                raise
    if returncode or "ERROR:" in (output / "runtime.log").read_text():
        raise RuntimeError(
            f"Exported viewer failed (exit {returncode}); inspect {output / 'runtime.log'}"
        )
    report = json.loads(report_path.read_text())
    checks = {
        "fixed_commit": report["commit_oid"] == version["commit_oid"],
        "view_root": report["view_root_sha256"] == version["view_root_sha256"],
        "local_bridge": report["view_validation"] == "local-trusted",
        "models_loaded": report["imports"] > 0 and bool(report["loaded_bounds"]),
        "assets_valid": not report["rejected"],
        "real_frames": report["frames"] > 10 and report["first_presented_seconds"] >= 0,
        "screenshot": screenshot.is_file() and screenshot.stat().st_size > 10000,
        "empty_authoring_path": True,
        "no_source_or_git_data": True,
    }
    summary = {
        "status": "passed" if all(checks.values()) else "failed",
        "app": str(app),
        "checks": checks,
        "commit_oid": report["commit_oid"],
        "seconds": report["seconds"],
        "imports": report["imports"],
        "frames": report["frames"],
        "focused_frames": report["focused_frames"],
        "unfocused_frames": report["unfocused_frames"],
        "foreground_measurement_valid": report["focused_frames"] >= 120
        and report["unfocused_frames"] == 0,
        "activation_requested": activate,
        "activation_result": activation.stdout.strip() if activation is not None else "",
        "frame_ms_p50": report["frame_ms_p50"],
        "frame_ms_p95": report["frame_ms_p95"],
        "godot": report["godot"],
        "renderer": report["renderer"],
        "gpu": report["gpu"],
        "screenshot": str(screenshot),
    }
    (output / "acceptance.json").write_text(json.dumps(summary, indent=2) + "\n")
    if summary["status"] != "passed":
        raise AssertionError(json.dumps(summary, indent=2))
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument(
        "--realm", type=Path, required=True, help="An external fixed-version browse bundle"
    )
    parser.add_argument("--output", type=Path, required=True, help="A new acceptance directory")
    parser.add_argument("--seconds", type=int, default=12)
    parser.add_argument(
        "--activate", action="store_true", help="Activate the existing app PID using AppKit"
    )
    parser.add_argument("--at", help="Initial position in metres: x,y,z")
    args = parser.parse_args()
    if not 5 <= args.seconds <= 60:
        parser.error("Choose a 5\u201360 second rendering sample")
    print(
        json.dumps(
            verify(args.app, args.realm, args.output, args.seconds, args.activate, args.at),
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
