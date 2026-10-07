"""Exercise LOD sharing, distinct modifier results and per-object material overrides."""

import hashlib
import json
import struct
import sys
import tempfile
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from toolchain.blender.rou_blender import scene


def build(kind):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16)
    base = bpy.context.object
    group = bpy.data.collections.new("entry")
    bpy.context.scene.collection.children.link(group)
    if kind == "collection_instances":
        prototype = bpy.data.collections.new("prototype")
        prototype.objects.link(base)
        for owner in list(base.users_collection):
            if owner != prototype:
                owner.objects.unlink(base)
        for index in range(20):
            obj = bpy.data.objects.new(f"instance_{index}", None)
            obj.instance_type = "COLLECTION"
            obj.instance_collection = prototype
            obj.location = (index * 3, 0, 0)
            group.objects.link(obj)
        return group
    materials = []
    if kind == "material_overrides":
        materials = [bpy.data.materials.new(name) for name in ["Red", "Blue"]]
        for material, color in zip(materials, [(1, 0, 0, 1), (0, 0, 1, 1)]):
            material.use_nodes = True
            material.node_tree.nodes.get("Principled BSDF").inputs[
                "Base Color"
            ].default_value = color
        base.data.materials.append(materials[0])
    count = 2 if kind in {"modifiers", "material_overrides"} else 20
    for index in range(count):
        data = base.data.copy() if kind == "independent" else base.data
        obj = bpy.data.objects.new(f"copy_{index}", data)
        obj.location = (index * 3, 0, 0)
        group.objects.link(obj)
        if kind == "modifiers":
            modifier = obj.modifiers.new("subdivision", "SUBSURF")
            modifier.levels = modifier.render_levels = index + 1
        if materials:
            obj.material_slots[0].link = "OBJECT"
            obj.material_slots[0].material = materials[index]
    bpy.data.objects.remove(base, do_unlink=True)
    return group


def main():
    checks = 0

    def check(condition, message):
        nonlocal checks
        if not condition:
            raise AssertionError(message)
        checks += 1

    for kind in [
        "shared",
        "independent",
        "modifiers",
        "material_overrides",
        "collection_instances",
    ]:
        group = build(kind)
        objects_before = set(bpy.data.objects.keys())
        meshes_before = set(bpy.data.meshes.keys())
        modifiers_before = {
            obj.name: [(mod.type, getattr(mod, "levels", None)) for mod in obj.modifiers]
            for obj in bpy.data.objects
        }
        with tempfile.TemporaryDirectory(prefix="rou-lod1-instancing-") as directory:
            root = Path(directory)
            source = root / "source.blend"
            bpy.ops.wm.save_as_mainfile(filepath=str(source))
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            result = scene.export_model(
                {
                    "id": "probe",
                    "entry": {"kind": "collection", "name": group.name},
                    "asset_path": "runtime/probe.glb",
                    "lod1_path": "runtime/probe.lod1.glb",
                },
                {"source_root": directory, "source": source.name, "output_dir": directory},
            )
            full, lod1 = result["metrics"]["asset"], result["metrics"]["lod1"]
            check(lod1["meshes"] <= full["meshes"], f"{kind}: mesh sharing was lost")
            check(lod1["indices"] <= full["indices"], f"{kind}: index data increased")
            check(lod1["indices"] < full["indices"] * 0.4, f"{kind}: geometry was not reduced")
            check(lod1["bytes"] < full["bytes"], f"{kind}: simplified output is larger")
            data = (root / result["lod1"]["path"]).read_bytes()
            length = struct.unpack_from("<I", data, 12)[0]
            document = json.loads(data[20 : 20 + length])
            if kind in {"shared", "collection_instances"}:
                check(full["meshes"] == lod1["meshes"] == 1, f"{kind}: expected one shared mesh")
                check(
                    sum("mesh" in node for node in document["nodes"]) == 20,
                    f"{kind}: instance placements were lost",
                )
            if kind == "modifiers":
                counts = {
                    document["accessors"][mesh["primitives"][0]["attributes"]["POSITION"]]["count"]
                    for mesh in document["meshes"]
                }
                check(len(counts) == 2, "Different subdivision results were merged")
            if kind == "material_overrides":
                check(
                    {material["name"] for material in document["materials"]} == {"Red", "Blue"},
                    "Different effective materials were merged",
                )
            check(hashlib.sha256(source.read_bytes()).hexdigest() == digest, "Saved source changed")
            print("LOD1_INSTANCING_CASE", json.dumps({"case": kind, **result["metrics"]}))
        check(set(bpy.data.objects.keys()) == objects_before, f"{kind}: temporary objects remain")
        check(set(bpy.data.meshes.keys()) == meshes_before, f"{kind}: temporary meshes remain")
        check(
            {
                obj.name: [(mod.type, getattr(mod, "levels", None)) for mod in obj.modifiers]
                for obj in bpy.data.objects
            }
            == modifiers_before,
            f"{kind}: author modifiers changed",
        )
    print("LOD1_INSTANCING_PASS", checks)


if __name__ == "__main__":
    main()
