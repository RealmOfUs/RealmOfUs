"""Background render monitor; finish the still and video without occupying chat.

Run with --renderer-pid for the existing video-render Blender process.
Progress and failures are persisted to build/walsh-offline/job-status.json.
"""

import argparse
import json
import os
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BUILD = ROOT / "build/walsh-offline"
EXPECTED = set(map(str, range(1, 289, 2)))


def status(phase, **details):
    payload = {"phase": phase, "updated_at": datetime.now(timezone.utc).isoformat(), **details}
    path = BUILD / "job-status.json"
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(payload, indent=2) + "\n")
    temporary.replace(path)
    return payload


def load_report():
    # The render process may be replacing its report while this monitor reads it.
    for _ in range(5):
        try:
            return json.loads((BUILD / "frames/render-report.json").read_text())
        except (OSError, ValueError):
            time.sleep(0.2)
    raise RuntimeError("Cannot read the frame render report")


def run_logged(command, log_name, phase):
    with (BUILD / log_name).open("a") as log:
        child = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        while child.poll() is None:
            status(phase, rendered_frames=144, total_frames=144, child_pid=child.pid, log=log_name)
            time.sleep(15)
        if child.returncode:
            raise RuntimeError(f"{phase} failed with exit {child.returncode}; see {log_name}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--renderer-pid", type=int, required=True)
    args = parser.parse_args()
    try:
        while True:
            report = load_report()
            done = len(EXPECTED.intersection(report["frames"]))
            status(
                "rendering_video",
                rendered_frames=done,
                total_frames=144,
                output_fps=24,
                output_seconds=6,
                renderer_pid=args.renderer_pid,
            )
            if done == 144:
                break
            try:
                os.kill(args.renderer_pid, 0)
            except ProcessLookupError:
                raise RuntimeError(f"Video renderer exited after {done}/144 frames")
            time.sleep(25)
        run_logged(
            [
                "caffeinate",
                "-i",
                "-s",
                "blender",
                "--background",
                str(BUILD / "living-room-cycles.blend"),
                "--python-exit-code",
                "1",
                "--python",
                str(ROOT / "examples/villa_anahid/offline/render.py"),
                "--",
                "--frames",
                "1",
                "--width",
                "3840",
                "--samples",
                "256",
                "--min-samples",
                "64",
                "--threshold",
                ".008",
                "--depth",
                "16",
                "--label",
                "stills",
            ],
            "still.log",
            "rendering_4k_still",
        )
        still = BUILD / "stills/0001.png"
        if not still.exists():
            raise RuntimeError("4K render returned without an image")
        run_logged(
            ["python3", "examples/villa_anahid/offline/package.py"],
            "delivery.log",
            "encoding_and_checking",
        )
        delivery = json.loads((ROOT / "dist/walsh-offline/delivery-report.json").read_text())
        result = status(
            "complete",
            rendered_frames=144,
            total_frames=144,
            movie=delivery["movie"],
            still=delivery["still"]["path"],
            movie_sha256=delivery["sha256"],
            decode_pass=delivery["decode_pass"],
        )
        print(json.dumps(result, indent=2), flush=True)
    except Exception as exc:
        status("failed", error=str(exc))
        raise


if __name__ == "__main__":
    main()
