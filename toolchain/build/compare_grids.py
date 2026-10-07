"""Measure reviewed calibration positions; spatial operations use rou_core."""

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.native import load_core


def with_grid(manifest: dict, edge_m: int, core=None) -> dict:
    core = core or load_core()
    return core.call(
        "preview.create",
        {
            "profile": core.call("profile.calibration", edge_m),
            "entities": [
                {
                    key: value
                    for key, value in entity.items()
                    if key in {"id", "label", "position_mm", "bounds_mm", "asset", "lod1"}
                }
                for entity in manifest["entities"]
            ],
        },
    )


def compare_grids(fixture: dict) -> list[dict]:
    core = load_core()
    # Historical geometry/position vectors stay read-only. Add only the current schema fields.
    catalog = {**fixture["manifest"], "format": core.FORMATS["preview"]}
    catalog["entities"] = [{**entity, "lod1": entity["asset"]} for entity in catalog["entities"]]
    manifest = core.call("manifest.validate", catalog)
    results = []
    for edge_m in (16, 32, 64):
        variant = with_grid(manifest, edge_m, core)
        views = []
        for observation in fixture["observations"]:
            selection = core.call(
                "viewer.query",
                {
                    "manifest": variant,
                    "position": observation["position"],
                    "radii": [48, 48, 48],
                    "loaded": [],
                },
            )
            selected = set(selection["load"])
            views.append(
                {
                    "entry": observation["name"],
                    "chunks": selection["selected_chunks"],
                    "entities": len(selected),
                    "runtime_bytes": sum(
                        entity["asset"]["bytes"]
                        for entity in variant["entities"]
                        if entity["id"] in selected
                    ),
                }
            )
        results.append(
            {
                "chunk_m": edge_m,
                "nonempty_chunks": len(variant["chunks"]),
                "entity_references": sum(len(ids) for ids in variant["chunks"].values()),
                "catalog_bytes": len(core.call("json.canonical", variant).encode()),
                "assets_bytes": sum(entity["asset"]["bytes"] for entity in variant["entities"]),
                "views_48m_radius": views,
            }
        )
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fixture", type=Path, default=ROOT / "toolchain/fixtures/calibration.json"
    )
    parser.add_argument("--output", type=Path, default=ROOT / "build/s1-2.1/grid-comparison.json")
    args = parser.parse_args()
    results = compare_grids(load_core().read_json(args.fixture))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(results, indent=2) + "\n")
    print("S1_GRID_COMPARISON", args.output)


if __name__ == "__main__":
    main()
