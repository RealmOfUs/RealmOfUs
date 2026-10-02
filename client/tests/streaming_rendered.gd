## Displayed metadata/mesh replacement and missing/corrupt asset acceptance.
extends SceneTree
var viewer: Node3D
var output := ""
var damaged := ""
var checks := 0
var errors: Array[String] = []
var captures: Array[Dictionary] = []


func check(value: bool, reason: String) -> void:
	checks += 1
	if not value:
		errors.append(reason)
		push_error(reason)


func _initialize() -> void:
	run.call_deferred()


func settle() -> void:
	var deadline := Time.get_ticks_msec() + 30000
	var continuous := true
	while viewer.streaming_busy() and Time.get_ticks_msec() < deadline:
		for id: String in viewer.catalog:
			if not viewer.loaded.has(id) and not viewer.impostors.instances[id].visible:
				continuous = false
		await process_frame
	check(not viewer.streaming_busy(), "Worker and mount queues finish")
	check(continuous, "Every entity keeps a box until its scene mounts")
	await create_timer(.5).timeout


func capture(name: String) -> void:
	await RenderingServer.frame_post_draw
	var focused := root.has_focus()
	check(focused, "Foreground window at capture: " + name)
	captures.append(
		{
			"name": name,
			"focused": focused,
			"loaded": viewer.loaded.size(),
			"rejected": viewer.rejected.size(),
			"radii": viewer.radii.duplicate()
		}
	)
	check(
		root.get_texture().get_image().save_png(output.path_join(name + ".png")) == OK,
		"Capture " + name
	)


func run() -> void:
	for argument: String in OS.get_cmdline_user_args():
		if argument.begins_with("--capture-dir="):
			output = argument.trim_prefix("--capture-dir=")
		if argument.begins_with("--damaged="):
			damaged = argument.trim_prefix("--damaged=")
	if output.is_empty() or damaged.is_empty():
		quit(2)
		return
	DirAccess.make_dir_recursive_absolute(output)
	root.size = Vector2i(1440, 810)
	viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	check(DisplayServer.get_name() != "headless", "Real display renderer is active")
	check(viewer.impostors.instances.size() == 800, "All 800 catalog entities have boxes")
	await settle()
	check(viewer.loaded.is_empty(), "High overview needs no GLB imports")
	await capture("overview")
	viewer.navigate_to(Vector3(-126, 30, 126), Vector3.ZERO)
	await settle()
	check(
		viewer.loaded.size() > 0 and viewer.loaded.size() < 800,
		"Corner shows full models and far boxes"
	)
	for id: String in viewer.loaded:
		check(not viewer.impostors.instances[id].visible, "Mounted model hides box: " + id)
	await capture("corner")
	viewer.navigate_to(Vector3(0, 20, 0), Vector3(80, 0, -80))
	await settle()
	await capture("middle")
	viewer.navigate_to(Vector3(0, 900, 900), Vector3.ZERO)
	await settle()
	check(viewer.loaded.is_empty(), "Leaving the region releases all full scenes")
	await capture("released")
	check(viewer.open_realm(damaged), "Missing/corrupt bundle metadata still opens")
	viewer.set_radii([48, 200, 4000])
	viewer.navigate_to(Vector3(0, 180, 180), Vector3.ZERO)
	await settle()
	check(viewer.rejected.size() > 0, "Damaged assets have explicit rejection")
	check(viewer.loaded.size() > 0, "Other assets still load normally")
	var missing := 0
	var corrupt := 0
	for id: String in viewer.rejected:
		check(viewer.impostors.instances[id].visible, "Rejected asset stays visible: " + id)
		if viewer.rejection_codes[id] == viewer.Loader.MISSING_ASSET:
			missing += 1
		else:
			corrupt += 1
	check(missing > 0 and corrupt > 0, "Missing and one-byte corruption remain distinct")
	viewer.diagnostic.show()
	await capture("missing-and-rejected")
	viewer.world_ui.tabs.current_tab = 3
	await capture("settings")
	var report := {
		"checks": checks,
		"errors": errors,
		"missing": missing,
		"corrupt": corrupt,
		"captures": captures,
		"godot": Engine.get_version_info().string,
		"renderer": RenderingServer.get_current_rendering_method(),
		"status": "passed" if errors.is_empty() else "failed"
	}
	var file := FileAccess.open(output.path_join("report.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(report, "  ") + "\n")
	file.close()
	viewer.clear_realm()
	viewer.queue_free()
	await process_frame
	await process_frame
	print("STREAMING_RENDERED_PASS ", checks if errors.is_empty() else errors)
	quit(0 if errors.is_empty() else 1)
