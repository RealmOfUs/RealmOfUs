"""Exercise Blender's real ZIP installation and restricted add-on lifecycle.

Run with Blender --background --factory-startup --python-exit-code 1. Set
BLENDER_USER_RESOURCES, BLENDER_USER_CONFIG and BLENDER_USER_SCRIPTS to directories
under --output before launching Blender; this test never saves user preferences.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tomllib
from contextlib import contextmanager
from pathlib import Path

import _bpy_restrict_state
import addon_utils
import bpy

MODULE = "bl_ext.realm_test.rou_blender"


class Receipt:
    def __init__(self, output: Path, archive: Path):
        self.output = output
        self.value = {
            "status": "running",
            "blender": bpy.app.version_string,
            "archive": str(archive),
            "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
            "checks": [],
            "restriction_entries": [],
        }

    def check(self, name: str, condition: bool, **evidence) -> None:
        self.value["checks"].append({"name": name, "passed": bool(condition), **evidence})
        if not condition:
            raise AssertionError(name)

    def rejected(self, name: str, operation) -> None:
        try:
            operation()
        except ValueError as error:
            self.check(name, True, error=str(error))
            return
        self.check(name, False)

    def write(self) -> None:
        self.output.mkdir(parents=True, exist_ok=True)
        (self.output / "installation-report.json").write_text(
            json.dumps(self.value, indent=2) + "\n"
        )


@contextmanager
def observe_restrictions(receipt: Receipt):
    """Record Blender's own restriction scope without replacing its behavior."""
    original = _bpy_restrict_state.RestrictBlend.__enter__

    def enter(scope):
        result = original(scope)
        receipt.value["restriction_entries"].append(
            {"data_type": type(bpy.data).__name__, "context_type": type(bpy.context).__name__}
        )
        return result

    _bpy_restrict_state.RestrictBlend.__enter__ = enter
    try:
        yield
    finally:
        _bpy_restrict_state.RestrictBlend.__enter__ = original


def check_isolation(receipt: Receipt) -> None:
    root = receipt.output.resolve()
    paths = {}
    for name in ("BLENDER_USER_RESOURCES", "BLENDER_USER_CONFIG", "BLENDER_USER_SCRIPTS"):
        value = os.environ.get(name)
        path = Path(value).resolve() if value else None
        receipt.check(name.lower() + "_isolated", path is not None and path.is_relative_to(root))
        paths[name] = str(path)
    for kind in ("CONFIG", "SCRIPTS"):
        path = Path(bpy.utils.user_resource(kind)).resolve()
        receipt.check("effective_" + kind.lower() + "_isolated", path.is_relative_to(root))
        paths[kind] = str(path)
    receipt.value["isolated_resources"] = paths


def registered(receipt: Receipt, addon, label: str) -> None:
    classes = [*addon.CLASSES, *addon.source_browser.CLASSES]
    receipt.check(label + "_classes", all(cls.is_registered for cls in classes), count=len(classes))
    panel = addon.ROU_PT_panel
    receipt.check(
        label + "_sidebar",
        (panel.bl_space_type, panel.bl_region_type, panel.bl_category)
        == ("VIEW_3D", "UI", "Realm"),
    )
    operators = [cls for cls in classes if issubclass(cls, bpy.types.Operator)]
    for operator in operators:
        namespace, name = operator.bl_idname.split(".")
        getattr(getattr(bpy.ops, namespace), name).get_rna_type()
    receipt.check(label + "_operators", True, count=len(operators))
    browser = addon.source_browser
    receipt.check(
        label + "_single_handlers",
        bpy.app.handlers.load_post.count(browser.source_loaded) == 1
        and bpy.app.handlers.save_post.count(browser.initial_source_saved) == 1,
    )
    receipt.check(label + "_properties", hasattr(bpy.types.WindowManager, "rou_source_items"))


def disabled(receipt: Receipt, addon, label: str) -> None:
    browser = addon.source_browser
    browser.ALLOW_INITIAL_SAVE = True
    receipt.check(
        label + "_operator", bpy.ops.preferences.addon_disable(module=MODULE) == {"FINISHED"}
    )
    receipt.check(label + "_state", not addon_utils.check(MODULE)[1])
    receipt.check(label + "_session", browser.SESSION is None and not browser.ALLOW_INITIAL_SAVE)
    receipt.check(
        label + "_handlers",
        browser.source_loaded not in bpy.app.handlers.load_post
        and browser.initial_source_saved not in bpy.app.handlers.save_post,
    )
    receipt.check(
        label + "_classes",
        all(not cls.is_registered for cls in [*addon.CLASSES, *browser.CLASSES]),
    )
    receipt.check(label + "_properties", not hasattr(bpy.types.WindowManager, "rou_source_items"))


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def create_fixture(receipt: Receipt, addon) -> tuple[Path, Path]:
    root = receipt.output / "realm"
    root.mkdir(exist_ok=False)
    subprocess.run(["git", "init", "-b", "main", str(root)], check=True, capture_output=True)
    profile = addon.core.CONFIG["profile"]
    write_json(
        root / "realm.json",
        {
            "format": "rou-realm/0.1",
            "id": "realm_install_regression",
            "profile": profile,
        },
    )
    source = root / "sources/houses/complete.blend"
    source.parent.mkdir(parents=True)
    write_json(
        root / "realm/models/house.json",
        {
            "format": "rou-model/0.1",
            "id": "house",
            "source": source.relative_to(root).as_posix(),
            "entry": {"kind": "object", "name": "House"},
            "dependencies": [],
            "settings": {},
        },
    )
    placement = root / "realm/entities/house.json"
    write_json(
        placement,
        {
            "format": "rou-entity/0.1",
            "id": "house",
            "model": "house",
            "label": "House",
            "position_mm": [12000, 2000, 8000],
        },
    )
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    entity = bpy.data.objects.new("House", None)
    entity["rou_entity_id"] = "house"
    entity.location = (1, -2, 3)
    bpy.context.collection.objects.link(entity)
    bpy.ops.mesh.primitive_cube_add(size=2)
    bpy.context.object.parent = entity
    bpy.context.object.location = (0, 0, 0)
    bpy.data.scenes.new("Author notes")
    bpy.ops.wm.save_as_mainfile(filepath=str(source))
    receipt.check("unguarded_save_does_not_start_session", addon.source_browser.SESSION is None)
    return source, placement


def exercise(receipt: Receipt, archive: Path) -> None:
    check_isolation(receipt)
    global MODULE
    bpy.context.preferences.view.show_splash = False
    bpy.ops.preferences.extension_repo_add(
        name="Realm Test",
        type="LOCAL",
        use_custom_directory=True,
        custom_directory=str(receipt.output / "extensions"),
    )
    repo = bpy.context.preferences.extensions.repos[-1]
    MODULE = f"bl_ext.{repo.module}.rou_blender"
    receipt.check(
        "zip_install_operator",
        bpy.ops.extensions.package_install_files(
            filepath=str(archive), repo=repo.module, enable_on_install=False
        )
        == {"FINISHED"},
    )
    receipt.check(
        "enable_operator", bpy.ops.preferences.addon_enable(module=MODULE) == {"FINISHED"}
    )
    addon = sys.modules[MODULE]
    browser = addon.source_browser
    receipt.value["addon_version"] = tomllib.loads(
        Path(addon.__file__).with_name("blender_manifest.toml").read_text()
    )["version"]
    receipt.check(
        "packaged_native_library",
        addon.core.library_path().is_relative_to(receipt.output)
        and addon.core.LIB.rou_abi_version() == 3,
    )
    receipt.value["installed_module"] = addon.__file__
    receipt.check(
        "installed_copy_used", Path(addon.__file__).resolve().is_relative_to(receipt.output)
    )
    receipt.check("initial_enabled_state", addon_utils.check(MODULE)[1])
    registered(receipt, addon, "initial_enable")
    receipt.check("blank_start_has_no_session", browser.SESSION is None)
    source, placement = create_fixture(receipt, addon)
    disabled(receipt, addon, "first_disable")

    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    # Background execution has no interactive undo baseline after loading.
    # Establish one so Blender marks the following operator edit as unsaved.
    bpy.ops.ed.undo_push(message="Before unsaved installation test edit")
    bpy.ops.mesh.primitive_cube_add(location=(40, 0, 0))
    bpy.context.object.name = "Unsaved change"
    receipt.check("dirty_fixture", bpy.data.is_dirty)
    source_before = source.read_bytes()
    placement_before = placement.read_bytes()
    bpy.ops.preferences.addon_enable(module=MODULE)
    registered(receipt, addon, "dirty_reenable")
    receipt.check(
        "dirty_scene_not_adopted_or_modified",
        browser.SESSION is None
        and "Unsaved change" in bpy.data.objects
        and tuple(bpy.data.objects["House"].location) == (1, -2, 3),
    )
    receipt.rejected("dirty_scene_write_refused", browser.write_placement)
    receipt.check(
        "refused_write_preserves_disk",
        source.read_bytes() == source_before and placement.read_bytes() == placement_before,
    )

    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    receipt.check("load_starts_session", browser.require_session().source == source)
    receipt.check("complete_source_loaded", "Author notes" in bpy.data.scenes)
    receipt.check(
        "placement_restored_on_load", tuple(bpy.data.objects["House"].location) == (12, -8, 2)
    )
    receipt.check(
        "catalog_operator",
        bpy.ops.rou.browse_sources(filepath=str(source.parents[2] / "realm.json")) == {"FINISHED"}
        and len(bpy.context.window_manager.rou_source_items) == 1,
    )
    bpy.data.objects["House"].location.x = 14
    receipt.check("placement_operator", bpy.ops.rou.write_placement() == {"FINISHED"})
    receipt.check(
        "placement_written", json.loads(placement.read_text())["position_mm"] == [14000, 2000, 8000]
    )
    receipt.check("save_operator", bpy.ops.rou.save_work() == {"FINISHED"})
    receipt.check("guarded_save_keeps_session", browser.require_session().source == source)

    value = json.loads(placement.read_text())
    value["position_mm"][0] = 15000
    write_json(placement, value)
    receipt.rejected("external_placement_stales_session", browser.write_placement)
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    receipt.check("external_placement_restored", bpy.data.objects["House"].location.x == 15)
    disabled(receipt, addon, "active_session_disable")
    bpy.ops.preferences.addon_enable(module=MODULE)
    registered(receipt, addon, "final_reenable")
    receipt.check("open_source_requires_reopen", browser.SESSION is None)
    bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
    receipt.check(
        "reenabled_load_restores_session_and_placement",
        browser.require_session().source == source and bpy.data.objects["House"].location.x == 15,
    )
    disabled(receipt, addon, "final_disable")
    entries = receipt.value["restriction_entries"]
    receipt.check(
        "real_blender_restrictions_observed",
        len(entries) >= 3 and all(entry["data_type"] == "_RestrictData" for entry in entries),
        count=len(entries),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--addon-zip", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    receipt = Receipt(args.output.resolve(), args.addon_zip.resolve())
    try:
        with observe_restrictions(receipt):
            exercise(receipt, args.addon_zip.resolve())
        receipt.value["status"] = "passed"
    except Exception as error:
        receipt.value["status"] = "failed"
        receipt.value["error"] = str(error)
        raise
    finally:
        receipt.write()
        print("ADDON_INSTALL_RESULT", json.dumps(receipt.value), flush=True)


if __name__ == "__main__":
    main()
