"""Blender scene traversal, static-data checks, bounds and GLB export."""

import json
from contextlib import contextmanager
from pathlib import Path

import bpy
from mathutils import Vector

MODEL_OWNERS_PROPERTY = "rou_model_owners_json"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def saved_model_owners():
    """Read source-local author IDs without requiring the native library in Blender exports."""
    owners = set()
    for scene in bpy.data.scenes:
        encoded = scene.get(MODEL_OWNERS_PROPERTY, "[]")
        require(isinstance(encoded, str), "Invalid saved model owners")
        values = json.loads(encoded)
        require(isinstance(values, list), "Invalid saved model owners")
        for value in values:
            require(isinstance(value, str), "Author entity ID must be a string")
            owners.add(value)
    return owners


def entity_objects():
    """Index current live objects by entity ID, including inactive Scenes and linked objects."""
    objects = {}
    for obj in bpy.data.objects:
        entity_id = obj.get("rou_entity_id")
        if not obj.users_scene or not entity_id:
            continue
        require(isinstance(entity_id, str), "Entity ID must be a string")
        require(entity_id not in objects, "Duplicate entity IDs; assign copied roots new IDs")
        objects[entity_id] = obj
    return objects


def require_entry_name(entry, owner):
    """Reject name drift before saving or resolving an Object entry for export."""
    require(
        entry["kind"] == "object" and entry["name"] == owner.name,
        "Source entry name changed; register marked entities, then save the source: " + owner.name,
    )


def require_model_owner(model):
    """Verify saved ownership against the entity IDs from the frozen Git snapshot."""
    if model["entry"]["kind"] != "object":
        return
    owners = saved_model_owners() & set(model["entity_ids"])
    require(len(owners) <= 1, "Multiple author entities own model: " + model["id"])
    if not owners:
        # Sources without registered ownership still use their declared entry.
        return
    entity_id = next(iter(owners))
    owner = entity_objects().get(entity_id)
    require(owner is not None, "Source model root missing; restore the entity object: " + entity_id)
    require_entry_name(model["entry"], owner)


def entry_objects(entry):
    if entry["kind"] == "object":
        root = bpy.data.objects.get(entry["name"])
        require(root is not None, f"Source object missing: {entry['name']}")
        require(root.parent is None, "Object entry must be a root")
        basis = root.matrix_basis.to_3x3()
        require(
            all(
                abs(basis[r][c] - (1.0 if r == c else 0.0)) < 1e-6
                for r in range(3)
                for c in range(3)
            ),
            "Apply root rotation and scale before exporting",
        )
        return [root, *root.children_recursive], [root]
    require(entry["kind"] == "collection", "Unsupported entry kind")
    collection = bpy.data.collections.get(entry["name"])
    require(collection is not None, f"Source collection missing: {entry['name']}")
    objects = list(collection.all_objects)
    require(objects, "Source collection is empty")
    selected = set(objects)
    require(
        all(obj.parent is None or obj.parent in selected for obj in objects),
        "Collection contains an object parented outside the entry",
    )
    return objects, []


def dependency_closure(roots):
    dependencies = {}
    for dependency, users in bpy.data.user_map().items():
        for user in users:
            dependencies.setdefault(user, set()).add(dependency)
    result = set(roots)
    pending = list(result)
    while pending:
        for dependency in dependencies.get(pending.pop(), ()):
            if dependency not in result:
                result.add(dependency)
                pending.append(dependency)
    return result


def check_static(objects):
    blocks = dependency_closure(objects)
    for block in blocks:
        require(
            not getattr(block, "animation_data", None),
            f"Animation or drivers are unsupported: {block.name}",
        )
        if isinstance(block, bpy.types.Object):
            require(not block.constraints, f"Constraints are unsupported: {block.name}")
            require(not block.particle_systems, f"Particles are unsupported: {block.name}")
            require(not block.rigid_body, f"Rigid body simulation is unsupported: {block.name}")
            require(
                block.type not in {"CURVE", "FONT", "SURFACE", "META", "VOLUME", "POINTCLOUD"},
                f"Convert {block.type} geometry to mesh before export: {block.name}",
            )
            for modifier in block.modifiers:
                require(
                    modifier.type
                    not in {
                        "ARMATURE",
                        "CLOTH",
                        "FLUID",
                        "SOFT_BODY",
                        "MESH_CACHE",
                        "MESH_SEQUENCE_CACHE",
                        "NODES",
                    },
                    f"Dynamic modifier is unsupported: {block.name}/{modifier.name}",
                )
        if isinstance(block, bpy.types.Image):
            require(
                block.source in {"FILE", "GENERATED"},
                f"Use a static image: {block.name}",
            )
            if block.source == "FILE" and not block.packed_file:
                target = Path(bpy.path.abspath(block.filepath, library=block.library))
                require(target.is_file(), f"Missing authoring file: {target}")
            require(min(block.size) > 0 and block.has_data, f"Image data missing: {block.name}")
    return blocks


def source_dependencies(objects, source_root, source):
    blocks = check_static(objects)
    result = set()
    paths = bpy.data.file_path_map(subset=list(blocks), include_libraries=True)
    for block, references in paths.items():
        if isinstance(block, bpy.types.Image) and block.packed_file:
            continue
        for reference in references:
            owner = block.library
            if isinstance(block, bpy.types.Library) and block.parent:
                owner = block.parent
            target = Path(bpy.path.abspath(reference, library=owner)).resolve()
            require(target.is_file(), f"Missing authoring file: {target}")
            require(target.is_relative_to(source_root), f"External dependency: {target}")
            relative = target.relative_to(source_root).as_posix()
            if relative != source:
                result.add(relative)
    for block in blocks:
        if isinstance(block, bpy.types.Image):
            require(min(block.size) > 0 and block.has_data, f"Image data missing: {block.name}")
    return sorted(result)


def select_scene(objects):
    if all(obj.name in bpy.context.scene.objects for obj in objects):
        return
    candidates = [
        scene for scene in bpy.data.scenes if all(obj.name in scene.objects for obj in objects)
    ]
    require(candidates, "Entry objects must belong to one saved Scene")
    bpy.context.window.scene = sorted(candidates, key=lambda scene: scene.name)[0]


def local_bounds(objects):
    points = []
    selected = set(objects)
    for instance in bpy.context.evaluated_depsgraph_get().object_instances:
        original = instance.object.original
        parent = instance.parent.original if instance.parent else None
        if original not in selected and (not instance.is_instance or parent not in selected):
            continue
        if instance.object.type != "MESH":
            continue
        for corner in instance.object.bound_box:
            point = instance.matrix_world @ Vector(corner)
            points.append(tuple(point))
    require(points, "Entry has no mesh geometry")
    return [
        [min(point[axis] for point in points) for axis in range(3)],
        [max(point[axis] for point in points) for axis in range(3)],
    ]


@contextmanager
def export_visibility(objects):
    """Include hidden static geometry and restore every changed scene setting."""
    selected = list(bpy.context.selected_objects)
    active = bpy.context.view_layer.objects.active
    states = [
        (obj, obj.hide_get(), obj.hide_select, obj.hide_viewport, obj.hide_render)
        for obj in objects
    ]
    try:
        bpy.ops.object.select_all(action="DESELECT")
        for obj, *_ in states:
            require(obj.name in bpy.context.view_layer.objects, "Entry is outside active Scene")
            obj.hide_set(False)
            obj.hide_select = False
            obj.hide_viewport = False
            obj.hide_render = False
            obj.select_set(True)
        bpy.context.view_layer.objects.active = objects[0]
        bpy.context.view_layer.update()
        yield
    finally:
        bpy.ops.object.select_all(action="DESELECT")
        for obj in selected:
            obj.select_set(True)
        for obj, hidden, locked, viewport, render in states:
            obj.hide_set(hidden)
            obj.hide_select = locked
            obj.hide_viewport = viewport
            obj.hide_render = render
        bpy.context.view_layer.objects.active = active
        bpy.context.view_layer.update()


def export_glb(path):
    """Export the selected static objects using the shared build/preview settings."""
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
        export_animations=False,
        export_extras=False,
        export_cameras=False,
        export_lights=False,
        export_materials="EXPORT",
    )


def export_model(model, request):
    # Direct preview callers have no Git entity mapping. The build helper must
    # supply it so a native Blender save cannot bypass the registration check.
    if "entity_ids" in model:
        require_model_owner(model)
    objects, roots = entry_objects(model["entry"])
    select_scene(objects)
    dependencies = source_dependencies(
        objects, Path(request["source_root"]).resolve(), request["source"]
    )
    locations = [(root, root.location.copy()) for root in roots]
    path = Path(request["output_dir"]) / model["asset_path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        for root, _ in locations:
            root.location = (0, 0, 0)
        bpy.context.view_layer.update()
        with export_visibility(objects):
            bounds = local_bounds(objects)
            export_glb(path)
    finally:
        for root, location in locations:
            root.location = location
        bpy.context.view_layer.update()
    return {
        "id": model["id"],
        "path": model["asset_path"],
        "bounds_blender_m": bounds,
        "dependencies": dependencies,
    }
