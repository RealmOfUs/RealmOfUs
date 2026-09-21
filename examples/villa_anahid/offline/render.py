"""Render actual animated Cycles frames, with restartable PNG output and timings.

blender -b build/walsh-offline/living-room-cycles.blend --python-exit-code 1 \
  --python examples/villa_anahid/offline/render.py -- --frames 1,121,240 --width 960 --samples 48 --label proofs
"""

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[3]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--output-dir", type=Path, default=ROOT / "build/walsh-offline")
    p.add_argument("--frames", default="1-288")
    p.add_argument("--width", type=int, default=1920)
    p.add_argument("--samples", type=int, default=128)
    p.add_argument("--min-samples", type=int, default=16)
    p.add_argument("--stride", type=int, default=1)
    p.add_argument("--threshold", type=float, default=0.018)
    p.add_argument("--label", default="frames")
    p.add_argument("--depth", choices=["8", "16"], default="8")
    p.add_argument("--overwrite", action="store_true")
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
    assert a.stride > 0 and a.width >= 64 and a.samples > 0 and a.min_samples > 0
    frames = []
    for token in a.frames.split(","):
        if "-" in token:
            lo, hi = map(int, token.split("-"))
            frames.extend(range(lo, hi + 1, a.stride))
        else:
            frames.append(int(token))
    assert frames and all(1 <= f <= 288 for f in frames)
    scene = bpy.context.scene
    assert scene.camera and scene.render.engine == "CYCLES"
    prefs = bpy.context.preferences.addons["cycles"].preferences
    prefs.compute_device_type = "METAL"
    prefs.refresh_devices()
    assert any(d.type == "METAL" for d in prefs.devices), "Metal GPU unavailable"
    for d in prefs.devices:
        d.use = d.type == "METAL"
    scene.cycles.device = "GPU"
    scene.cycles.samples = a.samples
    scene.cycles.adaptive_threshold = a.threshold
    scene.cycles.adaptive_min_samples = min(a.min_samples, a.samples)
    scene.render.resolution_x = a.width
    scene.render.resolution_y = round(a.width * 9 / 16)
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_depth = a.depth
    scene.render.image_settings.color_mode = "RGB"
    folder = a.output_dir.resolve() / a.label
    folder.mkdir(parents=True, exist_ok=True)
    report_path = folder / "render-report.json"
    report = json.loads(report_path.read_text()) if report_path.exists() else {"frames": {}}
    settings = {
        "width": scene.render.resolution_x,
        "height": scene.render.resolution_y,
        "samples_max": a.samples,
        "samples_min": min(a.min_samples, a.samples),
        "adaptive_threshold": a.threshold,
        "fps": 24,
        "renderer": "Cycles / Metal",
        "blender": bpy.app.version_string,
        "source_sha256": hashlib.sha256(Path(bpy.data.filepath).read_bytes()).hexdigest(),
    }
    if report.get("settings") and report["settings"] != settings:
        if not a.overwrite:
            raise RuntimeError("Output settings/source changed; select a new label or --overwrite.")
        report["frames"] = {}  # Old frames must never inherit new render settings.
    report["settings"] = settings
    for frame in frames:
        path = folder / f"{frame:04d}.png"
        if path.exists() and str(frame) in report["frames"] and not a.overwrite:
            continue
        scene.frame_set(frame)
        scene.render.filepath = str(path)
        start = time.monotonic()
        result = bpy.ops.render.render(write_still=True)
        elapsed = time.monotonic() - start
        if "CANCELLED" in result or not path.exists():
            print("RENDER_CANCELLED", frame, flush=True)
            return
        payload = path.read_bytes()
        report["frames"][str(frame)] = {
            "seconds": elapsed,
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
        }
        report_path.write_text(json.dumps(report, indent=2) + "\n")
        print(
            json.dumps({"completed_frame": frame, "seconds": round(elapsed, 3), "file": str(path)}),
            flush=True,
        )
    print("RENDER_COMPLETE", len(frames), flush=True)


if __name__ == "__main__":
    main()
