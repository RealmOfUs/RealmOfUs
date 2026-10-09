"""Capture focused near/lod1 evidence and verify release of the detailed sample."""

import argparse
import json
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def run(realm: Path, output: Path, activate: bool, url: str | None = None) -> dict:
    """Run the real display harness; engine errors and unfocused captures are fatal."""
    if not url and not realm.is_file():
        default = ROOT / "build/scale/detailed/.rou/view/worktree/realm.json"
        if realm != default:
            raise FileNotFoundError(realm)
        # The generator requires an empty destination and never replaces author data.
        subprocess.run(
            [sys.executable, str(ROOT / "examples/scale/make_detailed_realm.py")],
            cwd=ROOT,
            check=True,
        )
    output.mkdir(parents=True, exist_ok=True)
    report = output / "report.json"
    report.unlink(missing_ok=True)
    log = output / "display.log"
    command = [
        "godot",
        "--path",
        str(ROOT / "client/app"),
        "--script",
        "../tests/detailed_display.gd",
        "--",
        ("--url=" + url) if url else ("--realm=" + str(realm)),
        # The online variant keeps its download cache under the output directory
        # instead of the real user cache.
        *(["--cache=" + str(output / "cache")] if url else []),
        "--capture-dir=" + str(output),
        "--no-recent",
        "--balanced",
    ]
    with log.open("w") as stream:
        proc = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT, cwd=ROOT)
        try:
            if activate:
                time.sleep(1)
                script = (
                    'ObjC.import("AppKit"); '
                    f"$.NSRunningApplication.runningApplicationWithProcessIdentifier({proc.pid})"
                    ".activateWithOptions(3);"
                )
                activation_deadline = time.monotonic() + 10
                while proc.poll() is None and time.monotonic() < activation_deadline:
                    subprocess.run(
                        ["/usr/bin/osascript", "-l", "JavaScript", "-e", script],
                        capture_output=True,
                        text=True,
                        timeout=10,
                        check=True,
                    )
                    time.sleep(0.5)
            proc.wait(timeout=180)
        finally:
            if proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=10)
    text = log.read_text()
    if proc.returncode or "ERROR:" in text or "DETAILED_DISPLAY_PASS " not in text:
        raise RuntimeError("Detailed display failed; see " + str(log))
    data = json.loads(report.read_text())
    captures = data["captures"]
    if (
        data["status"] != "passed"
        or data["errors"]
        or data["rejected"]
        or data["asset_failures"]
        or data["resident_scenes"]
        or data["cached_prototypes"]
        or len(captures) != 6
        or not all(c["focused"] and Path(c["path"]).is_file() for c in captures)
    ):
        raise RuntimeError("Incomplete detailed display receipt; see " + str(report))
    data.update(
        checked_at=datetime.now(timezone.utc).isoformat(),
        command=command,
        activation_requested=activate,
        engine_errors=0,
    )
    report.write_text(json.dumps(data, indent=2) + "\n")
    print("DETAILED_DISPLAY_VERIFIED", json.dumps(data))
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--realm", type=Path, default=ROOT / "build/scale/detailed/.rou/view/worktree/realm.json"
    )
    parser.add_argument("--output", type=Path, default=ROOT / "build/scale/detailed-display")
    parser.add_argument("--url", help="Open the detailed sample from an online viewer address")
    parser.add_argument("--activate", action="store_true", help="Activate the measured macOS PID")
    args = parser.parse_args()
    run(args.realm.resolve(), args.output.resolve(), args.activate, args.url)


if __name__ == "__main__":
    main()
