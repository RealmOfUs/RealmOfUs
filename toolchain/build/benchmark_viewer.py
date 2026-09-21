"""Real displayed rendering, with process RSS sampled independently of engine memory."""

import argparse
import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def run(command, report, seconds=30, user_args=None):
    report = Path(report).resolve()
    report.parent.mkdir(parents=True, exist_ok=True)
    report.unlink(missing_ok=True)
    ready_report = Path(str(report) + ".ready.json")
    ready_report.unlink(missing_ok=True)
    log = report.with_suffix(".log")
    peak = 0
    samples = 0
    first_presented_wall = None
    started = time.monotonic()
    with log.open("w") as stream:
        proc = subprocess.Popen(
            command
            + ["--"]
            + (user_args or [])
            + ["--benchmark=" + str(seconds), "--report=" + str(report)],
            cwd="/tmp",
            stdout=stream,
            stderr=subprocess.STDOUT,
        )
        try:
            while proc.poll() is None:
                s = subprocess.run(
                    ["ps", "-o", "rss=", "-p", str(proc.pid)], capture_output=True, text=True
                ).stdout.strip()
                if s:
                    peak = max(peak, int(s) * 1024)
                    samples += 1
                if first_presented_wall is None and ready_report.exists():
                    first_presented_wall = time.monotonic() - started
                if time.monotonic() - started > seconds + 120:
                    raise TimeoutError("Viewer timeout: " + str(log))
                time.sleep(0.2)
        finally:
            if proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=10)
    if proc.returncode or not report.exists() or "SCRIPT ERROR:" in log.read_text():
        raise RuntimeError("Viewer failed: " + str(log))
    data = json.loads(report.read_text())
    if data.get("rejected") or data.get("frames", 0) == 0:
        raise RuntimeError("Viewer did not complete a usable measurement: " + str(log))
    data.update(
        peak_rss_bytes=peak,
        rss_samples=samples,
        process_wall_seconds=time.monotonic() - started,
        first_presented_process_seconds=first_presented_wall,
        startup_observation="New process; OS file caches and driver shader caches were not cleared",
        foreground_measurement_valid=data.get("focused_frames", 0) > 0
        and data.get("unfocused_frames", 0) == 0,
    )
    report.write_text(json.dumps(data, indent=2) + "\n")
    print(
        json.dumps(
            {
                k: data[k]
                for k in [
                    "realm",
                    "frame_ms_p95",
                    "first_ready_seconds",
                    "peak_rss_bytes",
                    "imports",
                    "unloads",
                    "cancelled",
                    "viewport",
                ]
            }
        ),
        flush=True,
    )
    return data


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--fixture", choices=["s1", "walsh", "walsh-v2", "walsh-v3"], default="s1")
    p.add_argument("--seconds", type=int, default=30)
    p.add_argument("--realm", type=Path, help="Measure any local realm instead of a named fixture")
    p.add_argument("--report", type=Path, help="Write the measurement here")
    p.add_argument("--executable", type=Path, help="Use an exported viewer executable")
    p.add_argument("--balanced", action="store_true")
    p.add_argument("--daylight", action="store_true")
    p.add_argument("--at", help="Initial x,y,z in metres")
    p.add_argument("--sample", choices=["villa-anahid"])
    p.add_argument("--station", type=int, default=0)
    a = p.parse_args()
    if a.seconds < 5:
        p.error("Measure at least five seconds")
    if a.realm or a.executable:
        command = (
            [str(a.executable.resolve())]
            if a.executable
            else ["godot", "--path", str(ROOT / "client/app")]
        )
        arguments = ["--realm=" + str(a.realm.resolve())] if a.realm else []
        arguments += ["--" + flag for flag in ["balanced", "daylight"] if getattr(a, flag)]
        if a.at:
            arguments.append("--at=" + a.at)
        if a.sample:
            arguments += ["--sample=" + a.sample, "--station=" + str(a.station)]
        run(command, a.report or ROOT / "build/viewer/measurement.json", a.seconds, arguments)
        return
    dest = ROOT / "build" / (a.fixture + "-2.1") / "measurements"
    dest.mkdir(parents=True, exist_ok=True)
    if a.fixture == "s1":
        for size in [16, 32, 64]:
            # --realm is a user argument, so arrange engine and user options explicitly.
            command = ["godot", "--path", str(ROOT / "client/app")]
            realm = ROOT / f"build/s1-2.1/preview/realm-{size}.json"
            # run() appends the separator; wrapper option is handled by its argument list below.
            run_with_realm(command, realm, dest / f"grid-{size}.json", a.seconds)
    else:
        directory = ROOT / "build" / (a.fixture + "-2.1")
        run(
            ["godot", "--path", str(ROOT / "client/app")],
            dest / "viewer.json",
            a.seconds,
            [
                "--realm=" + str(directory / "preview/realm.json"),
                "--sample=villa-anahid",
                "--sample-data=" + str(directory / "sample.json"),
                "--station=0",
                "--no-recent",
            ],
        )


def run_with_realm(command, realm, report, seconds):
    # A tiny per-run project override isn't needed: run accepts the user argument list.
    run(
        command,
        report,
        seconds,
        user_args=["--realm=" + str(realm), "--at=30,26,44", "--no-recent"],
    )


if __name__ == "__main__":
    main()
