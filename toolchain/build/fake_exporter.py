#!/usr/bin/env python3
"""Guarded deterministic exporter for scale_collab placeholders, never author projects."""

import json
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from examples.scale.make_bundle import variant
from toolchain.build.native import ffi_library, load_core


def templates() -> tuple[Path, Path]:
    """Find a complete pair from a real calibration acceptance run."""
    for run in sorted((ROOT / "build/s1-git/verification").glob("run-*"), reverse=True):
        pair = tuple(run / "official/runtime" / name for name in ("house.glb", "plaque.glb"))
        if all(p.is_file() and p.read_bytes()[:4] == b"glTF" for p in pair):
            return pair
    raise FileNotFoundError(
        "Calibration GLB templates missing; run make test-git without fake mode"
    )


def main() -> None:
    if os.environ.get("ROU_FAKE_EXPORTER") != "1":
        raise SystemExit("Fake exporter refused: ROU_FAKE_EXPORTER=1 is required")
    request = json.loads(Path(sys.argv[sys.argv.index("--request") + 1]).read_text())
    source_root = Path(request["source_root"])
    source = json.loads((source_root / request["source"]).read_text())
    if source.get("format") != "rou-collab-placeholder/1":
        raise SystemExit("Fake exporter accepts only scale_collab placeholder sources")
    ffi_library()
    core = load_core()
    # The request has no dependency lists; the generated placeholder carries them.
    pair = templates()
    models = []
    for model in request["models"]:
        dependencies = source["models"][model["id"]]["dependencies"]
        if not all((source_root / path).is_file() for path in dependencies):
            raise FileNotFoundError("A placeholder dependency was not frozen")
        result = {
            "id": model["id"],
            "metrics": {},
            "textures": {},
            "bounds_blender_m": [[0, 0, 0], [1, 1, 1]],
            "dependencies": dependencies,
        }
        for tier, path_key, template in zip(
            ("asset", "lod1"), ("asset_path", "lod1_path"), pair, strict=True
        ):
            data = variant(template.read_bytes(), model["id"])
            path = Path(request["output_dir"]) / model[path_key]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            for uri in core.call("glb.inspect", data)["metrics"]["images"]:
                shutil.copyfile(template.parent / uri, path.parent / uri)
            inspected = core.inspect_glb(path, "runtime/")
            result["textures"][tier] = inspected["textures"]
            result[tier] = {"path": model[path_key], **inspected["descriptor"]}
            result["metrics"][tier] = inspected["metrics"]
        models.append(result)
    output = {
        "format": request["response_format"],
        "models": models,
        "tool_versions": {"exporter": request["response_format"], "blender": "fake exporter"},
    }
    Path(sys.argv[sys.argv.index("--output") + 1]).write_text(json.dumps(output) + "\n")


if __name__ == "__main__":
    main()
