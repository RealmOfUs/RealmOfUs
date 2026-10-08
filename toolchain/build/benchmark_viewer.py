"""Real displayed rendering, with process RSS sampled independently of engine memory."""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def power_source():
    """Report mains or battery power; frame-time P95 was 3-5 ms worse on battery."""
    if sys.platform != "darwin":
        return "unknown"
    status = subprocess.run(["pmset", "-g", "batt"], capture_output=True, text=True).stdout
    if "AC Power" in status:
        return "ac"
    return "battery" if "Battery Power" in status else "unknown"


def run(command, report, seconds=30, user_args=None, json_output=False, activate=False):
    report = Path(report).resolve()
    report.parent.mkdir(parents=True, exist_ok=True)
    report.unlink(missing_ok=True)
    ready_report = Path(str(report) + ".ready.json")
    ready_report.unlink(missing_ok=True)
    log = report.with_suffix(".log")
    peak = 0
    samples = 0
    rss_timeline = []
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
        activation = None
        try:
            if activate:
                # Match the exported-app harness: activate this PID without opening another app.
                time.sleep(1)
                script = (
                    'ObjC.import("AppKit"); '
                    f"$.NSRunningApplication.runningApplicationWithProcessIdentifier({proc.pid})"
                    ".activateWithOptions(3);"
                )
                activation = subprocess.run(
                    ["/usr/bin/osascript", "-l", "JavaScript", "-e", script],
                    capture_output=True,
                    text=True,
                    timeout=10,
                    check=False,
                )
            while proc.poll() is None:
                s = subprocess.run(
                    ["ps", "-o", "rss=", "-p", str(proc.pid)], capture_output=True, text=True
                ).stdout.strip()
                # Exclude exit-time zero RSS and teardown after the engine closes its report.
                if s and int(s) > 0 and not report.exists():
                    peak = max(peak, int(s) * 1024)
                    samples += 1
                    rss_timeline.append(
                        {"seconds": time.monotonic() - started, "bytes": int(s) * 1024}
                    )
                if first_presented_wall is None and ready_report.exists():
                    first_presented_wall = time.monotonic() - started
                if time.monotonic() - started > seconds + 120:
                    raise TimeoutError("Viewer timeout: " + str(log))
                time.sleep(0.2)
        finally:
            if proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=10)
    if proc.returncode or not report.exists() or "ERROR:" in log.read_text():
        raise RuntimeError("Viewer failed: " + str(log))
    data = json.loads(report.read_text())
    if data.get("rejected") or data.get("asset_failures") or data.get("frames", 0) == 0:
        raise RuntimeError("Viewer did not complete a usable measurement: " + str(log))
    if data["mount_frames"]["count"] + data["steady_frames"]["count"] != data["frames"]:
        raise RuntimeError("Frame groups do not partition the measurement: " + str(report))
    data.update(
        activation_requested=activate,
        power_source=power_source(),
        activation_result=activation.stdout.strip() if activation else None,
        peak_rss_bytes=peak,
        rss_samples=samples,
        rss_timeline=rss_timeline,
        final_rss_bytes=rss_timeline[-1]["bytes"] if rss_timeline else 0,
        process_wall_seconds=time.monotonic() - started,
        first_presented_process_seconds=first_presented_wall,
        startup_observation="New process; OS file caches and driver shader caches were not cleared",
        foreground_measurement_valid=data.get("focused_frames", 0) > 0
        and data.get("unfocused_frames", 0) == 0,
    )
    report.write_text(json.dumps(data, indent=2) + "\n")
    summary = {
        k: data[k]
        for k in [
            "realm",
            "frame_ms_p95",
            "mount_frames",
            "steady_frames",
            "scene_ms_p95",
            "main_thread_scenes",
            "gltf_parse_in_worker",
            "sdfgi",
            "first_ready_seconds",
            "peak_rss_bytes",
            "final_rss_bytes",
            "imports",
            "unloads",
            "cancelled",
            "viewport",
            "foreground_measurement_valid",
            "power_source",
        ]
    }
    print(json.dumps(summary, indent=None if json_output else 2), flush=True)
    return data


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--fixture", choices=["s1", "walsh", "walsh-v2", "walsh-v3"], default="s1")
    p.add_argument("--seconds", type=int, default=30)
    p.add_argument("--realm", type=Path, help="Measure any local realm instead of a named fixture")
    p.add_argument("--report", type=Path, help="Write the measurement here")
    p.add_argument("--executable", type=Path, help="Use an exported viewer executable")
    p.add_argument("--activate", action="store_true", help="Activate the measured macOS app PID")
    p.add_argument("--json", action="store_true", help="Emit a compact JSON summary")
    p.add_argument(
        "--flight", action="store_true", help="Fly across the default world and release all assets"
    )
    p.add_argument("--radius", help="Near,middle,far radii in metres")
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
        if a.flight:
            if a.executable:
                p.error("--flight requires the development project test script")
            command += ["--script", "../tests/benchmark_flight.gd"]
        arguments = ["--realm=" + str(a.realm.resolve())] if a.realm else []
        arguments += ["--" + flag for flag in ["balanced", "daylight"] if getattr(a, flag)]
        if a.radius:
            arguments.append("--radius=" + a.radius)
        if a.at:
            arguments.append("--at=" + a.at)
        if a.sample:
            arguments += ["--sample=" + a.sample, "--station=" + str(a.station)]
        run(
            command,
            a.report or ROOT / "build/viewer/measurement.json",
            a.seconds,
            arguments,
            a.json,
            a.activate,
        )
        return
    dest = ROOT / "build" / (a.fixture + "-2.1") / "measurements"
    dest.mkdir(parents=True, exist_ok=True)
    if a.fixture == "s1":
        for size in [16, 32, 64]:
            # --realm is a user argument, so arrange engine and user options explicitly.
            command = ["godot", "--path", str(ROOT / "client/app")]
            realm = ROOT / f"build/s1-2.1/grid-{size}/realm.json"
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
