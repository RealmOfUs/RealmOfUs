"""Verify simplified geometry preserves effective material slots, UVs and author data."""

import hashlib
import json
import struct
import sys
import tempfile
from array import array
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from toolchain.blender.rou_blender import core, exporter, scene


def glb_json(path):
    data = path.read_bytes()
    length = struct.unpack_from("<I", data, 12)[0]
    return json.loads(data[20 : 20 + length])


def external_images(path):
    return {
        image["uri"]: (path.parent / image["uri"]).read_bytes()
        for image in glb_json(path).get("images", [])
    }


def author_state():
    return {
        "blocks": {
            name: set(getattr(bpy.data, name).keys())
            for name in ["objects", "meshes", "materials", "images", "node_groups", "collections"]
        },
        "images": {
            image.name: (
                tuple(image.size),
                image.filepath,
                image.source,
                image.is_dirty,
                image.colorspace_settings.name,
                image.alpha_mode,
                tuple(
                    image.pixels[offset + channel]
                    for offset in [0, len(image.pixels) // 2, len(image.pixels) - 4]
                    for channel in range(4)
                ),
            )
            for image in bpy.data.images
            if min(image.size) > 0
        },
        "nodes": {
            tree.as_pointer(): [
                (node.name, getattr(node, "image", None), getattr(node, "node_tree", None))
                for node in tree.nodes
            ]
            for tree in [
                *(material.node_tree for material in bpy.data.materials if material.node_tree),
                *bpy.data.node_groups,
            ]
        },
    }


def verify_texture_lod(check):
    limit = core.CONFIG["lod1"]["image_dimension"]
    for kind in ["dense_packed", "cube_linked_dirty", "cube_small"]:
        bpy.ops.wm.read_factory_settings(use_empty=True)
        if kind == "dense_packed":
            bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32)
        else:
            bpy.ops.mesh.primitive_cube_add()
        obj = bpy.context.object
        obj["rou_entity_id"] = "texture_probe"
        material = bpy.data.materials.new("Author material.001")
        material.use_nodes = True
        obj.data.materials.append(material)
        size = (16, 8) if kind == "cube_small" else (limit * 2, limit * 5 // 2)
        image = bpy.data.images.new("Color probe", *size)
        image.generated_color = (0.2, 0.5, 0.8, 1)
        with tempfile.TemporaryDirectory(prefix="rou-lod1-textures-") as directory:
            root = Path(directory)
            if kind == "cube_linked_dirty":
                other = bpy.data.materials.new("Author material.002")
                other.use_nodes = True
                library = root / "materials.blend"
                bpy.data.libraries.write(str(library), {other})
                bpy.data.materials.remove(other)
                with bpy.data.libraries.load(str(library), link=True) as (_, linked):
                    linked.materials = ["Author material.002"]
                obj.data.materials.append(linked.materials[0])
                for polygon in obj.data.polygons:
                    polygon.material_index = polygon.index % 2
                image.filepath_raw = str(root / "color.png")
                image.file_format = "PNG"
                image.save()
                bpy.data.images.remove(image)
                image = bpy.data.images.load(str(root / "color.png"))
                # Live paint edits differ from disk; the copied image must use these pixels.
                image.pixels.foreach_set(array("f", [0.8, 0.1, 0.2, 1]) * (size[0] * size[1]))
                group = bpy.data.node_groups.new("Author texture group", "ShaderNodeTree")
                group.interface.new_socket(
                    name="Color", in_out="OUTPUT", socket_type="NodeSocketColor"
                )
                output = group.nodes.new("NodeGroupOutput")
                texture = group.nodes.new("ShaderNodeTexImage")
                texture.image = image
                group.links.new(texture.outputs["Color"], output.inputs["Color"])
                nested = material.node_tree.nodes.new("ShaderNodeGroup")
                nested.node_tree = group
                material.node_tree.links.new(
                    nested.outputs["Color"],
                    material.node_tree.nodes.get("Principled BSDF").inputs["Base Color"],
                )
            else:
                texture = material.node_tree.nodes.new("ShaderNodeTexImage")
                texture.image = image
                material.node_tree.links.new(
                    texture.outputs["Color"],
                    material.node_tree.nodes.get("Principled BSDF").inputs["Base Color"],
                )
                image.pack()
            source = root / "source.blend"
            bpy.ops.wm.save_as_mainfile(filepath=str(source))
            before = author_state()
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            result = scene.export_model(
                {
                    "id": "texture_probe",
                    "entry": {"kind": "object", "name": obj.name},
                    "asset_path": "runtime/probe.glb",
                    "lod1_path": "runtime/probe.lod1.glb",
                },
                {"source_root": directory, "source": source.name, "output_dir": directory},
            )
            full, lod1 = result["metrics"]["asset"], result["metrics"]["lod1"]
            after = author_state()
            for key in before:
                if before[key] != after[key]:
                    print("AUTHOR_STATE_DIFFERENCE", kind, key, before[key], after[key])
            check(
                after == before,
                f"{kind}: author image pixels, dimensions or nodes changed",
            )
            check(
                hashlib.sha256(source.read_bytes()).hexdigest() == digest,
                f"{kind}: saved source changed",
            )
            check(full["max_image_dimension"] == max(size), f"{kind}: full image was resized")
            check(lod1["max_image_dimension"] <= limit, f"{kind}: lod1 image was not resized")
            paths = [root / result[tier]["path"] for tier in ["asset", "lod1"]]
            names = [{m["name"] for m in glb_json(path)["materials"]} for path in paths]
            expected_names = {slot.material.name for slot in obj.material_slots}
            check(names[0] == names[1] == expected_names, f"{kind}: material names changed")
            check(
                all(
                    "uri" in image and "bufferView" not in image
                    for path in paths
                    for image in glb_json(path).get("images", [])
                ),
                f"{kind}: embedded image remained",
            )
            full_images = {texture["sha256"] for texture in result["textures"]["asset"]}
            lod1_images = {texture["sha256"] for texture in result["textures"]["lod1"]}
            check(
                full_images == lod1_images
                if kind == "cube_small"
                else full_images.isdisjoint(lod1_images),
                f"{kind}: unexpected shared image set",
            )
            if kind == "cube_small":
                check(
                    result["asset"]["sha256"] == result["lod1"]["sha256"],
                    "Small geometry and image did not reuse bytes",
                )
                continue
            check(
                full["max_image_dimension"] > lod1["max_image_dimension"],
                f"{kind}: image sizes not distinct",
            )
            check(
                lod1["image_decoded_bytes"] < full["image_decoded_bytes"],
                f"{kind}: decoded image budget not reduced",
            )
            png = root / "lod1.png"
            png.write_bytes(next(iter(external_images(paths[1]).values())))
            resized = bpy.data.images.load(str(png))
            try:
                expected = tuple(max(1, round(edge * limit / max(size))) for edge in size)
                check(tuple(resized.size) == expected, f"{kind}: image aspect ratio changed")
                if kind == "cube_linked_dirty":
                    check(
                        abs(resized.pixels[0] - image.pixels[0]) < 0.02,
                        "Live paint edits were replaced by disk pixels",
                    )
            finally:
                bpy.data.images.remove(resized)
            if kind == "cube_linked_dirty":
                check(full["indices"] == lod1["indices"], "Small mesh geometry was simplified")
                preview = root / "preview"
                (preview / "objects").mkdir(parents=True)
                exported = exporter.export_entity(obj, preview)
                preview_metrics = core.inspect_glb(preview / exported["lod1"]["path"], "objects/")[
                    "metrics"
                ]
                check(
                    preview_metrics["max_image_dimension"] == limit,
                    "Live preview missed texture policy",
                )
                check(
                    author_state() == before, "Live preview modified author data or retained copies"
                )
            original_export = scene.export_glb

            def fail_export(_path):
                raise RuntimeError("Injected LOD export failure")

            try:
                scene.export_glb = fail_export
                try:
                    scene.export_lod1([obj], paths[0], root / "failed.glb", core.CONFIG["lod1"])
                    raise AssertionError("Expected LOD export failure")
                except RuntimeError as error:
                    check(str(error) == "Injected LOD export failure", "Unexpected export error")
            finally:
                scene.export_glb = original_export
            check(
                author_state() == before,
                f"{kind}: failed export changed author data or leaked copies",
            )
            print("LOD1_TEXTURE_CASE", json.dumps({"case": kind, **result["metrics"]}))


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, location=(4, 3, 2))
    obj = bpy.context.object
    materials = [bpy.data.materials.new(name) for name in ["Base", "Override", "Second"]]
    for material in materials:
        material.use_nodes = True
    image = bpy.data.images.new("UV probe", 2, 2)
    image.generated_color = (0.2, 0.5, 0.8, 1)
    texture = materials[1].node_tree.nodes.new("ShaderNodeTexImage")
    texture.image = image
    materials[1].node_tree.links.new(
        texture.outputs["Color"],
        materials[1].node_tree.nodes.get("Principled BSDF").inputs["Base Color"],
    )
    obj.data.materials.append(materials[0])
    obj.data.materials.append(materials[2])
    obj.material_slots[0].link = "OBJECT"
    obj.material_slots[0].material = materials[1]
    for polygon in obj.data.polygons:
        polygon.material_index = int(polygon.center.z > 0)
    before = {
        "location": tuple(obj.location),
        "faces": [polygon.material_index for polygon in obj.data.polygons],
        "uv": [tuple(value.uv) for value in obj.data.uv_layers.active.data],
        "objects": set(bpy.data.objects.keys()),
    }
    checks = 0

    def check(condition, reason):
        nonlocal checks
        if not condition:
            raise AssertionError(reason)
        checks += 1

    with tempfile.TemporaryDirectory(prefix="rou-lod1-") as directory:
        root = Path(directory)
        source = root / "source.blend"
        bpy.ops.file.pack_all()
        bpy.ops.wm.save_as_mainfile(filepath=str(source))
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        result = scene.export_model(
            {
                "id": "sphere",
                "entry": {"kind": "object", "name": obj.name},
                "asset_path": "runtime/sphere.glb",
                "lod1_path": "runtime/sphere.lod1.glb",
            },
            {"source_root": directory, "source": source.name, "output_dir": directory},
        )
        for tier in ["asset", "lod1"]:
            document = glb_json(root / result[tier]["path"])
            names = {material["name"] for material in document["materials"]}
            check(names == {"Override", "Second"}, f"Effective materials lost in {tier}: {names}")
            primitives = [p for mesh in document["meshes"] for p in mesh["primitives"]]
            used = {document["materials"][p["material"]]["name"] for p in primitives}
            check(used == names, f"Material assignments lost in {tier}")
            check(
                any("TEXCOORD_0" in p["attributes"] for p in primitives),
                f"Textured UV coordinates lost in {tier}",
            )
        check(
            result["metrics"]["lod1"]["indices"] < result["metrics"]["asset"]["indices"] * 0.4,
            "LOD did not reduce geometry",
        )
        check(
            external_images(root / result["asset"]["path"])
            == external_images(root / result["lod1"]["path"]),
            "Images within the size policy were re-encoded",
        )
        check(hashlib.sha256(source.read_bytes()).hexdigest() == digest, "Saved source changed")
    check(tuple(obj.location) == before["location"], "Root location changed")
    check([p.material_index for p in obj.data.polygons] == before["faces"], "Author faces changed")
    check(
        [tuple(value.uv) for value in obj.data.uv_layers.active.data] == before["uv"],
        "Author UVs changed",
    )
    check(not obj.modifiers and set(bpy.data.objects.keys()) == before["objects"], "Copies remain")
    check(obj.material_slots[0].link == "OBJECT", "Material slot link changed")
    check(obj.material_slots[0].material == materials[1], "Material override changed")
    check(obj.data.materials[0] == materials[0], "Underlying mesh material changed")
    verify_texture_lod(check)
    print("LOD1_EXPORT_PASS", checks)


if __name__ == "__main__":
    main()
