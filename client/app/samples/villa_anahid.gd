## This optional house presentation owns its local itinerary, rooms and lighting.
## Nothing here is stored in a realm or a browsing bundle.
extends Node3D
const Core = preload("res://core.gd")
var host: Node3D
var data: Dictionary = {}
var camera: Camera3D
var overlay: Control
var choices: OptionButton
var title: Label
var description: Label
var play: Button
var map_button: Button
var mini: Control
var station := 0
var autoplay := false
var elapsed := 0.0
var practical_lights: Array[Light3D] = []
var probes: Array[ReflectionProbe] = []
var probes_dirty := false
var active := false
var last_warm := false


func _ready() -> void:
	host = get_parent()
	camera = host.camera
	var path: String = host.options.get("sample-data", "res://samples/villa_anahid.json")
	var bytes: Dictionary = host.LocalCatalog.read_bytes(
		path, Core.config().limits.safety.json_bytes
	)
	if bytes.has("error"):
		host.fail(bytes.error)
		return
	var decoded := Core.invoke("json.strict", bytes.bytes)
	if not decoded.ok or not decoded.value is Dictionary:
		host.fail("Could not read the sample itinerary")
		return
	data = decoded.value
	if not data.get("waypoints") is Array or not data.get("listing") is Dictionary:
		host.fail("Sample data needs waypoints and listing details")
		return
	for waypoint: Variant in data.waypoints:
		if not waypoint is Dictionary or not waypoint.get("name") is String:
			host.fail("Invalid sample waypoint")
			return
		var pose := Core.invoke(
			"viewer.pose",
			{"position": waypoint.get("position"), "look_at": waypoint.get("look_at")}
		)
		if not pose.ok:
			host.fail(Core.message(pose))
			return
	host.realm_opened.connect(opened)
	host.realm_cleared.connect(cleared)
	host.asset_loaded.connect(
		func(node):
			if active:
				polish_materials(node)
				probes_dirty = true
	)
	setup_ui()


func setup_ui() -> void:
	overlay = Control.new()
	overlay.mouse_filter = Control.MOUSE_FILTER_IGNORE
	host.ui.add_child(overlay)
	var panel: PanelContainer = host.panel(
		overlay, Vector2(460, 790), Vector2(1380, 175), Color("14242bf0")
	)
	var column := VBoxContainer.new()
	panel.add_child(column)
	title = host.label(str(data.listing.get("address", "Villa Anahid")), 24, column)
	description = host.label("Local house study \u00b7 Choose a view to begin", 17, column)
	var row := HBoxContainer.new()
	column.add_child(row)
	choices = OptionButton.new()
	choices.custom_minimum_size.x = 320
	row.add_child(choices)
	for waypoint: Dictionary in data.waypoints:
		choices.add_item(waypoint.name)
	choices.item_selected.connect(visit)
	host.button("Previous", row).pressed.connect(
		func(): visit(posmod(station - 1, data.waypoints.size()))
	)
	host.button("Next", row).pressed.connect(func(): visit((station + 1) % data.waypoints.size()))
	play = host.button("Begin tour", row)
	play.pressed.connect(toggle_tour)
	map_button = host.button("Rooms by floor", row)
	map_button.pressed.connect(func(): mini.visible = not mini.visible)
	mini = preload("site_map.gd").new()
	mini.viewer = self
	mini.position = Vector2(1160, 145)
	mini.size = Vector2(682, 610)
	overlay.add_child(mini)
	mini.hide()
	overlay.hide()


func opened() -> void:
	active = (
		host.catalog.has("walsh_site")
		or host.catalog.keys().any(func(id): return str(id).begins_with("walsh_"))
	)
	overlay.visible = active
	if not active:
		return
	setup_listing_lights()
	if int(data.listing.get("visual_version", 0)) >= 3:
		setup_interior_lights()
	probes_dirty = true
	# Default world framing and --at remain authoritative until a player chooses a route.
	if host.options.has("station"):
		visit(clampi(int(host.options.station), 0, data.waypoints.size() - 1))


func cleared() -> void:
	active = false
	autoplay = false
	if overlay != null:
		overlay.hide()
		play.text = "Begin tour"
	for node: Node in get_children():
		remove_child(node)
		node.queue_free()
	practical_lights.clear()
	probes.clear()
	probes_dirty = false


func visit(index: int) -> void:
	if not active or data.waypoints.is_empty():
		return
	station = index
	elapsed = 0
	var waypoint: Dictionary = data.waypoints[index]
	host.set_pose({"position": waypoint.position, "look_at": waypoint.look_at})
	choices.select(index)
	title.text = waypoint.name
	description.text = waypoint.get("description", "")


func toggle_tour() -> void:
	autoplay = not autoplay
	play.text = "Pause tour" if autoplay else "Begin tour"
	if autoplay:
		visit(station)


func _process(delta: float) -> void:
	if not active:
		return
	elapsed += delta
	if autoplay and elapsed >= 8:
		visit((station + 1) % data.waypoints.size())
	if probes_dirty and host.queue.is_empty():
		rebuild_probes()
	if host.warm != last_warm:
		last_warm = host.warm
		for light: Light3D in practical_lights:
			light.visible = host.warm or light.has_meta("interior")
	if mini.visible:
		mini.queue_redraw()


func _unhandled_input(event: InputEvent) -> void:
	if not active or host.get_viewport().gui_get_focus_owner() is LineEdit:
		return
	if event is InputEventKey and event.pressed:
		if event.keycode == KEY_SPACE:
			toggle_tour()
		elif event.keycode == KEY_BRACKETRIGHT:
			visit((station + 1) % data.waypoints.size())
		elif event.keycode == KEY_BRACKETLEFT:
			visit(posmod(station - 1, data.waypoints.size()))
		elif event.keycode >= KEY_1 and event.keycode <= KEY_8:
			visit(event.keycode - KEY_1)
		elif event.physical_keycode in [KEY_W, KEY_A, KEY_S, KEY_D, KEY_Q, KEY_E]:
			autoplay = false
			play.text = "Begin tour"


func style(color: Color) -> StyleBoxFlat:
	return host.style(color)


func focus_room(room: Dictionary) -> void:
	autoplay = false
	play.text = "Begin tour"
	if (
		room.has("station")
		and not str(room.id).begins_with("suite_l3_")
		and not str(room.id).begins_with("adu_bed_")
	):
		visit(int(room.station))
		return
	var rect: Array = room.rect
	var upper: bool = room.floor == "ADU" if room.floor is String else int(room.floor) == 3
	var y := 8.4 if upper else 4.2
	host.set_pose(
		{
			"position":
			[
				rect[0] + rect[2] * (.40 if str(room.id).begins_with("adu_bed_") else .75),
				y + 1.65,
				rect[1] + rect[3] - (.40 if str(room.id).begins_with("adu_bed_") else .85)
			],
			"look_at": [rect[0] + rect[2] * .43, y + 1.05, rect[1] + rect[3] * .45]
		}
	)
	title.text = room.name
	description.text = "Approximate room arrangement"


func setup_listing_lights() -> void:
	# Local fill approximates diffuse bounce. Small fixtures and shadowed facade spots
	# give the light visible sources; this is real-time lighting, not a baked photo.
	for p: Vector3 in [
		Vector3(-5, 3.7, 0),
		Vector3(0, 7.95, 1),
		Vector3(-9, 7.95, -2),
		Vector3(-9, 11.45, 7),
		Vector3(7, 7.9, 9),
		Vector3(15, 7.9, -3),
		Vector3(-8, 11.45, -11),
		Vector3(3, 11.45, -11),
		Vector3(10, 11.3, -24)
	]:
		var light := SpotLight3D.new()
		add_child(light)
		light.position = p
		light.rotation_degrees.x = -90
		light.light_color = Color("ffe5c4")
		light.light_energy = 2.2
		light.spot_range = 10
		light.spot_angle = 82
		light.spot_attenuation = 1.3
		light.shadow_enabled = p.z > -5
		light.shadow_blur = 1.7
		practical_lights.append(light)
	for x: float in [-8.9, -7.1, 2.0, 4.0]:
		var wash := SpotLight3D.new()
		add_child(wash)
		wash.position = Vector3(x, 7.85, 13.75)
		wash.rotation_degrees.x = -82
		wash.light_color = Color("ffca8c")
		wash.light_energy = 2.7
		wash.spot_range = 8
		wash.spot_angle = 38
		wash.spot_attenuation = 1.1
		wash.shadow_enabled = true
		wash.shadow_blur = 1.6
		practical_lights.append(wash)
	for x: float in [-8.9, -7.1, 2.0, 4.0]:
		var light := SpotLight3D.new()
		add_child(light)
		light.position = Vector3(x, 2.48, 13.57)
		light.rotation_degrees.x = -70
		light.light_color = Color("ffc080")
		light.light_energy = 4.2
		light.spot_range = 4
		light.spot_angle = 48
		light.spot_attenuation = 1.5
		light.shadow_enabled = true
		practical_lights.append(light)
	for p: Vector3 in [Vector3(-3, 7.7, -4.7), Vector3(4.8, 7.8, 4), Vector3(-3, 3.5, -3)]:
		var light := OmniLight3D.new()
		add_child(light)
		light.position = p
		light.light_color = Color("ffbb72")
		light.light_energy = 1.5
		light.omni_range = 4
		practical_lights.append(light)
	for p: Vector3 in [Vector3(12, 4.35, 4), Vector3(18, 4.35, 10)]:
		var light := OmniLight3D.new()
		add_child(light)
		light.position = p
		light.light_color = Color("81dbcd")
		light.light_energy = .55
		light.omni_range = 6
		practical_lights.append(light)
	# Broad, low-level bounce fill keeps deep interiors readable while the direct
	# downlights and SDFGI retain contact shadows and warm surface-to-surface light.
	for p: Vector3 in [
		Vector3(-1, 4.8, 10),
		Vector3(-7, 5.9, -1),
		Vector3(3, 5.9, 1),
		Vector3(-5, 1.7, 0),
		Vector3(-9, 10, 7),
		Vector3(-8, 10, -11),
		Vector3(3, 10, -11),
		Vector3(10, 10, -24)
	]:
		var fill := OmniLight3D.new()
		add_child(fill)
		fill.position = p
		fill.light_color = Color("ffe8cf")
		fill.light_energy = .85
		fill.omni_range = 10
		fill.omni_attenuation = 1.0
		fill.light_indirect_energy = 1.3
		practical_lights.append(fill)
	# Created after the initial asset queue is drained, so reflections include the house.


func setup_interior_lights() -> void:
	# V3 room positions: keep lighting attached to the spaces actually modelled.
	for light: Light3D in practical_lights:
		light.queue_free()
	practical_lights.clear()
	for spec: Array in [
		[Vector3(-10, 7.95, -9), 9.0],
		[Vector3(-9, 7.95, 0), 8.0],
		[Vector3(4, 7.95, 1), 7.0],
		[Vector3(2, 7.95, -6), 7.0],
		[Vector3(-2, 7.95, -12), 4.0],
		[Vector3(5, 7.95, -11), 5.0],
		[Vector3(-19, 7.95, -8), 7.0],
		[Vector3(-5, 3.85, 0), 9.0],
		[Vector3(-10, 3.6, -10), 9.0],
		[Vector3(-11, 11.4, 8), 7.0],
		[Vector3(-12.7, 11.4, 2.7), 4.0],
		[Vector3(7, 11.4, -10), 6.0],
		[Vector3(-12, 11.4, -9), 5.0],
		[Vector3(-6, 11.4, -9), 5.0],
		[Vector3(0, 11.4, -9), 5.0],
		[Vector3(7, 11.2, -22), 6.0],
		[Vector3(14, 11.2, -22), 6.0],
		[Vector3(7, 11.2, -26), 5.0],
		[Vector3(14, 11.2, -26), 5.0]
	]:
		var spot := SpotLight3D.new()
		add_child(spot)
		spot.position = spec[0]
		spot.rotation_degrees.x = -90
		spot.light_color = Color("fff0da")
		spot.light_energy = 1.30
		spot.spot_range = spec[1]
		spot.spot_angle = 82
		spot.spot_attenuation = 1.0
		# Shadow maps on the principal living spaces; AO and diffuse fill support
		# smaller enclosed rooms without rendering the entire house for every lamp.
		spot.shadow_enabled = (
			spot.position
			in [
				Vector3(-10, 7.95, -9),
				Vector3(4, 7.95, 1),
				Vector3(2, 7.95, -6),
				Vector3(-5, 3.85, 0),
				Vector3(-10, 3.6, -10),
				Vector3(-11, 11.4, 8)
			]
		)
		spot.shadow_blur = 2.0
		spot.set_meta("interior", true)
		practical_lights.append(spot)
		var fill := OmniLight3D.new()
		add_child(fill)
		fill.position = spot.position - Vector3(0, 1.7, 0)
		fill.light_color = Color("fff0de")
		fill.light_energy = .80
		fill.omni_range = spec[1]
		fill.omni_attenuation = 1.0
		fill.set_meta("interior", true)
		practical_lights.append(fill)
	for p: Vector3 in [
		Vector3(-10, 6.3, -13.7),
		Vector3(-5.5, 2.1, -3.8),
		Vector3(8.1, 6.4, 1.5),
		Vector3(-12.8, 10.3, 1.5)
	]:
		var accent := OmniLight3D.new()
		add_child(accent)
		accent.position = p
		accent.light_color = Color("ffdab0")
		accent.light_energy = .50
		accent.omni_range = 3.5
		practical_lights.append(accent)
	for p: Vector3 in [Vector3(-3, 2.6, 10), Vector3(-3, 6.6, 10), Vector3(-3, 10.6, 10)]:
		var fill := OmniLight3D.new()
		add_child(fill)
		fill.position = p
		fill.light_color = Color("ffebd5")
		fill.light_energy = 1.0
		fill.omni_range = 7
		fill.omni_attenuation = 1.3
		fill.set_meta("interior", true)
		practical_lights.append(fill)
	for x: float in [-8.9, -7.1, 2.0, 4.0]:
		var wash := SpotLight3D.new()
		add_child(wash)
		wash.position = Vector3(x, 7.85, 13.75)
		wash.rotation_degrees.x = -82
		wash.light_color = Color("ffce9c")
		wash.light_energy = 2.7
		wash.spot_range = 8
		wash.spot_angle = 38
		wash.shadow_enabled = true
		wash.shadow_blur = 1.6
		practical_lights.append(wash)
	for p: Vector3 in [Vector3(12, 4.35, 4), Vector3(18, 4.35, 10)]:
		var light := OmniLight3D.new()
		add_child(light)
		light.position = p
		light.light_color = Color("81dbcd")
		light.light_energy = .55
		light.omni_range = 6
		practical_lights.append(light)
	host.set_time(host.warm)


func rebuild_probes() -> void:
	for probe: ReflectionProbe in probes:
		probe.queue_free()
	probes.clear()
	if not data.has("listing"):
		probes_dirty = false
		return
	var specs: Array = [
		[Vector3(15, 6, 6), Vector3(19, 12, 27)],
		[Vector3(-3, 6, 0), Vector3(24, 4, 12)],
		[Vector3(-6, 2, 0), Vector3(21, 4, 12)]
	]
	if int(data.get("listing", {}).get("visual_version", 0)) >= 3:
		specs.append([Vector3(-10, 6.2, -10), Vector3(11, 4, 10)])
		specs.append([Vector3(-12.7, 10, 2.7), Vector3(4.3, 3.2, 3.6)])
	for spec: Array in specs:
		var probe := ReflectionProbe.new()
		probe.position = spec[0]
		probe.size = spec[1]
		probe.box_projection = true
		probe.max_distance = 90
		probe.intensity = .75
		probe.blend_distance = 1.5
		probe.update_mode = ReflectionProbe.UPDATE_ONCE
		add_child(probe)
		probes.append(probe)
	probes_dirty = false


func polish_materials(node: Node) -> void:
	if node is MeshInstance3D:
		for i: int in node.mesh.get_surface_count():
			var m: Material = node.mesh.surface_get_material(i)
			if m is StandardMaterial3D:
				if m.resource_name == "glass":
					m.cull_mode = BaseMaterial3D.CULL_DISABLED
					node.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
				if m.resource_name == "water":
					var shader := ShaderMaterial.new()
					shader.shader = load("res://samples/water.gdshader")
					node.set_surface_override_material(i, shader)
				if m.resource_name == "Honey onyx":
					m.emission_enabled = true
					m.emission = Color(.6, .32, .11)
					m.emission_energy_multiplier = .24
				if m.resource_name == "light":
					m.emission_energy_multiplier = 3.0
				if m.resource_name.begins_with("leaf"):
					m.cull_mode = BaseMaterial3D.CULL_DISABLED
				if m.resource_name == "glass":
					m.roughness = .08
					m.metallic = .0
					m.albedo_color = Color(.82, .9, .92, .075)
	for child: Node in node.get_children():
		polish_materials(child)
