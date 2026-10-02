## Local file selection, reload, failed-open recovery, empty worlds and source edits.
extends SceneTree
const Streaming = preload("streaming.gd")

var errors: Array[String] = []
var checks := 0


func check(value: bool, reason: String) -> void:
	checks += 1
	if not value:
		errors.append(reason)
		push_error(reason)


func _initialize() -> void:
	run_test.call_deferred()


func drain(viewer: Node3D) -> void:
	check(await Streaming.drain(viewer), "Background imports complete before deadline")
	check(viewer.rejected.is_empty(), "Assets accepted")


func run_test() -> void:
	var output := ""
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--workflow="):
			output = arg.trim_prefix("--workflow=")
	check(not output.is_empty(), "Workflow directory supplied")
	root.size = Vector2i(1920, 1080)
	var viewer = load("res://main.tscn").instantiate()
	root.add_child(viewer)
	viewer.set_process(false)
	check(viewer.manifest.is_empty(), "Start without a hardcoded house")
	check(viewer.reload_button.disabled, "Reload unavailable before opening a world")
	await process_frame
	var click := InputEventMouseButton.new()
	click.button_index = MOUSE_BUTTON_LEFT
	click.position = viewer.open_button.get_global_rect().get_center()
	click.global_position = click.position
	click.pressed = true
	root.push_input(click, true)
	click = click.duplicate()
	click.pressed = false
	root.push_input(click, true)
	check(viewer.file_dialog.visible, "Pointer opens the file picker")
	viewer.file_dialog.file_selected.emit(output.path_join("before/realm.json"))
	viewer.file_dialog.hide()
	check(viewer.catalog.has("test_entity"), "Picker selection loads the chosen realm")
	check(not (viewer.presentation != null), "Generic world has no house floor plan")
	check(viewer.presentation == null, "Generic world has no house-specific lights")
	await drain(viewer)
	check(viewer.loaded.size() == 1, "Default camera frames the authored entity")
	var original_bounds: Dictionary = viewer.loaded_bounds.duplicate(true)
	var original_path: String = viewer.realm_path
	var original_node: Node = viewer.loaded.values()[0]
	check(
		not viewer.open_realm(output.path_join("invalid/realm.json")), "Malformed catalog rejected"
	)
	check(viewer.realm_path == original_path, "Failed open preserves the current path")
	check(viewer.loaded.values()[0] == original_node, "Failed open preserves the current world")
	viewer.error_dialog.hide()
	check(viewer.open_realm(output.path_join("after/realm.json")), "Open edited source export")
	check(
		viewer.world.get_child_count() == 0, "Previous instances removed before loading next world"
	)
	await drain(viewer)
	check(viewer.loaded_bounds != original_bounds, "Edited geometry is visible")
	var after_bounds: Dictionary = viewer.loaded_bounds.duplicate(true)
	viewer.file_dialog.dir_selected.emit(output.path_join("after"))
	await drain(viewer)
	check(
		(
			viewer.realm_path == output.path_join("after/realm.json")
			and viewer.loaded_bounds == after_bounds
		),
		"Preview folder selection preserves the catalog and asset root"
	)
	viewer.reload_button.pressed.emit()
	await drain(viewer)
	check(
		viewer.loaded_bounds == after_bounds and viewer.world.get_child_count() == 1,
		"Reload does not duplicate entities"
	)
	check(viewer.open_realm(output.path_join("empty/realm.json")), "Complete empty world opens")
	viewer._process(0.1)
	check(
		viewer.loaded.is_empty() and viewer.queue.is_empty(), "Empty world contains no old assets"
	)
	check("empty" in viewer.status_label.text.to_lower(), "Empty world clearly identified")
	check(
		viewer.open_realm(output.path_join("missing/realm.json")),
		"Metadata can open before local asset is available"
	)
	check(await Streaming.drain(viewer), "Background imports complete before deadline")
	check(viewer.rejected.get("test_entity") == "Asset missing", "Missing model identified")
	check(viewer.open_realm(output.path_join("after/realm.json")), "Recover after a missing asset")
	await drain(viewer)
	check(viewer.rejected.is_empty(), "New selection clears old rejection state")
	await process_frame
	var report := {
		"checks": checks, "errors": errors, "status": "passed" if errors.is_empty() else "failed"
	}
	var file := FileAccess.open(output.path_join("viewer-workflow.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(report, "  ") + "\n")
	print("LOCAL_WORKFLOW_PASS " + str(checks) if errors.is_empty() else "LOCAL_WORKFLOW_FAIL")
	viewer.queue_free()
	quit(0 if errors.is_empty() else 1)
