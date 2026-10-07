"""Export a group of models from one frozen .blend for the Rust build pipeline.

Scene export lives here; statistics and validation use the shared native core.
The Rust caller supplies identifiers and output paths, checks dependencies,
converts bounds to millimetres, and independently verifies both asset descriptors
and GLB metrics. The source is never saved by this helper.
"""

import argparse
import json
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).with_name("rou_blender")))
import core
from scene import export_model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    request = json.loads(args.request.read_bytes())
    for model in request["models"]:
        if "entity_ids" not in model:
            raise ValueError("Build request is missing author entity IDs")
    report = {
        "format": request["response_format"],
        "models": [export_model(model, request, core) for model in request["models"]],
        "tool_versions": {
            "blender": bpy.app.version_string,
            "blender_build": bpy.app.build_hash.decode(),
            "exporter": request["response_format"],
        },
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print("ROU_EXPORT_COMPLETE", len(report["models"]), flush=True)


if __name__ == "__main__":
    main()
