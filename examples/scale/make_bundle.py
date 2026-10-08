"""Create a deterministic synthetic preview using verified assets, without Blender.

By default three calibration models are placed ``--count`` times. ``--unique N``
derives N byte-distinct GLB variants (identical geometry and textures, a tagged
``asset.generator``) so streaming sees N different SHAs; ``--lod1-only`` uses
each source lod1 for both tiers, which keeps textured scale bundles small.
"""

import argparse
import hashlib
import json
import random
import shutil
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.native import ffi_library, load_core, validate_bundle


def variant(data: bytes, index: int) -> bytes:
    """Return a byte-distinct GLB with identical geometry and images, tagged in asset.generator."""
    magic, version, _total, length, kind = struct.unpack_from("<5I", data, 0)
    document = json.loads(data[20 : 20 + length])
    generator = document.setdefault("asset", {}).get("generator", "")
    document["asset"]["generator"] = f"{generator} scale-variant-{index:04d}".strip()
    chunk = json.dumps(document, separators=(",", ":")).encode()
    chunk += b" " * (-len(chunk) % 4)
    rest = data[20 + length :]
    header = struct.pack("<5I", magic, version, 20 + len(chunk) + len(rest), len(chunk), kind)
    return header + chunk + rest


def source_models(manifest: dict) -> list:
    by_id = {entity["id"]: entity for entity in manifest["entities"]}
    calibration = ["axis_probe", "house_0_1", "tree_00"]
    if all(name in by_id for name in calibration):
        return [by_id[name] for name in calibration]
    return list(manifest["entities"])


def generate(
    source: Path, output: Path, count: int, seed: int, unique: int = 0, lod1_only: bool = False
) -> dict:
    ffi_library()
    core = load_core()
    source_manifest = validate_bundle(source, core)
    models = source_models(source_manifest)
    profile = core.CONFIG["profile"]
    output.mkdir(parents=True, exist_ok=True)
    objects = output / "objects"
    objects.mkdir(exist_ok=True)
    payloads = {}
    if lod1_only:
        models = [{**model, "asset": model["lod1"]} for model in models]
    if unique:
        variants = []
        for index in range(unique):
            base = models[index % len(models)]
            model = dict(base)
            for key in ["asset", "lod1"]:
                data = variant((source.parent / base[key]["path"]).read_bytes(), index)
                sha = hashlib.sha256(data).hexdigest()
                model[key] = {
                    "path": core.call("asset.path", sha),
                    "sha256": sha,
                    "bytes": len(data),
                }
                payloads[model[key]["path"]] = data
            variants.append(model)
        models = variants
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
    if (output / "realm.json").exists():
        existing = core.read_json(output / "realm.json", "manifest.validate")
        if existing != manifest or (output / "version.json").exists():
            raise SystemExit("Output is not this generated preview; use an empty output directory")
    for model in models:
        for key in ["asset", "lod1"]:
            path = model[key]["path"]
            if path in payloads:
                (output / path).write_bytes(payloads[path])
            else:
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
        "unique": unique,
        "lod1_only": lod1_only,
        "source": str(source),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT / "build/s1-2.1/preview/realm.json")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--count", type=int, default=800)
    parser.add_argument("--seed", type=int, default=20260922)
    parser.add_argument(
        "--unique", type=int, default=0, help="byte-distinct GLB variants to derive"
    )
    parser.add_argument("--lod1-only", action="store_true", help="use each lod1 for both tiers")
    args = parser.parse_args()
    if not args.source.is_file():
        parser.error(
            "Generate calibration first: blender --background --python examples/calibration/create_scene.py"
        )
    if args.count < 1:
        parser.error("--count must be positive; the core checks the catalog budget")
    output = args.output or ROOT / f"build/scale/bundle-{args.count}"
    print(
        json.dumps(
            generate(
                args.source.resolve(),
                output.resolve(),
                args.count,
                args.seed,
                unique=args.unique,
                lod1_only=args.lod1_only,
            )
        )
    )


if __name__ == "__main__":
    main()
