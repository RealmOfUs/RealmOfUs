"""Real Blender round trip through the packaged Rust ABI and local preview exporter."""

import argparse
import importlib
import json
import shutil
import sys
import zipfile
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))


def load_addon(archive, output):
    if archive is None:
        from toolchain.blender import rou_blender

        return rou_blender
    installation = output.resolve() / "installed-addon"
    module = installation / "rou_blender"
    with zipfile.ZipFile(archive) as package:
        package.extractall(module)
    sys.path.insert(0, str(installation))
    addon = importlib.import_module("rou_blender")
    assert addon.core.library_path().is_relative_to(installation)
    return addon


def validate_bundle(core, directory):
    manifest = core.read_bundle(directory / "realm.json")
    assert {path.name for path in directory.iterdir()} == {"realm.json", "objects", "chunks"}
    for entity in manifest["entities"]:
        asset = entity["asset"]
        raw = (directory / asset["path"]).read_bytes()
        assert core.call("file.descriptor", raw) == {
            "sha256": asset["sha256"],
            "bytes": asset["bytes"],
        }
        core.call("glb.precheck", raw)
    return manifest


def verify(addon, output):
    core = addon.core
    bpy.ops.wm.read_factory_settings(use_empty=True)
    profile = core.CONFIG["profile"]
    bpy.context.scene["rou_profile_json"] = json.dumps(profile)
    root = bpy.data.objects.new("Cross-chunk entity", None)
    root["rou_entity_id"] = "test_entity"
    root.location = (31, -1, 1)
    bpy.context.collection.objects.link(root)
    bpy.ops.mesh.primitive_cube_add(size=2)
    cube = bpy.context.object
    cube.parent = root
    cube.location = (0, 0, 0)
    cube.scale.x = 2
    bpy.context.view_layer.update()
    selected = list(bpy.context.selected_objects)
    active = bpy.context.view_layer.objects.active
    assert bpy.ops.rou.show_grid() == {"FINISHED"}
    assert bpy.ops.rou.show_grid() == {"FINISHED"}
    assert bpy.ops.rou.export_local_preview(directory=str(output / "before")) == {"FINISHED"}
    first = validate_bundle(core, output / "before")
    assert set(first["chunks"]) == {"0,0,0", "1,0,0"}
    assert "entries" not in first
    assert list(bpy.context.selected_objects) == selected
    assert bpy.context.view_layer.objects.active == active
    root.location.x += 64
    bpy.context.view_layer.update()
    addon.exporter.export_scene(output / "after")
    moved = validate_bundle(core, output / "after")
    assert set(moved["chunks"]) == {"2,0,0", "3,0,0"}
    assert first["entities"][0]["asset"] == moved["entities"][0]["asset"]
    for name in ("invalid", "empty", "missing"):
        (output / name / "objects").mkdir(parents=True)
    (output / "invalid/realm.json").write_text('{"format": "invalid"}')
    empty = core.call("preview.create", {"profile": profile, "entities": []})
    core.write_bundle(output / "empty", empty)
    shutil.copytree(output / "after/chunks", output / "missing/chunks")
    shutil.copyfile(output / "after/realm.json", output / "missing/realm.json")
    shutil.copytree(output / "before", output / "live")
    before = (output / "before/realm.json").read_bytes()
    # Root transforms, dynamic children and non-mesh geometry cannot silently export.
    cases = []
    root.scale.x = 2
    cases.append(("root_scale", lambda: setattr(root.scale, "x", 1)))
    for name, restore in cases:
        bpy.context.view_layer.update()
        try:
            addon.exporter.export_scene(output / "before")
        except ValueError:
            pass
        else:
            raise AssertionError(name)
        restore()
    bpy.context.view_layer.update()
    cube.keyframe_insert(data_path="location", frame=1)
    try:
        addon.exporter.export_scene(output / "before")
    except ValueError:
        pass
    else:
        raise AssertionError("dynamic child accepted")
    cube.animation_data_clear()
    assert (output / "before/realm.json").read_bytes() == before
    assert list(bpy.context.selected_objects) == selected
    assert bpy.context.view_layer.objects.active == active
    # Both exporters include hidden descendants and restore their visibility.
    cube.hide_set(True)
    cube.hide_select = True
    cube.hide_viewport = True
    cube.hide_render = True
    addon.exporter.export_scene(output / "hidden")
    hidden = validate_bundle(core, output / "hidden")["entities"][0]
    assert hidden["asset"] == moved["entities"][0]["asset"]
    assert hidden["bounds_mm"] == moved["entities"][0]["bounds_mm"]
    result = addon.scene.export_model(
        {
            "id": "hidden",
            "entry": {"kind": "object", "name": root.name},
            "asset_path": "runtime/hidden.glb",
            "lod1_path": "runtime/hidden.lod1.glb",
        },
        {
            "source_root": str(output),
            "source": "test.blend",
            "output_dir": str(output / "build-hidden"),
        },
    )
    assert (output / "build-hidden/runtime/hidden.glb").read_bytes() == (
        output / "hidden" / hidden["asset"]["path"]
    ).read_bytes()
    local = core.call("blender.bounds_native", result["bounds_blender_m"])
    for axis in range(3):
        for bound in ("min", "max"):
            assert (
                local[bound][axis] + hidden["position_mm"][axis] == hidden["bounds_mm"][bound][axis]
            )
    assert cube.hide_get() and cube.hide_select and cube.hide_viewport and cube.hide_render
    cube.hide_set(False)
    cube.hide_select = cube.hide_viewport = cube.hide_render = False

    # A flat plane expands one millimetre in the degenerate exported axis.
    cube.scale.z = 0
    bpy.context.view_layer.update()
    addon.exporter.export_scene(output / "flat")
    flat = validate_bundle(core, output / "flat")["entities"][0]["bounds_mm"]
    assert flat["max"][1] - flat["min"][1] == 1
    return {
        "status": "passed",
        "checks": [
            "packaged_abi",
            "cross_chunk",
            "no_entries_or_sources",
            "movement_reuses_glb",
            "grid",
            "invalid_root",
            "dynamic_child",
            "failure_preserves_preview_and_selection",
            "flat_axis_expands_one_mm",
            "hidden_geometry_matches_build_and_restores_visibility",
        ],
        "blender": bpy.app.version_string,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--addon-zip", type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    args.output.mkdir(parents=True, exist_ok=True)
    addon = load_addon(args.addon_zip, args.output)
    addon.register()
    try:
        report = verify(addon, args.output)
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    finally:
        addon.unregister()
    print("RUST_ADDON_PASS", json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
