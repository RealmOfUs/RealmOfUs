import json

import bpy
from bpy.props import StringProperty

from . import core, exporter, source_browser


class ROU_OT_new_profile(bpy.types.Operator):
    bl_idname = "rou.new_profile"
    bl_label = "Start a default realm"

    def execute(self, context):
        if "rou_profile_json" in context.scene:
            self.report(
                {"ERROR"},
                "World parameters already exist; browse a realm catalog to load its world",
            )
            return {"CANCELLED"}
        context.scene["rou_profile_json"] = json.dumps(core.CONFIG["profile"])
        context.scene["rou_title"] = "My realm"
        context.scene["rou_license"] = "NOASSERTION"
        return {"FINISHED"}


class ROU_OT_mark(bpy.types.Operator):
    bl_idname = "rou.mark_entity"
    bl_label = "Make selected objects into entities"

    def execute(self, context):
        selected = list(context.selected_objects)
        if not selected:
            self.report({"ERROR"}, "Select an object first")
            return {"CANCELLED"}
        roots = []
        for obj in selected:
            if obj.parent is not None:
                continue
            if obj.get("rou_entity_id"):
                roots.append(obj)
                continue
            matrix = obj.matrix_world.copy()
            root = bpy.data.objects.new(obj.name + " entity", None)
            context.collection.objects.link(root)
            position = core.call(
                "blender.placement", {"direction": "to_realm", "position": list(matrix.translation)}
            )
            root.location = core.call(
                "blender.placement", {"direction": "to_blender", "position_mm": position}
            )
            context.view_layer.update()
            obj.parent = root
            obj.matrix_world = matrix
            exporter.assign_id(root)
            roots.append(root)
        if not roots:
            self.report({"ERROR"}, "Select a top-level object or its entity root")
            return {"CANCELLED"}
        bpy.ops.object.select_all(action="DESELECT")
        for root in roots:
            root.select_set(True)
        context.view_layer.objects.active = roots[0]
        return {"FINISHED"}


class ROU_OT_export_local_preview(bpy.types.Operator):
    bl_idname = "rou.export_local_preview"
    bl_label = "Export local preview"
    bl_description = "Export a standalone world for viewing without recording a repository version"
    directory: StringProperty(subtype="DIR_PATH")

    def invoke(self, context, event):
        context.window_manager.fileselect_add(self)
        return {"RUNNING_MODAL"}

    def execute(self, context):
        try:
            core.require(bool(self.directory), "Choose an export directory")
            exporter.export_scene(self.directory)
            self.report({"INFO"}, "Local preview exported; no repository version recorded")
            return {"FINISHED"}
        except Exception as e:
            self.report({"ERROR"}, str(e))
            return {"CANCELLED"}


class ROU_OT_grid(bpy.types.Operator):
    bl_idname = "rou.show_grid"
    bl_label = "Show chunk grid"

    def execute(self, context):
        p = core.call(
            "profile.validate", core.call("json.strict", context.scene["rou_profile_json"].encode())
        )
        existing = bpy.data.objects.get("ROU chunk guide")
        if existing:
            existing.hide_set(not existing.hide_get())
            return {"FINISHED"}
        grid = core.call("spatial.grid", p)
        vertices = [
            core.call("blender.placement", {"direction": "to_blender", "position_mm": p})
            for p in grid["vertices_mm"]
        ]
        edges = grid["edges"]
        mesh = bpy.data.meshes.new("Chunk guide")
        mesh.from_pydata(vertices, edges, [])
        obj = bpy.data.objects.new("ROU chunk guide", mesh)
        context.collection.objects.link(obj)
        obj.hide_render = True
        obj.show_in_front = True
        return {"FINISHED"}


class ROU_PT_panel(bpy.types.Panel):
    bl_label = "Realm of Us"
    bl_idname = "ROU_PT_panel"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "Realm"
    bl_ui_units_x = 22

    def draw(self, context):
        layout = self.layout
        layout.label(text="Git realm authoring \u2022 local")
        if "rou_profile_json" not in context.scene:
            layout.operator("rou.new_profile")
        layout.operator("rou.mark_entity")
        layout.operator("rou.browse_sources")
        if context.window_manager.rou_source_items:
            layout.prop(context.window_manager, "rou_source_filter", text="", icon="VIEWZOOM")
            layout.template_list(
                "ROU_UL_sources",
                "",
                context.window_manager,
                "rou_source_items",
                context.window_manager,
                "rou_source_index",
                rows=5,
            )
            layout.operator("rou.edit_source")
        layout.operator("rou.save_work")
        if source_browser.SESSION_ERROR:
            box = layout.box()
            box.label(text=source_browser.SESSION_ERROR, icon="ERROR")
        layout.operator("rou.save_source_only")
        layout.operator("rou.reapply_placement")
        if "rou_profile_json" in context.scene:
            p = json.loads(context.scene["rou_profile_json"])
            layout.label(
                text="Chunk: "
                + " \u00d7 ".join(f"{value / 1000:g}" for value in p["chunk_size_mm"])
                + " m"
            )
            if "rou_title" in context.scene:
                layout.prop(context.scene, '["rou_title"]', text="World name")
            if "rou_license" in context.scene:
                layout.prop(context.scene, '["rou_license"]', text="Content license")
            layout.operator("rou.show_grid")
            layout.operator("rou.register_entities")
            layout.operator("rou.write_placement")
            layout.operator("rou.export_local_preview")
        layout.label(text="Root = identity rotation / scale")


CLASSES = [
    ROU_OT_new_profile,
    ROU_OT_mark,
    ROU_OT_export_local_preview,
    ROU_OT_grid,
    ROU_PT_panel,
]


def register():
    core.call("config")
    source_browser.register()
    for c in CLASSES:
        bpy.utils.register_class(c)


def unregister():
    for c in reversed(CLASSES):
        bpy.utils.unregister_class(c)
    source_browser.unregister()
