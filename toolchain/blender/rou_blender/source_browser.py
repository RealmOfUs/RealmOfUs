"""Open complete Git-tracked authoring projects and explicitly exchange placement."""

from pathlib import Path

import bpy
from bpy.app.handlers import persistent
from bpy.props import CollectionProperty, IntProperty, StringProperty
from bpy_extras.io_utils import ImportHelper

from . import core, dependencies, exporter, git_session, scene, source_identity

SESSION = None
SESSION_ERROR = ""
SOURCE_ONLY = None
ALLOW_INITIAL_SAVE = False


def repository_work_directory(scene) -> Path | None:
    candidates = [Path(bpy.data.filepath)] if bpy.data.filepath else []
    if bpy.context.window_manager.rou_source_catalog:
        candidates.append(Path(bpy.context.window_manager.rou_source_catalog))
    for candidate in candidates:
        try:
            return git_session.repository(candidate)
        except (ValueError, OSError):
            continue
    return None


def author_session(source):
    """Capture Git inputs and the source objects that own model entries."""
    session = git_session.Session(source)
    models, entities = git_session.scoped_records(
        session.root,
        session.source.relative_to(session.root).as_posix(),
        session.cache,
        session.required,
    )
    session.model_roots = source_identity.capture(models, entities)
    return session


def begin_session() -> None:
    global SESSION, SESSION_ERROR, SOURCE_ONLY
    SESSION = None
    SOURCE_ONLY = None
    SESSION_ERROR = ""
    if not bpy.data.filepath:
        return
    try:
        SESSION = author_session(Path(bpy.data.filepath))
    except (ValueError, OSError) as error:
        SESSION_ERROR = str(error)


@persistent
def source_loaded(_unused):
    begin_session()
    if SESSION is not None:
        try:
            synchronize_placement(SESSION.root)
        except (ValueError, OSError) as error:
            global SESSION_ERROR
            SESSION_ERROR = str(error)


@persistent
def initial_source_saved(_unused):
    global ALLOW_INITIAL_SAVE
    if ALLOW_INITIAL_SAVE and SESSION is None:
        ALLOW_INITIAL_SAVE = False
        begin_session()


def require_session():
    if SESSION is None or SESSION.source != Path(bpy.data.filepath).resolve():
        raise ValueError("Reopen the saved source project to start an authoring session")
    SESSION.check()
    return SESSION


def require_entry_names(session):
    """Check current metadata before a save, allowing unrelated placement edits."""
    models, _ = git_session.scoped_records(
        session.root,
        session.source.relative_to(session.root).as_posix(),
        session.cache,
        session.required,
    )
    owners = source_identity.roots(
        {
            model_id: entity_id
            for model_id, entity_id in session.model_roots.items()
            if model_id in models
        }
    )
    for model_id, owner in owners.items():
        scene.require_entry_name(models[model_id]["entry"], owner)


def save_work():
    """Save the complete source only after its author entry names are synchronized."""
    session = require_session()
    require_entry_names(session)
    bpy.ops.wm.save_as_mainfile(filepath=str(session.source))
    session.accept_own_write()


def model_records(root):
    return git_session.records(root, "model")


def entities_for_source(root: Path) -> list:
    models = model_records(root)
    source = Path(bpy.data.filepath).resolve()
    result = []
    for entity in git_session.records(root, "entity").values():
        model = models.get(entity["model"])
        if model and git_session.relative_file(root, model["source"]).resolve() == source:
            result.append((root / core.PATHS["entities"] / (entity["id"] + ".json"), entity, model))
    return result


def synchronize_placement(root: Path) -> None:
    """JSON placement is authoritative when an Object-bound source is reopened."""
    realm = core.read_json(root / core.PATHS["realm"])
    encoded_profile = core.call("json.canonical", realm["profile"])
    if bpy.context.scene.get("rou_profile_json") != encoded_profile:
        bpy.context.scene["rou_profile_json"] = encoded_profile
    records = entities_for_source(root)
    for _, entity, model in records:
        if model["entry"]["kind"] != "object":
            continue
        obj = entity_object(entity)
        if obj is not None:
            obj.location = core.call(
                "blender.placement",
                {"direction": "to_blender", "position_mm": entity["position_mm"]},
            )
            obj["rou_entity_id"] = entity["id"]
    bpy.context.view_layer.update()


def entity_object(entity):
    matches = [obj for obj in bpy.context.scene.objects if obj.get("rou_entity_id") == entity["id"]]
    core.require(len(matches) <= 1, "Duplicate scene entity IDs")
    if matches:
        return matches[0]
    return None


def working_file_path(context) -> Path:
    if bpy.data.filepath:
        return Path(bpy.data.filepath)
    root = repository_work_directory(context.scene)
    return (root / "sources" if root else Path.home()) / "realm-work.blend"


def load_catalog(scene, filepath: str) -> dict:
    root = git_session.repository(Path(filepath))
    realm = core.read_json(root / core.PATHS["realm"])
    core.call("realm.validate", realm)
    manager = bpy.context.window_manager
    manager.rou_source_items.clear()
    for model in model_records(root).values():
        item = manager.rou_source_items.add()
        item.name = model["source"] + " \u00b7 " + model["entry"]["name"]
        item.model_id = model["id"]
    manager.rou_source_index = 0
    manager.rou_source_filter = ""
    manager.rou_source_catalog = str(root / core.PATHS["realm"])
    return realm


def atomic_json(path, value):
    core.write_json(path, value)


def register_entities() -> int:
    session = require_session()
    root = session.root
    source = session.source.relative_to(root).as_posix()
    roots = [obj for obj in bpy.context.scene.objects if obj.get("rou_entity_id")]
    ids = [obj["rou_entity_id"] for obj in roots]
    core.require(len(ids) == len(set(ids)), "Duplicate entity IDs; assign copied roots new IDs")
    owners = source_identity.roots(session.model_roots)
    for obj in roots:
        exporter.require_translation_root(obj)
    dependencies.validate_static_data([obj for root in roots for obj in exporter.descendants(root)])
    deps = [path for path in dependencies.project_files(root, make_relative=True) if path != source]
    updates = {}
    new_owners = {}
    profile = core.read_json(root / core.PATHS["realm"])["profile"]
    for obj in roots:
        entity_id = exporter.assign_id(obj)
        core.call("id.validate", entity_id)
        model_path = root / core.PATHS["models"] / f"{entity_id}.json"
        existing_entities = [
            path
            for path in (root / core.PATHS["entities"]).glob("**/*.json")
            if path.stem == entity_id
        ]
        core.require(len(existing_entities) <= 1, "Duplicate entity ID in metadata")
        entity_path = (
            existing_entities[0]
            if existing_entities
            else root / core.PATHS["entities"] / f"{entity_id}.json"
        )
        if entity_path.exists():
            entity = core.read_json(entity_path)
            matches = [
                path
                for path in (root / core.PATHS["models"]).glob("**/*.json")
                if path.stem == entity["model"]
            ]
            core.require(len(matches) == 1, "Existing entity has no unique authoring model")
            model_path = matches[0]
            existing = updates.get(model_path) or core.read_json(model_path)
            core.require(
                existing["source"] == source and existing["entry"]["kind"] == "object",
                "Entity ID already belongs to a different source or entry kind",
            )
            updated = {**existing, "dependencies": deps}
            owner = owners[existing["id"]]
            updated["entry"] = {"kind": "object", "name": owner.name}
            if updated != existing:
                updates[model_path] = updated
            continue
        core.require(not model_path.exists(), "Model ID is already registered")
        position = core.call(
            "blender.placement", {"direction": "to_realm", "position": list(obj.location)}
        )
        core.call("spatial.owner", {"profile": profile, "position_mm": position})
        updates[model_path] = {
            "format": core.FORMATS["model"],
            "id": entity_id,
            "source": source,
            "entry": {"kind": "object", "name": obj.name},
            "dependencies": deps,
            "settings": {},
        }
        updates[entity_path] = {
            "format": core.FORMATS["entity"],
            "id": entity_id,
            "model": entity_id,
            "label": obj.name,
            "position_mm": position,
        }
        new_owners[entity_id] = entity_id
    session.check()
    prospective = {
        "realm": core.read_json(root / core.PATHS["realm"]),
        "models": model_records(root),
        "entities": git_session.records(root, "entity"),
        "inputs": {},
    }
    for path, value in updates.items():
        prospective["models" if "source" in value else "entities"][value["id"]] = value
    core.call("world.metadata", prospective)
    for path, value in updates.items():
        atomic_json(path, value)
    session.model_roots.update(new_owners)
    source_identity.remember(session.model_roots.values())
    session.accept_own_write()
    return sum("source" in value for value in updates.values())


def write_placement() -> int:
    session = require_session()
    root = session.root
    profile = core.read_json(root / core.PATHS["realm"])["profile"]
    updates = []
    for path, entity, model in entities_for_source(root):
        if model["entry"]["kind"] != "object":
            continue
        obj = entity_object(entity)
        core.require(obj is not None, "Entity object missing: " + entity["id"])
        core.require(
            obj.get("rou_entity_id") == entity["id"],
            "Ambiguous placement; use an explicit entity root",
        )
        exporter.require_translation_root(obj)
        position = core.call(
            "blender.placement", {"direction": "to_realm", "position": list(obj.location)}
        )
        core.call("spatial.owner", {"profile": profile, "position_mm": position})
        if entity["position_mm"] != position:
            updates.append((path, {**entity, "position_mm": position}))
    session.check()
    for path, entity in updates:
        atomic_json(path, entity)
    session.accept_own_write()
    return len(updates)


def ready_file(root: Path, relative: str) -> Path:
    path = git_session.relative_file(root, relative)
    core.require(path.is_file(), "Authoring file is missing; fetch it with rou first: " + relative)
    with path.open("rb") as stream:
        core.require(
            core.call("lfs.pointer", stream.read(core.LIMITS["pointer_bytes"] + 1)) is None,
            "Authoring file is an LFS pointer; fetch it with rou first: " + relative,
        )
    return path


class ROU_SourceItem(bpy.types.PropertyGroup):
    model_id: StringProperty()


class ROU_UL_sources(bpy.types.UIList):
    def draw_item(self, context, layout, data, item, icon, active_data, active_propname, index):
        layout.label(text=item.name, icon="OUTLINER_COLLECTION")

    def filter_items(self, context, data, propname):
        query = context.window_manager.rou_source_filter.casefold().strip()
        flags = [
            self.bitflag_filter_item if query in item.name.casefold() else 0
            for item in getattr(data, propname)
        ]
        return flags, []


class ROU_OT_source_catalog(bpy.types.Operator, ImportHelper):
    bl_idname = "rou.browse_sources"
    bl_label = "Browse realm projects"
    filename_ext = ".json"
    filter_glob: StringProperty(default="*.json", options={"HIDDEN"})

    def execute(self, context):
        try:
            load_catalog(context.scene, self.filepath)
            return {"FINISHED"}
        except (ValueError, OSError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


class ROU_OT_edit_source(bpy.types.Operator):
    bl_idname = "rou.edit_source"
    bl_label = "Open complete source project"

    def execute(self, context):
        try:
            core.require(
                not bpy.data.is_dirty,
                "Save or discard current changes before opening another source",
            )
            root = git_session.repository(Path(context.window_manager.rou_source_catalog))
            item = context.window_manager.rou_source_items[context.window_manager.rou_source_index]
            model = model_records(root)[item.model_id]
            path = ready_file(root, model["source"])
            for dependency in model.get("dependencies", []):
                ready_file(root, dependency)
            bpy.ops.wm.open_mainfile(filepath=str(path), use_scripts=False)
            return {"FINISHED"}
        except (ValueError, OSError, KeyError, IndexError, RuntimeError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


class ROU_OT_save_work(bpy.types.Operator):
    bl_idname = "rou.save_work"
    bl_label = "Save complete source"

    def invoke(self, context, event):
        global ALLOW_INITIAL_SAVE
        if SESSION is not None:
            return self.execute(context)
        if bpy.data.filepath:
            self.report({"ERROR"}, "Reopen this source project before saving through the addon")
            return {"CANCELLED"}
        ALLOW_INITIAL_SAVE = True
        return bpy.ops.wm.save_as_mainfile(
            "INVOKE_DEFAULT", filepath=str(working_file_path(context))
        )

    def execute(self, context):
        try:
            save_work()
            self.report({"INFO"}, "Source saved; run rou build, review, then rou add")
            return {"FINISHED"}
        except (ValueError, OSError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


class ROU_OT_register_entities(bpy.types.Operator):
    bl_idname = "rou.register_entities"
    bl_label = "Register marked entities"

    def execute(self, context):
        try:
            count = register_entities()
            self.report(
                {"INFO"},
                f"Updated {count} models; save source, run rou build, then rou add/commit",
            )
            return {"FINISHED"}
        except (ValueError, OSError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


class ROU_OT_write_placement(bpy.types.Operator):
    bl_idname = "rou.write_placement"
    bl_label = "Write placement changes"

    def execute(self, context):
        try:
            count = write_placement()
            self.report(
                {"INFO"}, f"Updated {count} placement records; review with rou diff, then rou add"
            )
            return {"FINISHED"}
        except (ValueError, OSError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


def save_source_only():
    global SESSION, SESSION_ERROR, SOURCE_ONLY
    core.require(SESSION is not None, "Reopen the source before saving")
    SESSION.check_source()
    require_entry_names(SESSION)
    bpy.ops.wm.save_as_mainfile(filepath=str(SESSION.source))
    SOURCE_ONLY = git_session.Session(SESSION.source)
    SESSION = None
    SESSION_ERROR = (
        "Source saved only; reopen to restore JSON placement or reapply current placement"
    )


def reapply_placement():
    global SESSION, SESSION_ERROR, SOURCE_ONLY
    prior = SESSION or SOURCE_ONLY
    core.require(prior is not None, "Reopen the source before reapplying placement")
    prior.check_source()
    SESSION = author_session(Path(bpy.data.filepath))
    SOURCE_ONLY = None
    count = write_placement()
    SESSION_ERROR = ""
    return count


class ROU_OT_save_source_only(bpy.types.Operator):
    bl_idname = "rou.save_source_only"
    bl_label = "Save source only"
    bl_description = (
        "Save geometry without changing placement JSON when this source file is unchanged on disk"
    )

    def execute(self, context):
        try:
            save_source_only()
            return {"FINISHED"}
        except (ValueError, OSError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


class ROU_OT_reapply_placement(bpy.types.Operator):
    bl_idname = "rou.reapply_placement"
    bl_label = "Reapply placement to latest JSON"
    bl_description = "Explicitly write this scene's entity positions over current placement records"

    def execute(self, context):
        try:
            count = reapply_placement()
            self.report({"INFO"}, f"Reapplied {count} entity positions")
            return {"FINISHED"}
        except (ValueError, OSError) as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}


CLASSES = [
    ROU_SourceItem,
    ROU_UL_sources,
    ROU_OT_source_catalog,
    ROU_OT_edit_source,
    ROU_OT_save_work,
    ROU_OT_register_entities,
    ROU_OT_write_placement,
    ROU_OT_save_source_only,
    ROU_OT_reapply_placement,
]


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.WindowManager.rou_source_items = CollectionProperty(type=ROU_SourceItem)
    bpy.types.WindowManager.rou_source_index = IntProperty(default=0, min=0)
    bpy.types.WindowManager.rou_source_catalog = StringProperty(subtype="FILE_PATH")
    bpy.types.WindowManager.rou_source_filter = StringProperty(default="")
    bpy.app.handlers.load_post.append(source_loaded)
    bpy.app.handlers.save_post.append(initial_source_saved)
    # Blender restricts bpy.data and bpy.context while enabling add-ons.
    # Capture authoring state only after loading a source or its first save.
    # An already open project must be reopened, so an old Scene is never
    # accepted as the baseline for a newer Git worktree.


def unregister():
    global SESSION, SESSION_ERROR, SOURCE_ONLY, ALLOW_INITIAL_SAVE
    SESSION = None
    SESSION_ERROR = ""
    SOURCE_ONLY = None
    ALLOW_INITIAL_SAVE = False
    if initial_source_saved in bpy.app.handlers.save_post:
        bpy.app.handlers.save_post.remove(initial_source_saved)
    if source_loaded in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.remove(source_loaded)
    for name in ["rou_source_catalog", "rou_source_index", "rou_source_items", "rou_source_filter"]:
        delattr(bpy.types.WindowManager, name)
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
