"""Check the house's source-derived program, actual sofa facing, and shipped GLBs."""

import argparse
import collections
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build.native import load_core, validate_bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scene", type=Path, default=ROOT / "build/walsh-v3-2.1")
    path = parser.parse_args().scene.resolve()
    audit = json.loads((path / "room-audit.json").read_text())
    manifest = validate_bundle(path / "preview/realm.json")
    actual = audit["source_geometry"]
    beds = actual["beds"]
    assert len(beds) == 7, ("actual bed assemblies", len(beds))
    distribution = collections.Counter()
    for b in beds:
        residence = "adu" if b["entity"].startswith("walsh_adu_") else "main"
        distribution[(residence, round(b["position"][1], 1))] += 1
        matches = [r for r in audit["beds"] if math.dist(r["position"], b["position"]) < 0.005]
        assert len(matches) == 1, ("bed not represented once in floor schedule", b)
        room = next(r for r in audit["rooms"] if r["id"] == matches[0]["room"])
        x, z, w, d = room["rect"]
        p = b["position"]
        assert x < p[0] < x + w and z < p[2] < z + d, ("bed outside assigned room", b, room)
    assert distribution == {("main", 4.2): 1, ("main", 8.4): 4, ("adu", 8.4): 2}, distribution
    assert len(actual["sofas"]) >= 7
    for sofa in actual["sofas"]:
        assert sofa["actual_front_dot_target"] > 0.95, (
            "seat/back geometry faces away from table",
            sofa,
        )
    assert all(d["clear_width_m"] >= 1.0 for d in audit["doors"])
    rooms = {r["id"]: r for r in audit["rooms"]}
    for key, floor in [
        ("kitchen", 2),
        ("chef", 2),
        ("family", 2),
        ("suite_l2", 2),
        ("welcome", 1),
        ("cinema", 1),
        ("primary", 3),
        ("upper_lounge", 3),
        ("adu_living", "ADU"),
    ]:
        assert rooms[key]["floor"] == floor, (key, rooms[key])
    sample = json.loads((path / "sample.json").read_text())
    assert len(sample["waypoints"]) == 16
    assert sample["waypoints"][4]["name"] == "The main kitchen"
    assert "entries" not in manifest and "listing" not in manifest
    core = load_core()
    metrics = [
        core.call("glb.precheck", (path / "preview" / entity["asset"]["path"]).read_bytes())
        for entity in manifest["entities"]
    ]
    result = {
        "bed_distribution": {f"{r}@{y}m": n for (r, y), n in distribution.items()},
        "sofas_checked_from_mesh_centroids": len(actual["sofas"]),
        "minimum_facing_dot": min(s["actual_front_dot_target"] for s in actual["sofas"]),
        "door_openings": len(audit["doors"]),
        "entities_validated": len(manifest["entities"]),
        "runtime_bytes": sum(e["asset"]["bytes"] for e in manifest["entities"]),
        "vertices": sum(m["vertices"] for m in metrics),
        "max_entity_bytes": max(e["asset"]["bytes"] for e in manifest["entities"]),
        "max_entity_vertices": max(m["vertices"] for m in metrics),
    }
    (path / "checks.json").write_text(json.dumps(result, indent=2) + "\n")
    print("WALSH_V3_PASS", json.dumps(result))


if __name__ == "__main__":
    main()
