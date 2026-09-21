"""Assemble and validate the 6-second, 24 fps Cycles study from 144 real frames."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BUILD = ROOT / "build/walsh-offline"
DIST = ROOT / "dist/walsh-offline"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_video(build_dir: Path, output_dir: Path) -> dict:
    report = json.loads((build_dir / "frames/render-report.json").read_text())
    expected = list(range(1, 289, 2))
    sequence = output_dir / "film-sequence"
    sequence.mkdir(parents=True, exist_ok=True)
    for index, frame in enumerate(expected):
        original = build_dir / "frames" / f"{frame:04d}.png"
        assert str(frame) in report["frames"], f"Missing frame {frame}"
        assert sha(original) == report["frames"][str(frame)]["sha256"], f"Changed frame {frame}"
        # Validate PNG dimensions from its header, without adding Python packages.
        raw = original.read_bytes()
        assert raw[:8] == b"\x89PNG\r\n\x1a\n"
        assert (
            int.from_bytes(raw[16:20], "big") == 1920 and int.from_bytes(raw[20:24], "big") == 1080
        )
        target = sequence / f"{index:04d}.png"
        if target.exists():
            target.unlink()
        try:
            os.link(original, target)
        except OSError:
            shutil.copy2(original, target)
    output_dir.mkdir(parents=True, exist_ok=True)
    movie = output_dir / "living-room-cycles-1080p.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "warning",
            "-y",
            "-framerate",
            "24",
            "-i",
            str(sequence / "%04d.png"),
            "-frames:v",
            "144",
            "-vf",
            "fade=t=in:st=0:d=0.20,fade=t=out:st=5.70:d=0.30",
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "16",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-colorspace",
            "bt709",
            "-metadata",
            "title=Villa Anahid / Living Room / Cycles study",
            str(movie),
        ],
        check=True,
    )
    probe = json.loads(
        subprocess.check_output(
            [
                "ffprobe",
                "-v",
                "error",
                "-count_frames",
                "-show_entries",
                "stream=codec_name,width,height,r_frame_rate,nb_read_frames,pix_fmt:format=duration,size",
                "-of",
                "json",
                str(movie),
            ]
        )
    )
    stream = probe["streams"][0]
    assert (stream["width"], stream["height"]) == (1920, 1080)
    assert stream["nb_read_frames"] == "144" and stream["r_frame_rate"] == "24/1"
    assert abs(float(probe["format"]["duration"]) - 6) < 0.01
    subprocess.run(["ffmpeg", "-v", "error", "-i", str(movie), "-f", "null", "-"], check=True)
    # Keep the independent art/render source portable; packed assets need no cache.
    for name in ["scene-report.json", "asset-provenance.json"]:
        shutil.copy2(build_dir / name, output_dir / name)
    proof = build_dir / "stills/0001.png"
    if proof.exists():
        shutil.copy2(proof, output_dir / "living-room-4k.png")
    timings = [report["frames"][str(f)]["seconds"] for f in expected]
    result = {
        "movie": str(movie),
        "sha256": sha(movie),
        "probe": probe,
        "rendered_frames": 144,
        "source_frames": expected,
        "output_fps": 24,
        "duration_s": 6,
        "source_timeline_fps": 24,
        "source_stride": 2,
        "camera_retime": "2x / no interpolated or duplicated frames",
        "render_settings": report["settings"],
        "render_seconds": sum(timings),
        "frame_seconds_min": min(timings),
        "frame_seconds_max": max(timings),
        "decode_pass": True,
        "all_frame_hashes_pass": True,
    }
    if proof.exists():
        result["still"] = {"path": str(output_dir / "living-room-4k.png"), "sha256": sha(proof)}
    (output_dir / "delivery-report.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-dir", type=Path, default=BUILD)
    parser.add_argument("--output-dir", type=Path, default=DIST)
    args = parser.parse_args()
    print(json.dumps(package_video(args.build_dir.resolve(), args.output_dir.resolve()), indent=2))


if __name__ == "__main__":
    main()
