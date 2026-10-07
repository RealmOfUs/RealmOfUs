"""Fetch CC0 assets, create a detailed Git realm, build both tiers and validate a browse bundle.

Requires make all and Blender. The output must be empty. Failed downloads or
validation abort the run; no missing asset is silently substituted or skipped.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from examples.villa_anahid.offline.fetch_assets import download_assets
from toolchain.build.native import cli, ffi_library, load_core, validate_bundle


def generate(output):
    binary = cli()
    ffi_library()
    core = load_core()
    config = core.call("config")
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise ValueError("Choose an empty output directory for the detailed realm")
    records = download_assets(
        ["wool_boucle", "ArmChair_01", "WoodenChair_01"], "4k", "m3-provenance.json"
    )
    environment = {**os.environ, "ROU_TOOLCHAIN_ROOT": str(ROOT)}

    def command(arguments, log):
        result = subprocess.run(
            arguments, cwd=output, env=environment, capture_output=True, text=True, timeout=900
        )
        (output / ".rou" / log).write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError(f"Command failed: {arguments}; see {output / '.rou' / log}")
        return result.stdout

    subprocess.run([str(binary), "init"], cwd=output, env=environment, check=True)
    core.write_json(output / "asset-provenance.json", records)
    command(
        [
            os.environ.get("ROU_BLENDER", "blender"),
            "--background",
            "--factory-startup",
            "--python-exit-code",
            "1",
            "--python",
            str(ROOT / "examples/scale/create_detailed_scene.py"),
            "--",
            "--repo",
            str(output),
            "--provenance",
            str(output / "asset-provenance.json"),
        ],
        "detailed-source.log",
    )
    (output / "README.md").write_text(
        "# Detailed CC0 sample\n\nPoly Haven ArmChair_01, WoodenChair_01 and wool_boucle (4K).\n"
        "Furniture uses SIMPLE subdivision to stress tessellation while preserving the scanned shape.\n"
        "The woven panel is original procedural geometry with Poly Haven wool textures.\n"
        "Provenance includes provider URLs and verified hashes. Cached originals are never modified.\n"
    )
    sources = {
        p: hashlib.sha256(p.read_bytes()).hexdigest() for p in (output / "sources").rglob("*.blend")
    }
    built = json.loads(command([str(binary), "--json", "build"], "detailed-build.json"))
    if any(hashlib.sha256(p.read_bytes()).hexdigest() != digest for p, digest in sources.items()):
        raise AssertionError("Build modified author sources")
    command([str(binary), "add", "."], "detailed-add.log")
    validated = json.loads(
        command([str(binary), "--json", "validate", "--staged"], "detailed-validate.json")
    )
    viewed = json.loads(
        command([str(binary), "--json", "view", "--worktree", "--all"], "detailed-view.json")
    )
    manifest = validate_bundle(Path(viewed["manifest"]), core)
    for model, metrics in built["metrics"].items():
        if metrics["lod1"]["indices"] >= metrics["asset"]["indices"] * 0.4:
            raise AssertionError(f"LOD triangle reduction missing: {model}")
        if (
            metrics["asset"]["max_image_dimension"]
            < config["limits"]["reference"]["image_warn_dimension"]
        ):
            raise AssertionError(f"4K textures missing: {model}")
        if metrics["lod1"]["max_image_dimension"] > config["lod1"]["image_dimension"]:
            raise AssertionError(f"LOD texture reduction missing: {model}")
        if metrics["lod1"]["image_decoded_bytes"] >= metrics["asset"]["image_decoded_bytes"]:
            raise AssertionError(f"LOD decoded texture size did not decrease: {model}")
    result = {
        "build": built,
        "validation": validated,
        "view": viewed,
        "profile_hash": manifest["profile_hash"],
        "sources_unchanged": True,
        "provenance": records,
    }
    (output / ".rou/detailed-report.json").write_text(json.dumps(result, indent=2) + "\n")
    print(
        json.dumps(
            {
                "realm": viewed["manifest"],
                "metrics": built["metrics"],
                "warnings": built["warnings"],
            },
            indent=2,
        )
    )
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "build/scale/detailed")
    args = parser.parse_args()
    generate(args.output.resolve())


if __name__ == "__main__":
    main()
