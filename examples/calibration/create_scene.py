"""Generate an original CC0 neighborhood and export actual Blender-authored GLBs."""

import argparse
import faulthandler
import json
import sys
from pathlib import Path

import bpy

faulthandler.enable()
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from examples.common.blender_primitives import cone, cube, cylinder, material, roof, root
from toolchain.blender.rou_blender import core, exporter
from toolchain.build.compare_grids import with_grid


def build(output):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    mats = {
        "sand": material("Warm limestone", (0.74, 0.66, 0.49)),
        "white": material("Ivory", (0.89, 0.84, 0.68)),
        "roof": material("Terracotta", (0.55, 0.20, 0.105)),
        "road": material("Slate paving", (0.17, 0.22, 0.25)),
        "green": material("Sage", (0.23, 0.43, 0.30)),
        "leaf": material("Tree canopy", (0.18, 0.39, 0.25)),
        "leaf2": material("Light canopy", (0.39, 0.53, 0.26)),
        "wood": material("Cedar", (0.29, 0.15, 0.08)),
        "glass": material("Blue glass", (0.12, 0.32, 0.38), 0.25, 0.25),
        "water": material("Fountain water", (0.12, 0.53, 0.57), 0.2),
        "gold": material("Brass", (0.93, 0.62, 0.23), 0.4, 0.25),
        "red": material("Axis X", (0.86, 0.22, 0.18)),
        "blue": material("Axis Z", (0.18, 0.38, 0.8)),
        "axis_y": material("Axis Y", (0.18, 0.8, 0.3)),
    }
    image = bpy.data.images.new("Original paving checker", width=64, height=64)
    pixels = []
    for y in range(64):
        for x in range(64):
            c = 0.68 if ((x // 8 + y // 8) % 2) else 0.84
            pixels.extend([c, c * 0.95, c * 0.86, 1.0])
    image.pixels = pixels
    image.pack()
    tex = mats["sand"].node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    mats["sand"].node_tree.links.new(
        tex.outputs["Color"],
        mats["sand"].node_tree.nodes.get("Principled BSDF").inputs["Base Color"],
    )
    # Chunk-aligned road segments preserve source units; central plaza is one cross-chunk entity.
    for i in range(-3, 4):
        for axis in [0, 2]:
            p = [0.0, 0.0, 0.0]
            p[axis] = i * 28
            o = root(f"road_{axis}_{i + 3}", f"Promenade {axis} / {i + 3}", p)
            size = [10, 0.2, 10]
            size[axis] = 28
            cube(o, "Slate lane", (0, 0.1, 0), size, mats["road"])
            for k in [-1, 1]:
                pp = [0, 0.22, 0]
                pp[2 if axis == 0 else 0] = k * 5.4
                ss = [0.8, 0.25, 0.8]
                ss[axis] = 28
                cube(o, "Limestone kerb", pp, ss, mats["sand"])
    colors = ["white", "sand", "green", "white", "sand", "white"]
    for row, z in enumerate([-70, -40, 40, 70]):
        for col, x in enumerate([-78, -48, -18, 18, 48, 78]):
            if (row + col) % 7 == 0:
                continue
            o = root(f"house_{row}_{col}", f"Atelier {row + 1}.{col + 1}", (x, 0, z))
            h = 6 + (col % 3) * 1.2
            cube(o, "Courtyard", (0, 0.1, 0), (22, 0.2, 22), mats["sand"])
            cube(o, "House", (0, h / 2 + 0.2, 0), (12, h, 10), mats[colors[col]], 0.12)
            roof(o, 13, 11, 3, h + 0.2, mats["roof"])
            cube(o, "Door", (0, 1.6, 5.08), (1.6, 3, 0.15), mats["wood"])
            for wx in [-3.5, 3.5]:
                cube(o, "Window surround", (wx, 3.2, 5.09), (2.4, 2.8, 0.2), mats["white"])
                cube(o, "Glazing", (wx, 3.2, 5.23), (1.9, 2.3, 0.12), mats["glass"])
            cube(o, "Chimney", (3, h + 2, 0), (1.3, 3, 1.3), mats["sand"])
    o = root("plaza", "Commons / fountain", (0, 0, 0))
    cylinder(o, "Plaza", (0, 0.12, 0), 14, 0.24, mats["sand"], 32)
    cylinder(o, "Basin", (0, 0.55, 0), 4.2, 0.8, mats["white"], 24)
    cylinder(o, "Water", (0, 1.0, 0), 3.8, 0.1, mats["water"], 24)
    cylinder(o, "Central column", (0, 2.5, 0), 0.7, 3, mats["sand"])
    cylinder(o, "Top bowl", (0, 4.0, 0), 1.6, 0.3, mats["gold"], 16)
    for i, (x, z) in enumerate(
        [
            (-9, -9),
            (9, -9),
            (-9, 9),
            (9, 9),
            (-64, 18),
            (-32, 18),
            (32, 18),
            (64, 18),
            (-64, -18),
            (-32, -18),
            (32, -18),
            (64, -18),
        ]
    ):
        o = root(f"tree_{i:02}", f"Garden tree {i + 1}", (x, 0, z))
        cylinder(o, "Trunk", (0, 2, 0), 0.45, 4, mats["wood"])
        cone(o, "Lower crown", (0, 5, 0), 3.2, 6, mats["leaf"])
        cone(o, "Upper crown", (0, 7.2, 0), 2.3, 4.5, mats["leaf2"])
    o = root("cross_chunk_pavilion", "Cross-chunk gallery", (-14, 0, -22))
    cube(o, "Gallery floor", (0, 0.2, 0), (42, 0.4, 9), mats["sand"])
    for x in [-19, -10, 0, 10, 19]:
        for z in [-3, 3]:
            cube(o, "Column", (x, 3.2, z), (0.75, 6, 0.75), mats["white"], 0.08)
    cube(o, "Roof", (0, 6.5, 0), (44, 0.6, 11), mats["green"], 0.16)
    # Three unequal colored axes verify handedness, scale and exactly one coordinate conversion.
    o = root("axis_probe", "Axis probe / 1m X, 2m Y, 3m Z", (-105, 0, -100))
    cube(o, "X 1m", (0.5, 0.1, 0), (1, 0.2, 0.2), mats["red"])
    cube(o, "Y 2m", (0, 1, 0), (0.2, 2, 0.2), mats["axis_y"])
    cube(o, "Z 3m", (0, 0.1, 1.5), (0.2, 0.2, 3), mats["blue"])
    bpy.context.scene["rou_profile_json"] = json.dumps(core.CONFIG["profile"])
    bpy.context.scene["rou_title"] = "COMMON GROUND / \u5171\u540c\u8857\u533a"
    bpy.context.scene["rou_license"] = "CC0-1.0"
    bpy.context.view_layer.update()
    output.mkdir(parents=True, exist_ok=True)
    print("ROU_FIXTURE_BUILT", len(bpy.data.objects), flush=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "neighborhood.blend"))
    print("ROU_SOURCE_SAVED", flush=True)
    exporter.export_scene(output / "preview")
    # The same entity geometry and IDs are indexed under each experimental grid size.
    raw = core.read_bundle(output / "preview/realm.json")
    for size in [16, 32, 64]:
        projected = with_grid(raw, size, core)
        drafts = [
            {key: value for key, value in entity.items() if key not in {"owner_chunk", "offset_mm"}}
            for entity in projected["entities"]
        ]
        bundle = core.call("preview.create", {"profile": projected["profile"], "entities": drafts})
        directory = output / f"grid-{size}"
        import shutil

        shutil.copytree(output / "preview/objects", directory / "objects", dirs_exist_ok=True)
        core.write_bundle(directory, bundle)
    print("ROU_FIXTURE_COMPLETE", str(output / "preview/realm.json"))


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("--output", default=str(ROOT / "build/s1-2.1"))
    a = ap.parse_args(args)
    build(Path(a.output).resolve())
