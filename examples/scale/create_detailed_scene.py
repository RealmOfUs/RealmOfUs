"""Create three detailed static models from cached CC0 assets in a new Git realm."""

import argparse
import json
import math
import sys
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.blender.rou_blender import core


def save_model(repository, model_id, meshes, position):
    """Pack textures and save a complete source without modifying cached originals."""
    parent = bpy.data.objects.new(model_id, None)
    bpy.context.scene.collection.objects.link(parent)
    for obj in meshes:
        matrix = obj.matrix_world.copy()
        obj.parent = parent
        obj.matrix_world = matrix
        obj.hide_render = False
        obj.hide_viewport = False
        obj.hide_set(False)
        # The sample exercises static core PBR, which excludes glTF extensions.
        for material in obj.data.materials:
            if material is None or not material.use_nodes:
                continue
            for node in material.node_tree.nodes:
                if node.type == "BSDF_PRINCIPLED":
                    for name, value in {
                        "IOR": 1.5,
                        "Specular IOR Level": 0.5,
                        "Coat Weight": 0.0,
                        "Sheen Weight": 0.0,
                        "Transmission Weight": 0.0,
                    }.items():
                        for link in list(node.inputs[name].links):
                            material.node_tree.links.remove(link)
                        node.inputs[name].default_value = value
    bpy.context.view_layer.update()
    source = f"sources/{model_id}/scene.blend"
    path = repository / source
    path.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.file.pack_all()
    bpy.ops.wm.save_as_mainfile(filepath=str(path), relative_remap=True)
    core.write_json(
        repository / f"realm/models/{model_id}.json",
        {
            "format": core.FORMATS["model"],
            "id": model_id,
            "source": source,
            "entry": {"kind": "object", "name": model_id},
            "dependencies": [],
            "settings": {},
        },
    )
    core.write_json(
        repository / f"realm/entities/{model_id}.json",
        {
            "format": core.FORMATS["entity"],
            "id": model_id,
            "model": model_id,
            "position_mm": position,
            "label": model_id.replace("_", " ").title(),
        },
    )


def cached(provenance, asset_id, suffix):
    """Resolve one verified download by its recorded path, never by a naming convention."""
    paths = [
        ROOT / record["path"]
        for record in provenance
        if record["asset"] == asset_id and record["path"].endswith(suffix)
    ]
    if len(paths) != 1:
        raise ValueError(f"Expected one {suffix} for {asset_id} in provenance, found {len(paths)}")
    return paths[0]


def furniture(provenance, repository, asset_id, model_id, position):
    source = cached(provenance, asset_id, ".blend")
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
    if not meshes:
        raise ValueError(f"No furniture meshes in {source}")
    total = sum(sum(len(face.vertices) - 2 for face in obj.data.polygons) for obj in meshes)
    levels = 0
    while total * 4**levels < 300_000 and levels < 4:
        levels += 1
    # Explicit tessellation stress: preserve the scanned shape and UVs, without claiming new detail.
    for obj in meshes:
        modifier = obj.modifiers.new("Detailed sample tessellation", "SUBSURF")
        modifier.subdivision_type = "SIMPLE"
        modifier.levels = modifier.render_levels = levels
    save_model(repository, model_id, meshes, position)


def panel(provenance, repository):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    nx, ny = 1024, 512
    points = [
        (4 * x / nx - 2, 0.012 * math.sin(x * 0.37) * math.sin(y * 0.41), 2 * y / ny)
        for y in range(ny + 1)
        for x in range(nx + 1)
    ]
    faces = [
        (y * (nx + 1) + x, y * (nx + 1) + x + 1, (y + 1) * (nx + 1) + x + 1, (y + 1) * (nx + 1) + x)
        for y in range(ny)
        for x in range(nx)
    ]
    mesh = bpy.data.meshes.new("Woven panel geometry")
    mesh.from_pydata(points, [], faces)
    mesh.update()
    uv = mesh.uv_layers.new()
    for polygon in mesh.polygons:
        polygon.use_smooth = True
        for loop in polygon.loop_indices:
            point = mesh.vertices[mesh.loops[loop].vertex_index].co
            uv.data[loop].uv = ((point.x + 2) / 4, point.z / 2)
    material = bpy.data.materials.new("Poly Haven wool boucle 4K")
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    for channel, suffix, socket in [
        ("Diffuse", "diff", "Base Color"),
        ("Rough", "rough", "Roughness"),
        ("nor_gl", "nor_gl", "Normal"),
    ]:
        node = material.node_tree.nodes.new("ShaderNodeTexImage")
        node.image = bpy.data.images.load(
            str(cached(provenance, "wool_boucle", f"_{suffix}_4k.jpg"))
        )
        if channel != "Diffuse":
            node.image.colorspace_settings.name = "Non-Color"
        output = node.outputs["Color"]
        if socket == "Normal":
            normal = material.node_tree.nodes.new("ShaderNodeNormalMap")
            material.node_tree.links.new(output, normal.inputs["Color"])
            output = normal.outputs["Normal"]
        material.node_tree.links.new(output, shader.inputs[socket])
    mesh.materials.append(material)
    obj = bpy.data.objects.new("Woven panel", mesh)
    bpy.context.scene.collection.objects.link(obj)
    save_model(repository, "woven_panel", [obj], [48000, 0, 0])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    repository = args.repo.resolve()
    provenance = json.loads(args.provenance.read_text())
    furniture(provenance, repository, "ArmChair_01", "armchair", [-48000, 0, 0])
    furniture(provenance, repository, "WoodenChair_01", "wooden_chair", [0, 0, 0])
    panel(provenance, repository)
    print("DETAILED_SCENE_CREATED", json.dumps({"models": 3, "repository": str(repository)}))


if __name__ == "__main__":
    main()
