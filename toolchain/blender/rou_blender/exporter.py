"""Export marked roots to an entry-free local preview; preserve the author scene."""

import json
import shutil
import uuid
from pathlib import Path
from tempfile import TemporaryDirectory

import bpy

from . import core, scene


def assign_id(obj):
    if not obj.get("rou_entity_id"):
        obj["rou_entity_id"] = "entity_" + uuid.uuid4().hex
    return core.call("id.validate", str(obj["rou_entity_id"]))


def require_translation_root(root):
    basis = root.matrix_basis.to_3x3()
    identity = all(
        abs(basis[row][column] - (1.0 if row == column else 0.0)) < 1e-6
        for row in range(3)
        for column in range(3)
    )
    core.require(
        root.parent is None and identity and not root.constraints and root.animation_data is None,
        "Entity roots support translation only; apply rotation and scale",
    )


def descendants(root):
    return [root, *root.children_recursive]


def export_entity(root, output):
    require_translation_root(root)
    identity = assign_id(root)
    # Exported meshes are local to the root, while catalog bounds are world bounds.
    position = core.call(
        "blender.placement", {"direction": "to_realm", "position": list(root.location)}
    )
    objects = descendants(root)
    scene.check_static(objects)
    original = root.location.copy()
    temporary = output / (identity + ".glb")
    simplified = output / (identity + ".lod1.glb")
    try:
        with scene.export_visibility(objects):
            world_bounds = core.call("blender.bounds_native", scene.local_bounds(objects))
            root.location = (0, 0, 0)
            bpy.context.view_layer.update()
            scene.export_glb(temporary)
            scene.export_lod1(objects, temporary, simplified, core.CONFIG["lod1"])
    finally:
        root.location = original
        bpy.context.view_layer.update()
    assets = {}
    for key, path in [("asset", temporary), ("lod1", simplified)]:
        result = core.call("glb.inspect", core.read_bytes(path, core.LIMITS["glb_bytes"]))
        asset = result["descriptor"]
        asset["path"] = core.call("asset.path", asset["sha256"])
        path.replace(output / asset["path"])
        assets[key] = asset
    return {
        "id": identity,
        "label": root.name,
        "position_mm": position,
        "bounds_mm": world_bounds,
        **assets,
    }


def export_scene(output, profile=None):
    output = Path(output).absolute()
    core.require(not output.is_symlink(), "Preview directory must not be a symlink")
    output.parent.mkdir(parents=True, exist_ok=True)
    profile = core.call(
        "profile.validate",
        profile or core.call("json.strict", bpy.context.scene["rou_profile_json"].encode()),
    )
    roots = sorted(
        (obj for obj in bpy.context.scene.objects if obj.get("rou_entity_id")),
        key=lambda obj: obj["rou_entity_id"],
    )
    selected = list(bpy.context.selected_objects)
    active = bpy.context.view_layer.objects.active
    try:
        with TemporaryDirectory(prefix=".rou-preview-", dir=output.parent) as temporary:
            stage = Path(temporary)
            (stage / core.PATHS["objects"]).mkdir()
            entities = [export_entity(root, stage) for root in roots]
            manifest = core.call("preview.create", {"profile": profile, "entities": entities})
            core.write_json(stage / core.PATHS["realm"], manifest)
            if output.exists():
                core.require(
                    all(
                        p.name in {"realm.json", "objects"} and not p.is_symlink()
                        for p in output.iterdir()
                    ),
                    "Choose an empty directory or an existing local preview",
                )
                if (output / core.PATHS["realm"]).exists():
                    core.read_json(output / core.PATHS["realm"], "manifest.validate")
            output.mkdir(exist_ok=True)
            destination = output / core.PATHS["objects"]
            destination.mkdir(exist_ok=True)
            for path in (stage / core.PATHS["objects"]).iterdir():
                core.require(not (destination / path.name).is_symlink(), "Object symlink forbidden")
                shutil.copyfile(path, destination / path.name)
            (stage / core.PATHS["realm"]).replace(output / core.PATHS["realm"])
    finally:
        bpy.ops.object.select_all(action="DESELECT")
        for obj in selected:
            obj.select_set(True)
        bpy.context.view_layer.objects.active = active
    print(
        "ROU_PREVIEW_EXPORTED",
        json.dumps({"entities": len(entities), "output": str(output)}),
        flush=True,
    )
    return manifest
