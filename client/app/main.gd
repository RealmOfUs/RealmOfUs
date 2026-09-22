extends Node3D
signal realm_opened
signal realm_cleared
signal asset_loaded(node: Node3D)
const Core = preload("core.gd")
const LocalCatalog = preload("local_catalog.gd")
var camera := Camera3D.new()
var world := Node3D.new()
var grid := Node3D.new()
var sun := DirectionalLight3D.new()
var environment := Environment.new()
var world_ui: RefCounted
var ui: CanvasLayer
var status_label: Label
var diagnostic: Label
var reload_button: Button
var open_button: Button
var presentation: Node3D
var file_dialog: FileDialog
var error_dialog: AcceptDialog
var manifest: Dictionary = {}
var catalog: Dictionary = {}
var layout: Dictionary = {}
var native_catalog: Rou
var repository_info: Dictionary = {}
var known_chunks: Dictionary = {}
var total_chunks := 0
var current_version := ""
var current_chunk_key := ""
var realm_path := ""
var asset_root := ""
var loaded: Dictionary = {}
var loaded_bounds: Dictionary = {}
var rejected: Dictionary = {}
var prototypes: Dictionary = {}
var queue: Array[String] = []
var options: Dictionary = {}
var last_error := ""
var warm := false
var imports := 0
var asset_imports := 0
var unloads := 0
var cancellations := 0
var selected_chunks := 0
var started_at := 0
var last_tick := 0
var age := 0.0
var refresh_clock := 0.0
var recent_clock := 0.0
var first_ready := -1.0
var first_presented := -1.0
var frame_times: Array[float] = []
var import_times: Array[float] = []
var focused_frames := 0
var unfocused_frames := 0
var capture_done := false
var open_serial := 0
var recent_storage_path := "user://realm_positions.json"


func _ready() -> void:
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--"):
			var pair := arg.substr(2).split("=", true, 1)
			options[pair[0]] = pair[1] if pair.size() > 1 else "true"
	Engine.max_fps = 0 if options.has("benchmark") else 60
	setup_world()
	world_ui = preload("world_ui.gd").new()
	world_ui.setup(self)
	world_ui.cleared()
	set_time(false)
	if options.get("sample", "") == "villa-anahid":
		var sample = load("res://samples/villa_anahid.gd")
		if sample == null:
			fail("Sample presentation unavailable")
			return
		presentation = sample.new()
		add_child(presentation)
	started_at = Time.get_ticks_usec()
	last_tick = started_at
	if DisplayServer.get_name() != "headless":
		DisplayServer.window_move_to_foreground()
	var path: String = options.get("realm", "")
	if not path.is_empty():
		open_realm(path, options.get("commit", ""))

	else:
		choose_realm()


func choose_realm() -> void:
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	if not realm_path.is_empty():
		file_dialog.current_dir = realm_path.get_base_dir()
	file_dialog.popup_centered(Vector2i(1100, 700))


func open_realm(path: String, ref: String = "") -> bool:
	var result := LocalCatalog.read(path, ref)
	if result.has("error"):
		fail(result.error)
		return false
	var initial: Dictionary = result.layout.start
	var identity: String = result.repository.get("realm_id", "")
	var recent := read_recent()
	if not identity.is_empty() and recent.has(identity):
		var saved := Core.invoke("viewer.pose", recent[identity])
		if saved.ok:
			initial = saved.value
	if options.has("at"):
		var explicit := Core.invoke("viewer.at", options.at)
		if not explicit.ok:
			fail(Core.message(explicit))
			return false
		var aimed := Core.invoke(
			"viewer.aim", {"position": explicit.value, "look_at": result.layout.start.look_at}
		)
		if not aimed.ok:
			fail(Core.message(aimed))
			return false
		initial = aimed.value
	clear_realm()
	realm_path = result.path
	asset_root = result.asset_root
	manifest = result.manifest
	layout = result.layout
	native_catalog = result.native_catalog
	repository_info = result.repository
	known_chunks = layout.known.duplicate(true)
	total_chunks = known_chunks.size()
	current_version = repository_info.get("commit_oid", "")
	for entity: Dictionary in manifest.entities:
		catalog[entity.id] = entity
	setup_grid()
	set_pose(initial)
	world_ui.opened()
	realm_opened.emit()
	return true


func clear_realm() -> void:
	save_recent()
	realm_cleared.emit()
	open_serial += 1
	for node: Node in world.get_children():
		world.remove_child(node)
		node.queue_free()
	for node: Node in grid.get_children():
		grid.remove_child(node)
		node.queue_free()
	for prototype: Node in prototypes.values():
		prototype.free()
	prototypes.clear()
	manifest.clear()
	catalog.clear()
	layout.clear()
	native_catalog = null
	repository_info.clear()
	known_chunks.clear()
	loaded.clear()
	loaded_bounds.clear()
	rejected.clear()
	queue.clear()
	frame_times.clear()
	import_times.clear()
	total_chunks = 0
	current_version = ""
	current_chunk_key = ""
	realm_path = ""
	asset_root = ""
	imports = 0
	asset_imports = 0
	unloads = 0
	cancellations = 0
	selected_chunks = 0
	first_ready = -1
	first_presented = -1
	focused_frames = 0
	unfocused_frames = 0
	started_at = Time.get_ticks_usec()
	last_tick = started_at
	age = 0
	refresh_clock = 0
	capture_done = false
	last_error = ""
	grid.hide()
	if world_ui != null:
		world_ui.cleared()
	if status_label != null:
		status_label.text = "Ready to open a world"


func fail(message: String) -> void:
	last_error = message
	push_error(message)
	status_label.text = message
	error_dialog.dialog_text = message
	if not options.has("benchmark"):
		error_dialog.popup_centered()


func read_recent() -> Dictionary:
	if options.has("no-recent") or not FileAccess.file_exists(recent_storage_path):
		return {}
	var bytes := LocalCatalog.read_bytes(recent_storage_path, Core.config().limits.json_bytes)
	if bytes.has("error"):
		return {}
	var result := Core.invoke("json.strict", bytes.bytes)
	return result.value if result.ok and result.value is Dictionary else {}


func save_recent() -> void:
	var identity: String = repository_info.get("realm_id", "")
	if identity.is_empty() or options.has("no-recent"):
		return
	var position := {
		"position": Core.array(camera.position),
		"look_at": Core.array(camera.position - camera.basis.z)
	}
	if not Core.invoke("viewer.pose", position).ok:
		return
	var recent := read_recent()
	recent[identity] = position
	var encoded := Core.invoke("json.pretty", recent)
	if not encoded.ok:
		return
	DirAccess.make_dir_recursive_absolute(recent_storage_path.get_base_dir())
	var file := FileAccess.open(recent_storage_path + ".tmp", FileAccess.WRITE)
	if file != null:
		file.store_string(encoded.value)
		file.close()
		DirAccess.rename_absolute(recent_storage_path + ".tmp", recent_storage_path)


func resume_location() -> void:
	var identity: String = repository_info.get("realm_id", "")
	var recent := read_recent()
	if recent.has(identity):
		var valid := Core.invoke("viewer.pose", recent[identity])
		if valid.ok:
			set_pose(valid.value)


func overview() -> void:
	if not layout.is_empty():
		set_pose(layout.start)


func set_pose(pose: Dictionary) -> void:
	camera.position = Core.vec(pose.position)
	var target := Core.vec(pose.look_at)
	var direction := target - camera.position
	if direction.length_squared() > 0.000001:
		camera.look_at(
			target,
			Vector3.FORWARD if absf(direction.normalized().dot(Vector3.UP)) > .999 else Vector3.UP
		)
	refresh_assets()
	if world_ui != null:
		world_ui.update()


func navigate_position(position: Vector3) -> void:
	set_pose({"position": Core.array(position), "look_at": Core.array(position - camera.basis.z)})


func navigate_to(position: Vector3, target: Vector3) -> void:
	var valid := Core.invoke(
		"viewer.pose", {"position": Core.array(position), "look_at": Core.array(target)}
	)
	if valid.ok:
		set_pose(valid.value)


func jump_to_chunk(selector: String) -> bool:
	if manifest.is_empty():
		return false
	var result := Core.invoke("viewer.chunk", {"profile": manifest.profile, "selector": selector})
	if not result.ok:
		return false
	var position := Core.vec(result.value.position)
	navigate_to(position, position + Vector3(0, -.3, -1))
	return true


func refresh_assets() -> void:
	if manifest.is_empty():
		return
	var response := native_catalog.catalog_query(
		JSON.stringify(Core.array(camera.position)).to_utf8_buffer()
	)
	if not response.ok:
		return
	var needed: Array = response.value.entity_ids
	selected_chunks = response.value.selected_chunks
	current_chunk_key = response.value.chunk_key
	for id: String in loaded.keys():
		if not id in needed:
			world.remove_child(loaded[id])
			loaded[id].queue_free()
			loaded.erase(id)
			loaded_bounds.erase(id)
			unloads += 1
	for id: String in queue:
		if not id in needed:
			cancellations += 1
	queue.clear()
	for id: String in needed:
		if not loaded.has(id) and not rejected.has(id):
			queue.append(id)


func import_one(id: String) -> void:
	if loaded.has(id) or not catalog.has(id):
		return
	var started := Time.get_ticks_usec()
	var entity: Dictionary = catalog[id]
	var asset: Dictionary = entity.asset
	if not prototypes.has(asset.sha256):
		var path: String = asset_root.path_join(asset.path)
		var folder := DirAccess.open(path.get_base_dir())
		if (
			DirAccess.open(asset_root).is_link("objects")
			or (folder != null and folder.is_link(path.get_file()))
		):
			rejected[id] = "Symlink asset forbidden"
			return
		var file := FileAccess.open(path, FileAccess.READ)
		if file == null:
			rejected[id] = "Asset missing"
			return
		if file.get_length() > Core.config().limits.glb_bytes:
			rejected[id] = "Asset size mismatch"
			return
		var bytes := file.get_buffer(file.get_length())
		var inspected := Core.invoke("glb.inspect", bytes)
		if not inspected.ok:
			rejected[id] = Core.message(inspected)
			return
		var matches := Core.invoke(
			"asset.match", {"asset": asset, "actual": inspected.value.descriptor}
		)
		if not matches.ok:
			rejected[id] = "Asset hash mismatch"
			return
		var document := GLTFDocument.new()
		var state := GLTFState.new()
		if document.append_from_buffer(bytes, "", state) != OK:
			rejected[id] = "glTF import failed"
			return
		var prototype: Node3D = document.generate_scene(state)
		if prototype == null:
			rejected[id] = "Empty glTF scene"
			return
		prototypes[asset.sha256] = prototype
		asset_imports += 1
	var scene: Node3D = prototypes[asset.sha256].duplicate()
	world.add_child(scene)
	scene.position = Core.vec(layout.entities[id].position)
	loaded[id] = scene
	var box := collect_bounds(scene)
	loaded_bounds[id] = {"min": Core.array(box.position), "max": Core.array(box.end)}
	asset_loaded.emit(scene)
	imports += 1
	import_times.append((Time.get_ticks_usec() - started) / 1000.0)


func chunk_state(key: String) -> String:
	if manifest.is_empty():
		return "Outside world"
	if not layout.known.has(key):
		return "Outside world"
	if not known_chunks.has(key):
		return "Unknown metadata"
	var ids: Array = known_chunks[key]
	if ids.is_empty():
		return "Known empty"
	for id: String in ids:
		if rejected.has(id):
			return "Asset missing" if rejected[id] == "Asset missing" else "Asset failed validation"
		if not FileAccess.file_exists(asset_root.path_join(catalog[id].asset.path)):
			return "Asset missing"
	for id: String in ids:
		if id in queue:
			return "Loading assets"
	return "Content available"


func setup_grid() -> void:
	var result := Core.invoke("spatial.grid", manifest.profile)
	if not result.ok:
		return
	var mesh := ImmediateMesh.new()
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = Color(.4, .7, .7, .6)
	material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mesh.surface_begin(Mesh.PRIMITIVE_LINES, material)
	var vertices: Array = []
	for point: Array in result.value.vertices_mm:
		vertices.append(Core.vec(Core.invoke("viewer.metres", point).value))
	for edge: Array in result.value.edges:
		mesh.surface_add_vertex(vertices[edge[0]])
		mesh.surface_add_vertex(vertices[edge[1]])
	mesh.surface_end()
	var instance := MeshInstance3D.new()
	instance.mesh = mesh
	grid.add_child(instance)
	grid.hide()


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_RIGHT:
		Input.mouse_mode = Input.MOUSE_MODE_CAPTURED if event.pressed else Input.MOUSE_MODE_VISIBLE
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		camera.rotation.y -= event.relative.x * .0025
		camera.rotation.x = clampf(
			camera.rotation.x - event.relative.y * .0025, -PI * .499, PI * .499
		)
	if event is InputEventKey and event.pressed:
		if event.keycode == KEY_ESCAPE:
			Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
		elif event.keycode == KEY_O and (event.ctrl_pressed or event.meta_pressed):
			choose_realm()
		elif event.keycode == KEY_F1:
			diagnostic.visible = not diagnostic.visible
		elif event.keycode == KEY_F2:
			grid.visible = not grid.visible
		elif event.keycode == KEY_H:
			ui.visible = not ui.visible


func _process(delta: float) -> void:
	var now := Time.get_ticks_usec()
	age = (now - started_at) / 1000000.0
	if first_ready >= 0 and age > first_ready + 2:
		frame_times.append((now - last_tick) / 1000.0)
		if get_window().has_focus():
			focused_frames += 1
		else:
			unfocused_frames += 1
	last_tick = now
	if manifest.is_empty():
		return
	if not queue.is_empty():
		import_one(queue.pop_front())
	elif first_ready < 0:
		first_ready = age
		if DisplayServer.get_name() != "headless":
			announce_ready.call_deferred(open_serial)
	var direction := Vector3.ZERO
	if (
		not get_viewport().gui_get_focus_owner() is LineEdit
		and not file_dialog.visible
		and not error_dialog.visible
	):
		direction = Vector3(
			(
				float(Input.is_physical_key_pressed(KEY_D))
				- float(Input.is_physical_key_pressed(KEY_A))
			),
			(
				float(Input.is_physical_key_pressed(KEY_E))
				- float(Input.is_physical_key_pressed(KEY_Q))
			),
			(
				float(Input.is_physical_key_pressed(KEY_S))
				- float(Input.is_physical_key_pressed(KEY_W))
			)
		)
	if direction != Vector3.ZERO:
		camera.position += (
			camera.basis
			* direction.normalized()
			* delta
			* (13 if Input.is_physical_key_pressed(KEY_SHIFT) else 4)
		)
	refresh_clock += delta
	recent_clock += delta
	if refresh_clock >= .35:
		refresh_clock = 0
		refresh_assets()
		world_ui.update()
	if recent_clock > 2:
		recent_clock = 0
		save_recent()
	status_label.text = (
		"%d loaded \u00b7 %d loading \u00b7 %d unavailable \u00b7 %s"
		% [loaded.size(), queue.size(), rejected.size(), chunk_state(current_chunk_key)]
	)
	if loaded.is_empty() and queue.is_empty() and rejected.is_empty() and not catalog.is_empty():
		status_label.text = "No nearby entities \u00b7 Choose a place on the map or in the entity list"
	diagnostic.text = (
		"%.1f FPS \u00b7 %d imported assets\nCamera %.2f, %.2f, %.2f m"
		% [
			Engine.get_frames_per_second(),
			asset_imports,
			camera.position.x,
			camera.position.y,
			camera.position.z
		]
	)
	if (
		options.has("screenshot")
		and not capture_done
		and first_ready >= 0
		and age > first_ready + float(options.get("capture-after", "4"))
		and queue.is_empty()
	):
		capture_done = true
		capture.call_deferred()
	if (
		options.has("benchmark")
		and first_ready >= 0
		and age > first_ready + float(options.benchmark)
	):
		options.erase("benchmark")
		finish_report.call_deferred()


func _exit_tree() -> void:
	save_recent()
	for prototype: Node in prototypes.values():
		prototype.free()
	prototypes.clear()


func setup_world() -> void:
	add_child(world)
	add_child(grid)
	add_child(camera)
	camera.current = true
	camera.fov = 55
	camera.near = .06
	camera.far = 350
	var we := WorldEnvironment.new()
	we.environment = environment
	add_child(we)
	environment.background_mode = Environment.BG_SKY
	var sky := Sky.new()
	var sky_mat := PanoramaSkyMaterial.new()
	sky_mat.panorama = load("res://assets/kloppenheim_06_puresky_2k.hdr")
	sky.sky_material = sky_mat
	sky.radiance_size = Sky.RADIANCE_SIZE_256
	environment.sky = sky
	environment.sky_rotation = Vector3(0, 1.8, 0)
	environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.ambient_light_color = Color("c5c9c9")
	environment.reflected_light_source = Environment.REFLECTION_SOURCE_SKY
	environment.tonemap_mode = Environment.TONE_MAPPER_ACES
	environment.tonemap_exposure = 1.1
	environment.ssao_enabled = true
	environment.ssao_radius = .65
	environment.ssao_intensity = 1.25
	environment.ssao_power = 1.1
	if not options.has("balanced"):
		environment.sdfgi_enabled = true
		environment.sdfgi_min_cell_size = .25
		environment.sdfgi_cascades = 4
		environment.sdfgi_use_occlusion = true
		environment.sdfgi_bounce_feedback = .45
		environment.sdfgi_read_sky_light = true
	environment.ssil_enabled = true
	environment.ssil_radius = 3.0
	environment.ssil_intensity = .65
	environment.glow_enabled = true
	environment.glow_intensity = .42
	environment.glow_bloom = .05
	environment.fog_enabled = true
	environment.fog_light_color = Color("536879")
	environment.fog_density = .00035
	environment.fog_sky_affect = .08
	add_child(sun)
	sun.rotation_degrees = Vector3(-12, -38, 0)
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 100
	sun.shadow_blur = 2


func label(
	text: String, size: int, parent: Node, color: Color = Color("f4efdf"), serif: bool = false
) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", size)
	l.add_theme_color_override("font_color", color)
	var font := SystemFont.new()
	font.font_names = PackedStringArray(
		["Georgia", "Times New Roman"] if serif else ["Helvetica Neue", "Arial"]
	)
	l.add_theme_font_override("font", font)
	parent.add_child(l)
	return l


func style(bg: Color, border: Color = Color("536159"), radius: int = 8) -> StyleBoxFlat:
	var s := StyleBoxFlat.new()
	s.bg_color = bg
	s.border_color = border
	s.set_border_width_all(1)
	s.set_corner_radius_all(radius)
	s.content_margin_left = 20
	s.content_margin_right = 20
	s.content_margin_top = 13
	s.content_margin_bottom = 13
	return s


func panel(parent: Node, pos: Vector2, size: Vector2, bg: Color) -> PanelContainer:
	var p := PanelContainer.new()
	parent.add_child(p)
	p.position = pos
	p.custom_minimum_size = size
	p.add_theme_stylebox_override("panel", style(bg))
	return p


func button(text: String, parent: Node) -> Button:
	var b := Button.new()
	parent.add_child(b)
	b.text = text
	b.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	b.add_theme_font_size_override("font_size", 17)
	b.add_theme_color_override("font_color", Color("e6e7dc"))
	b.add_theme_color_override("font_hover_color", Color.WHITE)
	b.add_theme_stylebox_override(
		"normal", style(Color(.045, .075, .063, .85), Color(.5, .6, .5, .22), 6)
	)
	b.add_theme_stylebox_override("hover", style(Color("34473b"), Color("a3b18b"), 6))
	b.add_theme_stylebox_override("pressed", style(Color("5c6850"), Color("c9d1b4"), 6))
	b.focus_mode = Control.FOCUS_NONE
	return b


func collect_bounds(node: Node3D) -> AABB:
	var result := AABB()
	var first := true
	var nodes: Array[Node] = [node]
	while not nodes.is_empty():
		var n: Node = nodes.pop_back()
		if n is MeshInstance3D:
			var b: AABB = n.global_transform * n.get_aabb()
			result = b if first else result.merge(b)
			first = false
		for c: Node in n.get_children():
			nodes.append(c)
	return result


func announce_ready(serial: int) -> void:
	await RenderingServer.frame_post_draw
	if serial != open_serial:
		return
	get_window().grab_focus()
	first_presented = (Time.get_ticks_usec() - started_at) / 1000000.0
	if options.has("report"):
		var ready_file := FileAccess.open(str(options.report) + ".ready.json", FileAccess.WRITE)
		if ready_file:
			ready_file.store_string(
				JSON.stringify({"realm": realm_path, "seconds": first_presented})
			)
	print("WORLD_READY ", JSON.stringify({"realm": realm_path, "seconds": first_presented}))


func capture() -> void:
	await RenderingServer.frame_post_draw
	var img: Image = get_viewport().get_texture().get_image()
	var err: Error = img.save_png(options.screenshot)
	print("SCREENSHOT ", options.screenshot, " ", err)


func percentile(a: Array[float], p: float) -> float:
	if a.is_empty():
		return 0
	var b: Array[float] = a.duplicate()
	b.sort()
	return b[mini(int(ceil(b.size() * p)) - 1, b.size() - 1)]


func finish_report() -> void:
	var report: Dictionary = {
		"realm": realm_path,
		"commit_oid": current_version,
		"view_root_sha256": repository_info.get("view_root_sha256", ""),
		"view_validation": repository_info.get("validation", "unversioned"),
		"seconds": age,
		"first_ready_seconds": first_ready,
		"first_presented_seconds": first_presented,
		"user_data_dir": OS.get_user_data_dir(),
		"frames": frame_times.size(),
		"focused_frames": focused_frames,
		"unfocused_frames": unfocused_frames,
		"frame_ms_p50": percentile(frame_times, .50),
		"frame_ms_p95": percentile(frame_times, .95),
		"frame_ms_p99": percentile(frame_times, .99),
		"import_ms_p95": percentile(import_times, .95),
		"imports": imports,
		"unloads": unloads,
		"cancelled": cancellations,
		"rejected": rejected,
		"loaded_bounds": loaded_bounds,
		"viewport":
		[get_viewport().get_texture().get_width(), get_viewport().get_texture().get_height()],
		"visual_preset": "daylight" if not warm else "dusk",
		"asset_imports": asset_imports,
		"ssil": environment.ssil_enabled,
		"sdfgi": environment.sdfgi_enabled,
		"render_scale": get_viewport().scaling_3d_scale,
		"msaa": get_viewport().msaa_3d,
		"renderer": RenderingServer.get_current_rendering_method(),
		"gpu": RenderingServer.get_video_adapter_name(),
		"godot": Engine.get_version_info().string,
		"static_memory_bytes": OS.get_static_memory_usage(),
		"render_objects": Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME),
		"render_primitives": Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME)
	}
	var f := FileAccess.open(options.get("report", "user://viewer-report.json"), FileAccess.WRITE)
	if f:
		f.store_string(JSON.stringify(report, "  ") + "\n")
	print("VIEWER_REPORT ", JSON.stringify(report))
	get_tree().quit(0 if rejected.is_empty() else 1)


func set_time(dusk: bool) -> void:
	warm = dusk
	sun.rotation_degrees = Vector3(-12, -38, 0) if dusk else Vector3(-48, -28, 0)
	sun.light_color = Color("ffc68f") if dusk else Color("fff0d7")
	sun.light_energy = .22 if dusk else .95
	environment.background_energy_multiplier = .14 if dusk else .4
	environment.ambient_light_energy = .30 if dusk else .5
	environment.tonemap_exposure = 1.15 if dusk else .9
