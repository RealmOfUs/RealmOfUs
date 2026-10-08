## General world browser. Sample presentations own their routes separately.
extends RefCounted
const Core = preload("core.gd")
var host: Node3D
var root: Control
var tabs: TabContainer
var region_map: Control
var layer_input: SpinBox
var region_state: Label
var coordinate_input: LineEdit
var chunk_input: LineEdit
var entity_list: ItemList
var entity_search: LineEdit
var entity_details: Label
var version_details: Label
var visible_ids: Array[String] = []
var selection := ""
var summary: Label
var radius_inputs: Array[SpinBox] = []
var settings_status: Label


func setup(viewer: Node3D) -> void:
	host = viewer
	host.ui = CanvasLayer.new()
	host.add_child(host.ui)
	root = Control.new()
	host.ui.add_child(root)
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var top := HBoxContainer.new()
	root.add_child(top)
	top.position = Vector2(32, 24)
	top.size = Vector2(1856, 64)
	top.add_theme_constant_override("separation", 12)
	host.label("Realm of Us", 28, top)
	var spacer := Control.new()
	spacer.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	top.add_child(spacer)
	host.open_button = host.button("Open world\u2026", top)
	host.open_button.pressed.connect(host.choose_realm)
	host.reload_button = host.button("Reload", top)
	host.reload_button.pressed.connect(func(): host.open_realm(host.realm_path))
	host.button("Overview", top).pressed.connect(host.overview)
	host.button("Recent location", top).pressed.connect(host.resume_location)
	host.button("Day / dusk", top).pressed.connect(func(): host.set_time(not host.warm))
	var sidebar: PanelContainer = host.panel(
		root, Vector2(32, 116), Vector2(400, 818), Color("10212aee")
	)
	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 12)
	sidebar.add_child(column)
	summary = host.label("Open an exported world", 18, column)
	summary.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	tabs = TabContainer.new()
	tabs.size_flags_vertical = Control.SIZE_EXPAND_FILL
	column.add_child(tabs)
	var region := page("Places")
	host.label("Choose a place to explore", 17, region)
	region_map = preload("region_map.gd").new()
	region_map.viewer = host
	region_map.custom_minimum_size = Vector2(320, 280)
	region.add_child(region_map)
	var layer_row := HBoxContainer.new()
	region.add_child(layer_row)
	host.label("Map layer Y", 15, layer_row)
	layer_input = SpinBox.new()
	layer_input.step = 1
	layer_row.add_child(layer_input)
	layer_input.value_changed.connect(
		func(value):
			region_map.layer = int(value)
			region_map.refresh()
	)
	region_state = host.label("", 15, region)
	region_state.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	coordinate_input = LineEdit.new()
	coordinate_input.placeholder_text = "Coordinates in metres: x,y,z"
	region.add_child(coordinate_input)
	coordinate_input.text_submitted.connect(func(_text): jump_to_input())
	host.button("Go to coordinates", region).pressed.connect(jump_to_input)
	chunk_input = LineEdit.new()
	chunk_input.placeholder_text = "Chunk: x,y,z"
	region.add_child(chunk_input)
	chunk_input.text_submitted.connect(func(_text): jump_to_chunk_input())
	host.button("Go to chunk", region).pressed.connect(jump_to_chunk_input)
	var entities := page("Entities")
	entity_search = LineEdit.new()
	entity_search.placeholder_text = "Find an entity"
	entities.add_child(entity_search)
	entity_search.text_changed.connect(func(_text): fill_entities())
	entity_list = ItemList.new()
	entity_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	entity_list.custom_minimum_size.y = 300
	entities.add_child(entity_list)
	entity_list.item_selected.connect(func(index): select_entity(visible_ids[index]))
	entity_list.item_activated.connect(
		func(index):
			select_entity(visible_ids[index])
			visit_entity()
	)
	entity_details = host.label("Select an entity", 15, entities)
	entity_details.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	host.button("Visit entity", entities).pressed.connect(visit_entity)
	version_details = host.label("No world open", 15, page("Version"))
	version_details.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	var settings := page("Settings")
	host.label("Viewing radii (metres)", 17, settings)
	for index: int in 3:
		host.label(["Near detail", "Middle detail", "Far boxes"][index], 15, settings)
		var input := SpinBox.new()
		input.min_value = .001
		input.max_value = Core.config().viewer.max_radius
		input.step = .001
		input.allow_greater = true
		input.value = host.radii[index]
		settings.add_child(input)
		radius_inputs.append(input)
	host.button("Apply radii", settings).pressed.connect(apply_radii)
	settings_status = host.label("Near <= middle <= far", 15, settings)
	settings_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	host.label("Boxes: blue = available\nOrange = missing\nRed = failed validation", 15, settings)
	var footer := VBoxContainer.new()
	root.add_child(footer)
	footer.position = Vector2(460, 985)
	host.status_label = host.label("Ready to open a world", 17, footer)
	(
		host
		. label(
			"RMB look \u00b7 WASD move \u00b7 Q/E down/up \u00b7 Shift faster \u00b7 F1 diagnostics \u00b7 F2 grid \u00b7 H hide UI",
			14,
			footer,
			Color("a1b8bd")
		)
	)
	host.diagnostic = host.label("", 16, root)
	host.diagnostic.position = Vector2(460, 116)
	host.diagnostic.hide()
	host.file_dialog = FileDialog.new()
	host.file_dialog.title = "Open an exported world folder or realm.json"
	host.file_dialog.access = FileDialog.ACCESS_FILESYSTEM
	host.file_dialog.file_mode = FileDialog.FILE_MODE_OPEN_ANY
	host.file_dialog.filters = PackedStringArray(["*.json ; World catalog"])
	host.file_dialog.file_selected.connect(host.open_realm)
	host.file_dialog.dir_selected.connect(host.open_realm)
	host.add_child(host.file_dialog)
	host.error_dialog = AcceptDialog.new()
	host.error_dialog.title = "Could not open world"
	host.error_dialog.min_size = Vector2i(620, 180)
	host.add_child(host.error_dialog)


func sync_radii() -> void:
	for index: int in radius_inputs.size():
		radius_inputs[index].value = host.radii[index]


func apply_radii() -> void:
	var values: Array = []
	for input: SpinBox in radius_inputs:
		input.apply()
		values.append(input.value)
	settings_status.text = (
		"Radii applied" if host.set_radii(values) else "Enter positive radii: near <= middle <= far"
	)


func page(title: String) -> VBoxContainer:
	var box := VBoxContainer.new()
	box.name = title
	box.add_theme_constant_override("separation", 12)
	tabs.add_child(box)
	return box


func opened() -> void:
	host.reload_button.disabled = false
	sync_radii()
	layer_input.min_value = host.layout.layers.min
	layer_input.max_value = host.layout.layers.max
	layer_input.value = 0
	region_map.layer = int(layer_input.value)
	selection = ""
	entity_details.text = "Select an entity"
	fill_entities()
	var descriptor: Dictionary = host.repository_info
	if descriptor.is_empty():
		summary.text = "Unbound local preview"
		version_details.text = "Unbound local preview\n\nThis bundle has no fixed Git version."
	else:
		var commit: String = descriptor.commit_oid
		summary.text = (
			"Fixed version \u00b7 " + commit.left(8) if not commit.is_empty() else "Working draft"
		)
		version_details.text = (
			"Realm\n%s\n\n%s\n\n%d entities \u00b7 %d chunks"
			% [
				descriptor.realm_id,
				commit if not commit.is_empty() else "Uncommitted worktree view",
				host.manifest.entity_count,
				host.total_chunks
			]
		)
	update()


func cleared() -> void:
	host.reload_button.disabled = true
	selection = ""
	visible_ids.clear()
	entity_list.clear()
	entity_details.text = "Select an entity"
	version_details.text = "No world open"
	summary.text = "Open an exported world"
	region_state.text = ""
	region_map.refresh()


func fill_entities() -> void:
	entity_list.clear()
	visible_ids.clear()
	var query := entity_search.text.strip_edges().to_lower()
	var ids: Array = host.catalog.keys()
	ids.sort()
	for id: String in ids:
		var entity: Dictionary = host.catalog[id]
		var name: String = entity.get("label", id)
		if not query.is_empty() and not query in (name + " " + id).to_lower():
			continue
		visible_ids.append(id)
		entity_list.add_item(name)


func tier_bytes(entity: Dictionary, tier: String) -> int:
	var total: int = entity[tier].bytes
	for texture: Dictionary in entity.textures[tier]:
		total += int(texture.bytes)
	return total


func select_entity(id: String) -> void:
	if not host.catalog.has(id):
		return
	selection = id
	var entity: Dictionary = host.catalog[id]
	entity_details.text = (
		"%s\n%s\n\nOwner chunk: %s\nNear: %.1f KiB\nMiddle: %.1f KiB"
		% [
			entity.get("label", id),
			id,
			str(entity.owner_chunk),
			tier_bytes(entity, "asset") / 1024.0,
			tier_bytes(entity, "lod1") / 1024.0
		]
	)
	if host.rejected.has(id):
		entity_details.text += "\n\n" + str(host.rejected[id])


func visit_entity() -> void:
	if host.layout.entities.has(selection):
		host.set_pose(host.layout.entities[selection].focus)


func jump_to_input() -> void:
	var result := Core.invoke("viewer.at", coordinate_input.text)
	if result.ok:
		host.navigate_position(Core.vec(result.value))
		coordinate_input.release_focus()
	else:
		region_state.text = Core.message(result)


func jump_to_chunk_input() -> void:
	if not host.jump_to_chunk(chunk_input.text):
		region_state.text = "Enter a chunk inside this world"
	chunk_input.release_focus()


func update() -> void:
	if host.manifest.is_empty():
		return
	if not selection.is_empty():
		select_entity(selection)
	region_state.text = (
		"%.1f, %.1f, %.1f m\n%s"
		% [
			host.camera.position.x,
			host.camera.position.y,
			host.camera.position.z,
			host.chunk_state(host.current_chunk_key)
		]
	)
	region_map.refresh()
