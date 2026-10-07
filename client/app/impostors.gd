## One metadata-only box per catalog entity. All mutations happen on the main thread.
extends MultiMeshInstance3D
const Core = preload("core.gd")
const Loader = preload("loader.gd")
const AVAILABLE := Color(.43, .62, .67, .45)
const MISSING := Color(1.0, .38, .12, .72)
const INVALID := Color(.95, .15, .32, .72)
var instances: Dictionary = {}


func setup(items: Array) -> void:
	multimesh = MultiMesh.new()
	multimesh.transform_format = MultiMesh.TRANSFORM_3D
	multimesh.use_colors = true
	var box := BoxMesh.new()
	box.size = Vector3.ONE
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.vertex_color_use_as_albedo = true
	material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	box.material = material
	multimesh.mesh = box
	multimesh.instance_count = items.size()
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	var bounds := AABB()
	for index: int in items.size():
		var item: Dictionary = items[index]
		var center := Core.vec(item.center_m)
		var size_m := Core.vec(item.size_m)
		var placement := Transform3D(Basis.from_scale(size_m), center)
		instances[item.id] = {"index": index, "transform": placement, "visible": true, "error": ""}
		multimesh.set_instance_transform(index, placement)
		multimesh.set_instance_color(index, AVAILABLE)
		var entity_bounds := AABB(center - size_m * .5, size_m)
		bounds = entity_bounds if index == 0 else bounds.merge(entity_bounds)
	multimesh.custom_aabb = bounds


func set_visible_entity(id: String, show_box: bool) -> void:
	if not instances.has(id) or instances[id].visible == show_box:
		return
	var item: Dictionary = instances[id]
	item.visible = show_box
	multimesh.set_instance_transform(
		item.index,
		(
			item.transform
			if show_box
			else Transform3D(Basis.from_scale(Vector3.ZERO), item.transform.origin)
		)
	)


func apply(visible_ids: Array, hidden_ids: Array) -> void:
	var visible_set: Dictionary = {}
	for id: String in visible_ids:
		visible_set[id] = true
	for id: String in hidden_ids:
		visible_set.erase(id)
	for id: String in instances:
		set_visible_entity(id, visible_set.has(id))


func reject(id: String, error: String, code: String) -> void:
	if instances.has(id):
		instances[id].error = error
		instances[id].error_code = code
		multimesh.set_instance_color(
			instances[id].index, MISSING if code == Loader.MISSING_ASSET else INVALID
		)


func clear_error(id: String) -> void:
	if instances.has(id):
		instances[id].error = ""
		instances[id].erase("error_code")
		multimesh.set_instance_color(instances[id].index, AVAILABLE)
