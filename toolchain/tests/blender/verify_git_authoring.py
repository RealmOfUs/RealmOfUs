"""Verify complete-source authoring and stale-editor protection in real Blender."""

import argparse
import json
import subprocess
import sys
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from toolchain.tests.blender.verify_addon import load_addon  # noqa: E402


def run(root, *args):
    subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)


def rejected(operation, reason):
    try:
        operation()
    except (ValueError, OSError):
        return
    raise AssertionError(reason)


def verify_persistent_identity(addon, root, source, model_path, placement):
    """Exercise identity changes through real Blender saves, undo and duplication."""
    browser = addon.source_browser

    def owner():
        matches = [
            obj for obj in bpy.context.scene.objects if obj.get("rou_entity_id") == "model_0"
        ]
        assert len(matches) == 1
        return matches[0]

    def check_entry(changed):
        before = placement.read_bytes()
        assert browser.register_entities() == changed
        model = addon.core.read_json(model_path)
        assert model["entry"]["name"] == owner().name
        objects, roots = addon.scene.entry_objects(model["entry"])
        assert roots == [owner()]
        assert any(obj.type == "MESH" for obj in objects)
        assert all(obj.get("rou_entity_id") != "instance_two" for obj in objects)
        assert placement.read_bytes() == before
        return model

    owner().name = "Rename before reapply"
    browser.reapply_placement()
    check_entry(1)
    owner().name = "Rename before source-only save"
    source_before = source.read_bytes()
    metadata_before = (model_path.read_bytes(), placement.read_bytes())
    rejected(browser.save_work, "Complete source save must require synchronized entry names")
    rejected(browser.save_source_only, "Source-only save must require synchronized entry names")
    assert source.read_bytes() == source_before
    assert (model_path.read_bytes(), placement.read_bytes()) == metadata_before
    browser.reapply_placement()
    check_entry(1)
    browser.save_source_only()
    browser.reapply_placement()
    check_entry(0)

    metadata_before = (model_path.read_bytes(), placement.read_bytes())
    old_name = owner().name
    owner().name = "Renamed across file reload"
    bpy.ops.mesh.primitive_cube_add(size=50)
    bpy.context.object.name = old_name
    rejected(browser.save_work, "Same-name impostor must not bypass the save check")
    # Native Blender saves bypass add-on operators. The actual build helper
    # must reject the saved name drift before exporting the impostor's mesh.
    bpy.ops.wm.save_as_mainfile(filepath=str(source))
    stale_model = addon.core.read_json(model_path)
    export_root = root / "identity-export"
    export_root.mkdir()
    request = export_root / "stale-request.json"
    request.write_text(
        json.dumps(
            {
                "response_format": addon.core.FORMATS["blender_export"],
                "source_root": str(root),
                "source": source.relative_to(root).as_posix(),
                "output_dir": str(export_root / "rejected"),
                "models": [
                    {
                        **stale_model,
                        "asset_path": "impostor.glb",
                        "lod1_path": "impostor.lod1.glb",
                        "entity_ids": ["model_0", "instance_two"],
                    }
                ],
            }
        )
    )
    rejected_build = subprocess.run(
        [
            bpy.app.binary_path,
            "--background",
            "--factory-startup",
            "--disable-autoexec",
            str(source),
            "--python-exit-code",
            "1",
            "--python",
            str(ROOT / "toolchain/blender/export_git_realm.py"),
            "--",
            "--request",
            str(request),
            "--output",
            str(export_root / "rejected-response.json"),
        ],
        capture_output=True,
        text=True,
        timeout=60,
    )
    log = rejected_build.stdout + rejected_build.stderr
    (export_root / "rejected-build.log").write_text(log)
    assert rejected_build.returncode != 0 and "Source entry name changed" in log, log
    assert not (export_root / "rejected/impostor.glb").exists()
    assert model_path.read_bytes() == metadata_before[0]
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    model = check_entry(1)
    assert browser.SESSION.model_roots["shared_definition"] == "model_0"
    exported = addon.scene.export_model(
        {
            **model,
            "asset_path": "identity.glb",
            "lod1_path": "identity.lod1.glb",
            "entity_ids": ["model_0", "instance_two"],
        },
        {
            "source_root": str(root),
            "source": source.relative_to(root).as_posix(),
            "output_dir": str(root / "identity-export"),
        },
    )
    assert exported["id"] == "shared_definition"
    addon.core.call("glb.precheck", (root / "identity-export/identity.glb").read_bytes())
    assert bpy.ops.rou.save_work() == {"FINISHED"}

    # Initialize the background undo stack explicitly, delete the RNA owner,
    # and restore it through the same undo operator used by the interactive UI.
    bpy.context.preferences.edit.use_global_undo = True
    bpy.ops.ed.undo_push(message="Before deleting the author root")
    removed = owner()
    bpy.data.objects.remove(removed, do_unlink=True)
    bpy.ops.ed.undo_push(message="Deleted the author root")
    assert bpy.ops.ed.undo() == {"FINISHED"}
    try:
        removed.name
    except ReferenceError:
        pass
    else:
        raise AssertionError("Undo test did not invalidate the old RNA reference")
    check_entry(0)
    owner().name = "Author restored by undo"
    check_entry(1)

    # Shift+D copies the root ID. Reject two live copies, then accept the
    # surviving copy after removing the original hierarchy, without a reload.
    original = owner()
    hierarchy = [original, *original.children_recursive]
    bpy.ops.object.select_all(action="DESELECT")
    for obj in hierarchy:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = original
    assert bpy.ops.object.duplicate() == {"FINISHED"}
    before = model_path.read_bytes()
    rejected(browser.register_entities, "Two live roots with one ID must be rejected")
    assert model_path.read_bytes() == before
    for obj in reversed(hierarchy):
        bpy.data.objects.remove(obj, do_unlink=True)
    owner().name = "Surviving duplicated author"
    check_entry(1)
    assert bpy.ops.rou.save_work() == {"FINISHED"}
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    check_entry(0)

    # Losing the actual owner must not let an instance with its name take over,
    # including after a source-only save and reload. Restore the test source.
    saved_source = source.read_bytes()
    name = owner().name
    bpy.data.objects.remove(owner(), do_unlink=True)
    instance = next(
        obj for obj in bpy.context.scene.objects if obj.get("rou_entity_id") == "instance_two"
    )
    instance.name = name
    rejected(browser.register_entities, "Missing owner was replaced by a shared instance")
    rejected(browser.save_source_only, "Missing owner must block source-only save")
    # Keep the cross-session corruption fixture by explicitly bypassing the
    # newly guarded save operator, as an external Blender editor can do.
    bpy.ops.wm.save_as_mainfile(filepath=str(source))
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    rejected(browser.register_entities, "Reload rebound a missing owner by entry name")
    source.write_bytes(saved_source)
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    check_entry(0)
    return owner()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--addon-zip", type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    addon = load_addon(args.addon_zip, root)
    run(root, "init", "-b", "main")
    run(root, "config", "user.name", "Authoring Test")
    run(root, "config", "user.email", "authoring@example.invalid")
    (root / ".gitignore").write_text("*.blend1\n.rou/\ninstalled-addon/\n")
    profile = addon.core.CONFIG["profile"]
    (root / "realm.json").write_text(
        json.dumps(
            {
                "format": "rou-realm/0.1",
                "id": "realm_authoring_test",
                "profile": profile,
            }
        )
    )
    source = root / "sources" / "nested" / "complete.blend"
    source.parent.mkdir(parents=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon.register()
    bpy.context.scene["rou_profile_json"] = json.dumps(profile)
    for index in range(2):
        entity = bpy.data.objects.new(f"Model {index}", None)
        entity["rou_entity_id"] = f"model_{index}"
        entity.location = (5 + index * 10, -5, 2)
        bpy.context.collection.objects.link(entity)
        bpy.ops.mesh.primitive_cube_add(size=2)
        bpy.context.object.parent = entity
        bpy.context.object.location = (0, 0, 0)
    bpy.data.scenes.new("Author's extra Scene")
    bpy.ops.wm.save_as_mainfile(filepath=str(source))
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    assert addon.source_browser.register_entities() == 2
    run(root, "add", ".")
    run(root, "commit", "-m", "Complete original project")
    rejected(addon.source_browser.write_placement, "External commit must stale the open Scene")
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    assert "Author's extra Scene" in bpy.data.scenes
    assert len([o for o in bpy.context.scene.objects if o.get("rou_entity_id")]) == 2
    placement_before = (root / "realm/entities/model_0.json").read_bytes()
    texture = root / "sources/shared/new-texture.png"
    texture.parent.mkdir(parents=True)
    image = bpy.data.images.new("New shared texture", width=2, height=2)
    image.filepath_raw = str(texture)
    image.file_format = "PNG"
    image.save()
    bpy.data.images.remove(image)
    image = bpy.data.images.load(str(texture))
    material = bpy.data.materials.new("Shared textured material")
    material.use_nodes = True
    node = material.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = image
    material.node_tree.links.new(
        node.outputs["Color"], material.node_tree.nodes.get("Principled BSDF").inputs["Base Color"]
    )
    bpy.data.objects["Model 0"].children[0].data.materials.append(material)
    assert addon.source_browser.register_entities() == 2
    model = json.loads((root / "realm/models/model_0.json").read_text())
    assert "sources/shared/new-texture.png" in model["dependencies"]
    assert image.filepath.startswith("//")
    assert (root / "realm/entities/model_0.json").read_bytes() == placement_before
    assert bpy.ops.rou.save_work() == {"FINISHED"}
    image.source = "SEQUENCE"
    rejected(
        lambda: addon.dependencies.validate_static_data(
            [bpy.data.objects["Model 0"], *bpy.data.objects["Model 0"].children_recursive],
        ),
        "Image sequence must not silently become a still texture",
    )
    image.source = "FILE"
    rotation_root = bpy.data.objects["Model 0"]
    rotation_root.rotation_mode = "QUATERNION"
    rotation_root.rotation_quaternion = (0.7071068, 0.7071068, 0, 0)
    bpy.context.view_layer.update()
    rejected(
        addon.source_browser.register_entities,
        "Quaternion rotation is unsupported at an entity root",
    )
    rotation_root.rotation_quaternion = (1, 0, 0, 0)
    bpy.context.view_layer.update()
    missing = "sources/not-downloaded.png"
    rejected(
        lambda: addon.source_browser.ready_file(root, missing),
        "Missing dependency must prevent opening",
    )
    pointer = root / "sources/pointer.png"
    pointer.write_text(
        "version https://git-lfs.github.com/spec/v1\noid sha256:" + "0" * 64 + "\nsize 100\n"
    )
    rejected(
        lambda: addon.source_browser.ready_file(root, "sources/pointer.png"),
        "Pointer dependency must prevent opening",
    )
    entity = bpy.data.objects["Model 0"]
    entity.location.x += 4
    assert addon.source_browser.write_placement() == 1
    placement = root / "realm/entities/model_0.json"
    value = json.loads(placement.read_text())
    assert value["position_mm"][0] == 9000
    source_before = source.read_bytes()
    assert addon.source_browser.require_session()
    assert bpy.ops.rou.save_work() == {"FINISHED"}
    assert source.read_bytes() != source_before
    value["position_mm"][0] = 12000
    placement.write_text(json.dumps(value))
    expected = placement.read_bytes()
    rejected(addon.source_browser.write_placement, "Old Scene must not overwrite new placement")
    assert placement.read_bytes() == expected
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    assert bpy.data.objects["Model 0"].location.x == 12
    run(root, "switch", "-c", "feature/another-author")
    rejected(addon.source_browser.write_placement, "Branch switch must stale the Scene")
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    original = source.read_bytes()
    source.write_bytes(original + b"external-change")
    rejected(
        addon.source_browser.write_placement, "External source replacement must stale the Scene"
    )
    source.write_bytes(original)
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    dependency = root / "sources/shared.txt"
    dependency.write_text("shared v1\n")
    model_path = root / "realm/models/model_0.json"
    model = json.loads(model_path.read_text())
    model["dependencies"] = ["sources/shared.txt"]
    model_path.write_text(json.dumps(model))
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    dependency.write_text("shared v2\n")
    rejected(addon.source_browser.write_placement, "External dependency must stale the Scene")
    # A real linked .blend carrying an external image uses // paths relative
    # to its own library, not to the current source project.
    from examples.calibration.create_git_scene import shared_files

    shared_files(root)
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    library_path = root / "sources/shared/ornament.blend"
    with bpy.data.libraries.load(str(library_path), link=True, relative=True) as (_, requested):
        requested.meshes = ["SharedOrnamentMesh"]
    ornament = bpy.data.objects.new("Linked regression", requested.meshes[0])
    bpy.context.collection.objects.link(ornament)
    ornament.parent = bpy.data.objects["Model 0"]
    assert addon.source_browser.register_entities() == 2
    assert bpy.ops.rou.save_work() == {"FINISHED"}
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    model = addon.core.read_json(root / "realm/models/model_0.json", "model.validate")
    assert {"sources/shared/ornament.blend", "sources/shared/checker.png"} <= set(
        model["dependencies"]
    )
    assert all(library.filepath.startswith("//") for library in bpy.data.libraries)
    assert addon.source_browser.register_entities() == 0
    texture = root / "sources/shared/checker.png"
    texture.rename(texture.with_suffix(".missing"))
    try:
        addon.dependencies.project_files(root)
    except ValueError as error:
        assert "Missing" in str(error)
    else:
        raise AssertionError("missing linked texture accepted")
    finally:
        texture.with_suffix(".missing").rename(texture)

    # Unrelated records and cold sources do not stale this source's baseline.
    other = {
        **model,
        "id": "unrelated",
        "source": "sources/cold/not-loaded.blend",
        "dependencies": [],
    }
    addon.core.write_json(root / "realm/models/unrelated.json", other)
    unrelated = {
        "format": addon.core.FORMATS["entity"],
        "id": "unrelated",
        "model": "unrelated",
        "position_mm": [0, 0, 0],
    }
    addon.core.write_json(root / "realm/entities/unrelated.json", unrelated)
    addon.source_browser.require_session()
    unrelated["position_mm"][0] = 2000
    addon.core.write_json(root / "realm/entities/unrelated.json", unrelated)
    addon.source_browser.require_session()

    # Unrelated malformed records do not invalidate an existing source baseline.
    unrelated_path = root / "realm/entities/unrelated.json"
    saved_unrelated = unrelated_path.read_bytes()
    unrelated_path.write_bytes(b"{invalid json")
    addon.source_browser.require_session()
    unrelated_path.write_bytes(saved_unrelated)
    # Repeated checks reuse the source descriptor; a real source edit still stales it.
    session = addon.source_browser.require_session()
    cached_values = dict(session.cache.values)
    addon.source_browser.require_session()
    assert session.cache.values == cached_values

    # Two entities sharing one model map to different tagged objects.
    original = bpy.data.objects["Model 0"]
    duplicate = original.copy()
    duplicate.name = "Second model instance"
    duplicate["rou_entity_id"] = "instance_two"
    duplicate.location.x = 20
    bpy.context.collection.objects.link(duplicate)
    instance = {
        "format": addon.core.FORMATS["entity"],
        "id": "instance_two",
        "model": "model_0",
        "position_mm": [1000, 0, 1000],
    }
    addon.core.write_json(root / "realm/entities/instance_two.json", instance)
    addon.source_browser.reapply_placement()
    duplicate.location.x = 25
    assert addon.source_browser.write_placement() == 1
    assert (
        addon.core.read_json(root / "realm/entities/instance_two.json")["position_mm"][0] == 25000
    )
    assert addon.core.read_json(placement)["position_mm"][0] == 12000

    # Registering again must accept the instance by entity ID, despite its name.
    assert addon.source_browser.register_entities() == 0
    new_root = bpy.data.objects.new("New root after instance", None)
    bpy.context.collection.objects.link(new_root)
    new_root["rou_entity_id"] = "after_instance"
    assert addon.source_browser.register_entities() == 1
    assert (
        addon.core.read_json(root / "realm/entities/after_instance.json")["model"]
        == "after_instance"
    )

    # Renaming the author root must update the model even if another object
    # takes its old name. Renaming a shared instance must never redirect it.
    # Model IDs need not match either entity ID.
    old_model_path = root / "realm/models/model_0.json"
    shared_model = addon.core.read_json(old_model_path)
    shared_model["id"] = "shared_definition"
    model_path = root / "realm/models/shared_definition.json"
    addon.core.write_json(model_path, shared_model)
    old_model_path.unlink()
    for entity_id in ("model_0", "instance_two"):
        entity_path = root / "realm/entities" / (entity_id + ".json")
        entity = addon.core.read_json(entity_path)
        entity["model"] = shared_model["id"]
        addon.core.write_json(entity_path, entity)
    addon.source_browser.reapply_placement()
    original.name = "Renamed author root"
    impostor = bpy.data.objects.new("Model 0", None)
    bpy.context.collection.objects.link(impostor)
    duplicate.name = "Renamed shared instance"
    placement_bytes = placement.read_bytes()
    assert addon.source_browser.register_entities() == 1
    updated_model = addon.core.read_json(model_path)
    assert updated_model["entry"]["name"] == original.name
    objects, roots = addon.scene.entry_objects(updated_model["entry"])
    assert roots == [original] and impostor not in objects and duplicate not in objects
    assert placement.read_bytes() == placement_bytes
    assert addon.source_browser.register_entities() == 0

    # Each changed model is written once, regardless of its instance count.
    browser = addon.source_browser
    real_dependencies = addon.dependencies.project_files
    real_write = browser.atomic_json
    dependency = root / "sources/shared/registration-extra.png"
    dependency.write_bytes(texture.read_bytes())
    writes = []

    def changed_dependencies(*args, **kwargs):
        return sorted(
            [*real_dependencies(*args, **kwargs), "sources/shared/registration-extra.png"]
        )

    def record_write(path, value):
        writes.append(path)
        real_write(path, value)

    addon.dependencies.project_files = changed_dependencies
    browser.atomic_json = record_write
    try:
        changed = browser.register_entities()
    finally:
        addon.dependencies.project_files = real_dependencies
        browser.atomic_json = real_write
    assert writes.count(model_path) == 1
    assert len(writes) == len(set(writes)) == changed
    browser.register_entities()
    original = verify_persistent_identity(addon, root, source, model_path, placement)

    # Explicit source-only saving leaves the newer JSON intact, then explicit
    # reapplication can transfer scene positions into the latest records.
    value = addon.core.read_json(placement)
    value["position_mm"][0] = 18000
    addon.core.write_json(placement, value)
    expected = placement.read_bytes()
    original.location.x = 19
    addon.source_browser.save_source_only()
    assert placement.read_bytes() == expected
    rejected(
        addon.source_browser.write_placement, "Source-only save must not silently adopt placement"
    )
    addon.source_browser.reapply_placement()
    assert addon.core.read_json(placement)["position_mm"][0] == 19000

    realm_path = root / "realm.json"
    saved_realm = realm_path.read_bytes()
    realm_path.write_bytes(b"{}")
    addon.source_browser.begin_session()
    assert addon.source_browser.SESSION is None
    assert addon.source_browser.SESSION_ERROR
    realm_path.write_bytes(saved_realm)
    report = {
        "status": "passed",
        "blender": bpy.app.version_string,
        "checks": [
            "complete_source_multi_scene",
            "multiple_entity_registration",
            "explicit_placement",
            "guarded_complete_source_save",
            "placement_authority_on_reopen",
            "commit_stales_session",
            "branch_stales_session",
            "placement_stales_session",
            "source_stales_session",
            "dependency_stales_session",
            "existing_model_dependencies_refresh",
            "relative_dependency_paths",
            "registration_preserves_placement",
            "static_images_only",
            "quaternion_root_rejected",
            "missing_dependency_rejected",
            "pointer_dependency_rejected",
            "linked_library_external_texture_relative_paths",
            "missing_linked_texture_reports_missing",
            "unrelated_cold_source_does_not_stale_session",
            "unrelated_malformed_record_does_not_stale_session",
            "unchanged_source_reuses_descriptor",
            "shared_model_placement_uses_entity_identity",
            "registration_after_shared_model_instance",
            "renamed_author_entry_rejects_impostor_geometry",
            "independent_model_and_entity_ids_preserve_entry_owner",
            "shared_model_dependency_update_written_once",
            "renamed_root_survives_reapply",
            "renamed_root_survives_source_only_save",
            "renamed_root_survives_save_reload_with_impostor",
            "reloaded_owner_exports_real_geometry",
            "undo_restores_owner_after_rna_reference_invalidation",
            "duplicate_entity_ids_rejected_before_metadata_write",
            "surviving_duplicate_registers_before_and_after_reload",
            "missing_owner_never_rebinds_shared_instance_after_reload",
            "entry_name_drift_blocks_complete_source_save",
            "entry_name_drift_blocks_source_only_save_without_writes",
            "native_save_cannot_export_impostor_geometry",
            "registered_rename_saves_and_exports",
            "source_only_save_preserves_latest_placement",
            "explicit_reapply_uses_latest_records",
            "begin_session_failure_is_visible",
        ],
    }
    (root / "authoring-report.json").write_text(json.dumps(report, indent=2) + "\n")
    addon.unregister()
    print("GIT_AUTHORING_PASS", json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
