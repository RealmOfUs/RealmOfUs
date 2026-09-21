## Rendered acceptance of the viewer scenes using scripted input events.
## Run one suite at a time; performance uses the exported app separately.
extends SceneTree

var viewer: Node3D
var output := ""
var suite := ""
var workflow := ""
var checks := 0
var errors: Array[String] = []
var observations: Array[Dictionary] = []


func _initialize() -> void:
	run.call_deferred()


func check(value: bool, reason: String) -> void:
	checks += 1
	if not value:
		errors.append(reason)
		push_error(reason)


func click_at(position: Vector2) -> void:
	var event := InputEventMouseButton.new()
	event.button_index = MOUSE_BUTTON_LEFT
	event.position = position
	event.global_position = position
	event.pressed = true
	root.push_input(event, true)
	event = event.duplicate()
	event.pressed = false
	root.push_input(event, true)
	await process_frame


func click(control: Control) -> void:
	await click_at(control.get_global_rect().get_center())


func key(code: Key, command: bool = false) -> void:
	var event := InputEventKey.new()
	event.keycode = code
	event.physical_keycode = code
	event.meta_pressed = command
	event.pressed = true
	root.push_input(event, true)
	event = event.duplicate()
	event.pressed = false
	root.push_input(event, true)
	await process_frame


func settle() -> void:
	var start := Time.get_ticks_msec()
	while not viewer.queue.is_empty():
		if Time.get_ticks_msec() - start > 30000:
			check(false, "Loading timed out")
			break
		await process_frame
	await create_timer(2).timeout
	check(viewer.rejected.is_empty(), "All requested assets accepted")


func capture(name: String) -> void:
	await RenderingServer.frame_post_draw
	check(
		root.get_texture().get_image().save_png(output.path_join(name + ".png")) == OK,
		"Saved " + name
	)
	observations.append(
		{
			"image": name + ".png",
			"station": viewer.presentation.station if viewer.presentation != null else -1,
			"loaded": viewer.loaded.keys(),
			"focused": root.has_focus(),
			"camera": [viewer.camera.position.x, viewer.camera.position.y, viewer.camera.position.z]
		}
	)
	print("RENDERED_CAPTURE ", name)


func free_flight() -> void:
	var initial_position: Vector3 = viewer.camera.position
	var event := InputEventKey.new()
	event.keycode = KEY_W
	event.physical_keycode = KEY_W
	event.pressed = true
	Input.parse_input_event(event)
	await create_timer(.5).timeout
	event = event.duplicate()
	event.pressed = false
	Input.parse_input_event(event)
	await process_frame
	check(viewer.camera.position.distance_to(initial_position) > .5, "W moves the camera")
	var initial_basis: Basis = viewer.camera.basis
	var mouse := InputEventMouseButton.new()
	mouse.button_index = MOUSE_BUTTON_RIGHT
	mouse.position = Vector2(960, 400)
	mouse.pressed = true
	root.push_input(mouse, true)
	var motion := InputEventMouseMotion.new()
	motion.position = mouse.position
	motion.relative = Vector2(45, 15)
	root.push_input(motion, true)
	mouse = mouse.duplicate()
	mouse.pressed = false
	root.push_input(mouse, true)
	check(not viewer.camera.basis.is_equal_approx(initial_basis), "Right drag turns the camera")
	check(Input.mouse_mode == Input.MOUSE_MODE_VISIBLE, "Right release restores the pointer")
	await key(KEY_F2)
	check(viewer.grid.visible, "F2 shows chunk grid")
	await capture("free-flight-grid")
	await key(KEY_F2)


func view_places() -> void:
	if viewer.presentation != null:
		for index: int in viewer.presentation.data.waypoints.size():
			viewer.presentation.visit(index)
			await settle()
			await capture("sample-%02d" % index)
		return
	var fixture: Dictionary = (
		viewer
		. Core
		. invoke(
			"json.strict",
			FileAccess.get_file_as_bytes("res://../../toolchain/fixtures/calibration.json")
		)
		. value
	)
	for index: int in fixture.observations.size():
		var destination: Dictionary = fixture.observations[index]
		viewer.set_pose({"position": destination.position, "look_at": destination.look_at})
		await settle()
		await capture("place-%02d" % index)


func rapid_navigation() -> void:
	var before: int = viewer.cancellations
	viewer.navigate_position(Vector3(10000, 10000, 10000))
	var ids: Array = viewer.catalog.keys()
	viewer.world_ui.select_entity(ids[0])
	viewer.world_ui.visit_entity()
	viewer.navigate_position(Vector3(10000, 10000, 10000))
	check(viewer.cancellations > before, "Rapid navigation cancels obsolete queued imports")
	viewer.world_ui.visit_entity()
	await settle()


func house_controls() -> void:
	var sample: Node3D = viewer.presentation
	check(sample.map_button.visible, "House map is available")
	await click(sample.map_button)
	check(sample.mini.visible, "Map button opens local room plan")
	for floor_number: int in range(1, 5):
		await click_at(
			sample.mini.global_position + Vector2(24 + (floor_number - 1) * 158 + 70, 70)
		)
		check(sample.mini.selected_floor == floor_number, "Map floor " + str(floor_number))
		await capture("floor-%d" % floor_number)
	await click_at(sample.mini.global_position + Vector2(24 + 158 + 70, 70))
	await RenderingServer.frame_post_draw
	var kitchen_hit := false
	for hit: Dictionary in sample.mini.hit_rooms:
		if hit.room.id == "kitchen":
			await click_at(sample.mini.global_position + hit.rect.get_center())
			await settle()
			check(sample.station == int(hit.room.station), "Room map visits the kitchen")
			kitchen_hit = true
			break
	check(kitchen_hit, "Kitchen exists in local sample room plan")
	await capture("kitchen-map")
	await click(sample.map_button)


func authoring_reload() -> void:
	await key(KEY_O, true)
	check(viewer.file_dialog.visible, "Cmd+O opens the file picker")
	await capture("file-picker")
	viewer.file_dialog.hide()
	viewer.file_dialog.file_selected.emit(workflow.path_join("live/realm.json"))
	await settle()
	check(viewer.catalog.has("test_entity"), "Selected authored realm opens")
	check(not (viewer.presentation != null), "Authored realm has no room plan")
	var old_position: Array = viewer.catalog.test_entity.position_mm.duplicate()
	var old_bounds: Dictionary = viewer.loaded_bounds.duplicate(true)
	await capture("authored-before")
	var source := FileAccess.get_file_as_bytes(workflow.path_join("after/realm.json"))
	var live := FileAccess.open(workflow.path_join("live/realm.json"), FileAccess.WRITE)
	live.store_buffer(source)
	live.close()
	await click(viewer.reload_button)
	await settle()
	check(
		viewer.catalog.test_entity.position_mm != old_position, "Reload reads the edited placement"
	)
	check(viewer.loaded_bounds != old_bounds, "Edited placement replaces the prior world bounds")
	check(viewer.loaded.size() == 1, "Reload leaves exactly one entity")
	await capture("authored-after")
	await click(viewer.open_button)
	check(viewer.file_dialog.visible, "Open button opens the picker")
	viewer.file_dialog.hide()
	viewer.file_dialog.file_selected.emit(workflow.path_join("invalid/realm.json"))
	check(viewer.error_dialog.visible, "Invalid catalog shows an error")
	check(viewer.loaded.size() == 1, "Failed selection preserves the current model")
	await capture("invalid-preserves-world")
	viewer.error_dialog.hide()
	await free_flight()
	check(viewer.open_realm(workflow.path_join("empty/realm.json")), "Open empty world")
	await settle()
	check(viewer.loaded.is_empty(), "Empty world releases the prior model")
	check("empty" in viewer.status_label.text.to_lower(), "Empty world has explicit status")
	await capture("empty-world")
	check(viewer.open_realm(workflow.path_join("missing/realm.json")), "Open missing-asset catalog")
	while not viewer.queue.is_empty():
		await process_frame
	check(viewer.rejected.has("test_entity"), "Missing asset has explicit rejection")
	await capture("missing-asset")
	check(viewer.open_realm(workflow.path_join("after/realm.json")), "Recover from missing asset")
	await settle()
	check(viewer.loaded.size() == 1, "Recovery displays the valid model")


func run() -> void:
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--capture-dir="):
			output = arg.trim_prefix("--capture-dir=")
		elif arg.begins_with("--suite="):
			suite = arg.trim_prefix("--suite=")
		elif arg.begins_with("--workflow="):
			workflow = arg.trim_prefix("--workflow=")
	if output.is_empty() or suite not in ["calibration", "house", "authoring"]:
		push_error("Supply --capture-dir and --suite=calibration|house|authoring")
		quit(2)
		return
	DirAccess.make_dir_recursive_absolute(output)
	root.size = Vector2i(1920, 1080)
	viewer = load("res://main.tscn").instantiate()
	if suite == "house":
		viewer.options["sample"] = "villa-anahid"
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	await settle()
	check(DisplayServer.get_name() != "headless", "Actual display renderer")
	if suite == "authoring":
		await authoring_reload()
	else:
		await view_places()
		if suite == "house":
			await house_controls()
		else:
			await rapid_navigation()
		await free_flight()
	var report := {
		"suite": suite,
		"checks": checks,
		"errors": errors,
		"status": "passed" if errors.is_empty() else "failed",
		"executable": OS.get_executable_path(),
		"renderer": RenderingServer.get_current_rendering_method(),
		"interaction": "Scripted input and file selection callbacks in a real Godot window",
		"observations": observations
	}
	var file := FileAccess.open(output.path_join("report.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(report, "  ") + "\n")
	print("RENDERED_WORKFLOW_PASS" if errors.is_empty() else "RENDERED_WORKFLOW_FAIL")
	viewer.clear_realm()
	viewer.queue_free()
	await process_frame
	await process_frame
	quit(0 if errors.is_empty() else 1)
