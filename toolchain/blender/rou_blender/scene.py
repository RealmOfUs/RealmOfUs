"""Blender traversal, static-data checks, bounds and geometry/texture LOD export."""

import json
import shutil
import struct
import uuid
from array import array
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


@contextmanager
def lod1_materials(materials, image_dimension):
    """Resize private image copies and remap only affected materials and node groups."""
    images, copies, groups, affected = {}, {}, {}, {}
    author_names = {material.name for material in bpy.data.materials}

    def needs_copy(tree):
        if tree not in affected:
            affected[tree] = False
            affected[tree] = any(
                (node.type == "TEX_IMAGE" and node.image in images)
                or (node.type == "GROUP" and node.node_tree and needs_copy(node.node_tree))
                for node in tree.nodes
            )
        return affected[tree]

    def remap(tree):
        for node in tree.nodes:
            if node.type == "TEX_IMAGE" and node.image in images:
                node.image = images[node.image]
            elif node.type == "GROUP" and node.node_tree and needs_copy(node.node_tree):
                original = node.node_tree
                if original not in groups:
                    groups[original] = original.copy()
                    remap(groups[original])
                node.node_tree = groups[original]

    try:
        for original in dependency_closure(materials):
            if not isinstance(original, bpy.types.Image) or max(original.size) <= image_dimension:
                continue
            image = original.copy()
            images[original] = image
            if original.source == "FILE":
                image.filepath = bpy.path.abspath(original.filepath, library=original.library)
            # Image.copy can reload disk/packed pixels instead of copying live paint edits.
            if original.is_dirty or original.source == "GENERATED":
                pixels = array("f", [0.0]) * len(original.pixels)
                original.pixels.foreach_get(pixels)
                image.pixels.foreach_set(pixels)
                del pixels
            require(min(image.size) > 0 and image.has_data, f"Image copy missing: {original.name}")
            longest = max(original.size)
            size = [max(1, round(edge * image_dimension / longest)) for edge in original.size]
            image.scale(*size)
            image.pack()
        for original in materials:
            if original.node_tree and needs_copy(original.node_tree):
                copies[original] = original.copy()
                # Linked libraries have separate name spaces; avoid ambiguous exported names.
                while True:
                    name = "rou_lod1_" + uuid.uuid4().hex
                    if name not in author_names:
                        break
                copies[original].name = name
                remap(copies[original].node_tree)
        yield copies
    finally:
        for material in copies.values():
            bpy.data.materials.remove(material, do_unlink=True)
        for group in groups.values():
            bpy.data.node_groups.remove(group, do_unlink=True)
        for image in images.values():
            bpy.data.images.remove(image, do_unlink=True)


def restore_material_names(path, materials):
    """Restore exported names without renaming author IDs, including linked materials.

    Only the generated JSON chunk changes; stream the BIN chunk without re-encoding.
    Hashes and metrics are computed after this step by the caller.
    """
    if not materials:
        return
    names = {copy.name: original.name for original, copy in materials.items()}
    temporary = path.with_suffix(".names.glb")
    try:
        with path.open("rb") as source, temporary.open("wb") as target:
            magic, version, total, length, kind = struct.unpack("<5I", source.read(20))
            document = json.loads(source.read(length))
            for material in document.get("materials", []):
                if material.get("name") in names:
                    material["name"] = names[material["name"]]
            chunk = json.dumps(document, separators=(",", ":"), ensure_ascii=False).encode()
            chunk += b" " * (-len(chunk) % 4)
            target.write(
                struct.pack("<5I", magic, version, total - length + len(chunk), len(chunk), kind)
            )
            target.write(chunk)
            shutil.copyfileobj(source, target)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def export_lod1(objects, full_path, path, policy):
    """Decimate each shared evaluated mesh once, preserving effective material slots.

    Core policy defines the mesh threshold, triangle ratio and maximum image edge.
    Copy the full GLB only when both geometry and images need no reduction.
    Images, affected materials and nested node groups are copied before resampling.
    Empty nodes are omitted and their transforms baked into the mesh instances.
    Evaluated mesh identity keeps different modifier results separate without a
    partial modifier signature that could merge unequal geometry. Author data is
    never modified.
    """
    graph = bpy.context.evaluated_depsgraph_get()
    selected = set(objects)
    meshes = []
    frozen = {}
    placements = []
    copies = []
    collection = None
    try:
        for instance in graph.object_instances:
            original = instance.object.original
            parent = instance.parent.original if instance.parent else None
            if original not in selected and (not instance.is_instance or parent not in selected):
                continue
            if instance.object.type != "MESH":
                continue
            materials = tuple(
                slot.material.original if slot.material else None
                for slot in instance.object.material_slots
            )
            key = (instance.object.data.as_pointer(), materials)
            if key not in frozen:
                mesh = bpy.data.meshes.new_from_object(
                    instance.object, preserve_all_data_layers=True, depsgraph=graph
                )
                meshes.append(mesh)
                # Original IDs outlive depsgraph updates; evaluated material IDs do not.
                for index, material in enumerate(materials):
                    mesh.materials[index] = material
                triangles = sum(len(polygon.vertices) - 2 for polygon in mesh.polygons)
                frozen[key] = (mesh, triangles)
            placements.append((key, instance.matrix_world.copy()))
        require(meshes, "Entry has no mesh geometry")
        materials = {material for _, slots in frozen for material in slots if material}
        with lod1_materials(materials, policy["image_dimension"]) as replacements:
            if not replacements and not any(
                triangles >= policy["min_triangles"] for _, triangles in frozen.values()
            ):
                shutil.copyfile(full_path, path)
                return
            for mesh, _ in frozen.values():
                for index, material in enumerate(mesh.materials):
                    if material in replacements:
                        mesh.materials[index] = replacements[material]
            collection = bpy.data.collections.new("rou_lod1_temporary")
            bpy.context.scene.collection.children.link(collection)
            simplified = {}
            for key, matrix in placements:
                mesh, triangles = frozen[key]
                mesh = simplified.get(key, mesh)
                obj = bpy.data.objects.new(mesh.name + "_lod1", mesh)
                collection.objects.link(obj)
                obj.matrix_world = matrix
                copies.append(obj)
                if key not in simplified and triangles >= policy["min_triangles"]:
                    modifier = obj.modifiers.new("rou_lod1", "DECIMATE")
                    modifier.ratio = policy["ratio"]
                    modifier.use_collapse_triangulate = True
                    with export_visibility([obj]):
                        bpy.ops.object.modifier_apply(modifier=modifier.name)
                    if obj.data != mesh:
                        meshes.append(obj.data)
                simplified[key] = obj.data
            with export_visibility(copies):
                export_glb(path)
            restore_material_names(path, replacements)
    finally:
        for obj in copies:
            bpy.data.objects.remove(obj, do_unlink=True)
        if collection is not None:
            bpy.data.collections.remove(collection)
        for mesh in meshes:
            bpy.data.meshes.remove(mesh)
        bpy.context.view_layer.update()


def describe_glb(path, relative, core):
    """Delegate all output statistics, hashing and safety validation to the native core."""
    result = core.externalise_glb(path, texture_prefix=Path(relative).parent.as_posix() + "/")
    return {"path": relative, **result["descriptor"]}, result["metrics"], result["textures"]


def export_model(model, request, core=None):
    if core is None:
        from . import core
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
    lod1_relative = model["lod1_path"]
    lod1_path = Path(request["output_dir"]) / lod1_relative
    try:
        for root, _ in locations:
            root.location = (0, 0, 0)
        bpy.context.view_layer.update()
        with export_visibility(objects):
            bounds = local_bounds(objects)
            export_glb(path)
            export_lod1(objects, path, lod1_path, core.CONFIG["lod1"])
    finally:
        for root, location in locations:
            root.location = location
        bpy.context.view_layer.update()
    asset, full_metrics, full_textures = describe_glb(path, model["asset_path"], core)
    lod1, lod1_metrics, lod1_textures = describe_glb(lod1_path, lod1_relative, core)
    return {
        "id": model["id"],
        "asset": asset,
        "lod1": lod1,
        "metrics": {"asset": full_metrics, "lod1": lod1_metrics},
        "textures": {"asset": full_textures, "lod1": lod1_textures},
        "bounds_blender_m": bounds,
        "dependencies": dependencies,
    }
