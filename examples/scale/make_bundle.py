"""Create a deterministic synthetic preview using verified calibration assets, without Blender."""

import argparse
import json
import random
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.native import ffi_library, load_core, validate_bundle


def generate(source: Path, output: Path, count: int, seed: int) -> dict:
    ffi_library()
    core = load_core()
    source_manifest = validate_bundle(source, core)
    by_id = {entity["id"]: entity for entity in source_manifest["entities"]}
    models = [by_id[name] for name in ["axis_probe", "house_0_1", "tree_00"]]
    profile = core.CONFIG["profile"]
    rng = random.Random(seed)
    entities = []
    for index in range(count):
        model = models[index % len(models)]
        local = {
            end: [model["bounds_mm"][end][a] - model["position_mm"][a] for a in range(3)]
            for end in ["min", "max"]
        }
        position = [
            rng.randint(
                profile["origin_mm"][a]
                + profile["min_chunk"][a] * profile["chunk_size_mm"][a]
                - local["min"][a],
                profile["origin_mm"][a]
                + profile["max_chunk"][a] * profile["chunk_size_mm"][a]
                - local["max"][a]
                - 1,
            )
            if a != 1
            else 0
            for a in range(3)
        ]
        entities.append(
            {
                "id": f"scale_{index:04d}",
                "label": f"{model['id']} instance {index:04d}",
                "position_mm": position,
                "bounds_mm": {
                    end: [local[end][a] + position[a] for a in range(3)] for end in ["min", "max"]
                },
                "asset": model["asset"],
                "lod1": model["lod1"],
            }
        )
    # The core derives owners, offsets, profile hash and the complete spatial index.
    manifest = core.call("preview.create", {"profile": profile, "entities": entities})
    layout = core.call("viewer.catalog", manifest)
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        existing = core.read_json(output / "realm.json", "manifest.validate")
        if existing != manifest or (output / "version.json").exists():
            raise SystemExit("Output is not this generated preview; use an empty output directory")
    objects = output / "objects"
    objects.mkdir(exist_ok=True)
    for model in models:
        for key in ["asset", "lod1"]:
            path = model[key]["path"]
            shutil.copyfile(source.parent / path, output / path)
    core.write_json(output / "realm.json", manifest)
    validate_bundle(output / "realm.json", core)
    return {
        "realm": str(output / "realm.json"),
        "entities": len(entities),
        "unique_assets": len(
            {model[key]["sha256"] for model in models for key in ["asset", "lod1"]}
        ),
        "occupied_chunks": len(manifest["chunks"]),
        "impostors": len(layout["impostors"]),
        "seed": seed,
        "source": str(source),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT / "build/s1-2.1/preview/realm.json")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--count", type=int, default=800)
    parser.add_argument("--seed", type=int, default=20260922)
    args = parser.parse_args()
    if not args.source.is_file():
        parser.error(
            "Generate calibration first: blender --background --python examples/calibration/create_scene.py"
        )
    if args.count < 1:
        parser.error("--count must be positive; the core checks the catalog budget")
    output = args.output or ROOT / f"build/scale/bundle-{args.count}"
    print(json.dumps(generate(args.source.resolve(), output.resolve(), args.count, args.seed)))


if __name__ == "__main__":
    main()
