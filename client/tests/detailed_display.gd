## Focused full/lod1 inspection of the three-model detailed sample, not a frame benchmark.
extends SceneTree
const Core = preload("res://core.gd")
const Streaming = preload("streaming.gd")
var viewer: Node3D
var output := ""
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


func focus_window() -> bool:
	DisplayServer.window_move_to_foreground()
	root.grab_focus()
	var deadline := Time.get_ticks_msec() + 15000
	var focused_since := 0
	while focused_since == 0 or Time.get_ticks_msec() - focused_since < 500:
		if Time.get_ticks_msec() >= deadline:
			check(false, "Window obtains continuous focus within 15 seconds")
			return false
		await process_frame
		if not root.has_focus():
			focused_since = 0
		elif focused_since == 0:
			focused_since = Time.get_ticks_msec()
	check(true, "Window has continuous focus for at least 500 ms")
	return true


func capture(id: String, tier: String) -> void:
	check(await Streaming.drain(viewer), "Streaming completes for " + id + " / " + tier)
	viewer.world_ui.update()
	viewer.status_label.text = "Detailed display inspection: " + id + " / " + tier
	check(viewer.tiers[tier].has(id), "Core selects the requested distance tier")
	var asset: Dictionary = viewer.catalog[id].asset if tier == "near" else viewer.catalog[id].lod1
	check(viewer.loaded_assets.get(id) == asset.sha256, "Mounted SHA matches " + id + " / " + tier)
	check(viewer.asset_observations.has(asset.sha256), "Mounted asset has inspected metrics")
	if not viewer.asset_observations.has(asset.sha256):
		return
	var metrics: Dictionary = viewer.asset_observations[asset.sha256].metrics
	check(
		(
			metrics.max_image_dimension <= Core.config().lod1.image_dimension
			if tier == "mid"
			else metrics.max_image_dimension >= 4096
		),
		"Texture dimensions match the detailed sample tier"
	)
	check(not viewer.impostors.instances[id].visible, "Mounted detail hides its metadata box")
	viewer.diagnostic.text = (
		"%s / %s\nMounted SHA: %s\nMaximum image edge: %d px\nCamera FOV: %.2f degrees"
		% [id, tier, asset.sha256, metrics.max_image_dimension, viewer.camera.fov]
	)
	if not await focus_window():
		return
	await RenderingServer.frame_post_draw
	var path := output.path_join(id + "-" + tier + ".png")
	check(root.has_focus(), "Window remains focused at capture")
	check(root.get_texture().get_image().save_png(path) == OK, "Save focused screenshot")
	captures.append(
		{
			"id": id,
			"tier": tier,
			"sha256": asset.sha256,
			"max_image_dimension": metrics.max_image_dimension,
			"path": path,
			"focused": root.has_focus(),
			"continuous_focus_ms": 500,
			"camera_position": Core.array(viewer.camera.position),
			"camera_fov": viewer.camera.fov,
			"radii": viewer.radii.duplicate()
		}
	)


func run() -> void:
	for argument: String in OS.get_cmdline_user_args():
		if argument.begins_with("--capture-dir="):
			output = argument.trim_prefix("--capture-dir=")
	if output.is_empty() or DisplayServer.get_name() == "headless":
		push_error("Detailed display requires --capture-dir and a real display renderer")
		quit(2)
		return
	DirAccess.make_dir_recursive_absolute(output)
	root.size = Vector2i(1440, 810)
	root.title = "Realm of Us - Detailed display acceptance"
	print("DETAILED_DISPLAY_WAITING_FOR_FOCUS")
	if not await focus_window():
		quit(1)
		return
	viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	viewer.options["balanced"] = "true"
	root.add_child(viewer)
	viewer.set_process(false)
	viewer.file_dialog.hide()
	check(viewer.catalog.size() == 3, "Detailed sample contains three models")
	if viewer.catalog.size() != 3:
		viewer.queue_free()
		await process_frame
		quit(1)
		return
	viewer.set_radii(Core.config().viewer.radii)
	viewer.diagnostic.show()
	for id: String in viewer.catalog:
		check(
			viewer.catalog[id].asset.sha256 != viewer.catalog[id].lod1.sha256,
			"Distinct tier SHAs: " + id
		)
		viewer.world_ui.selection = id
		var pose: Dictionary = viewer.layout.entities[id].focus
		var target := Core.vec(pose.look_at)
		var offset := Core.vec(pose.position) - target
		viewer.camera.fov = 75
		viewer.set_pose(pose)
		await capture(id, "near")
		# Keep the object comparable in screen size while querying a real middle distance.
		# This is an inspection zoom, recorded in the receipt, not normal flight quality.
		viewer.camera.fov = rad_to_deg(2 * atan(tan(deg_to_rad(75.0) / 2) * offset.length() / 128))
		viewer.navigate_to(target + offset.normalized() * 128, target)
		await capture(id, "mid")
	check(captures.size() == 6, "Every model has near and middle visual evidence")
	check(viewer.rejected.is_empty() and viewer.asset_failures.is_empty(), "No asset was rejected")
	viewer.navigate_to(Vector3(0, 6000, 6000), Vector3.ZERO)
	check(await Streaming.drain(viewer), "Release finishes outstanding worker tasks")
	# Advance host cache age; expiration is wall-clock policy, not camera simulation time.
	for prototype: Dictionary in viewer.prototypes.values():
		prototype.used_at -= viewer.PROTOTYPE_IDLE_SECONDS + 1
	viewer.prune_prototypes()
	await process_frame
	check(
		viewer.loaded.is_empty() and viewer.world.get_child_count() == 0,
		"Released resident scenes reach zero"
	)
	check(viewer.prototypes.is_empty(), "Released cached prototypes reach zero")
	var report := {
		"status": "passed" if errors.is_empty() else "failed",
		"checks": checks,
		"errors": errors,
		"realm": viewer.realm_path,
		"captures": captures,
		"resident_scenes": viewer.loaded.size(),
		"cached_prototypes": viewer.prototypes.size(),
		"rejected": viewer.rejected.duplicate(),
		"asset_failures": viewer.asset_failures.duplicate(),
		"godot": Engine.get_version_info().string,
		"renderer": RenderingServer.get_current_rendering_method(),
		"viewport": [root.get_texture().get_width(), root.get_texture().get_height()],
		"inspection_zoom":
		"Middle captures use a narrower FOV at 128 m from the entity center; not a performance sample"
	}
	var file := FileAccess.open(output.path_join("report.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(report, "  ") + "\n")
	file.close()
	viewer.clear_realm()
	viewer.queue_free()
	await process_frame
	await process_frame
	print("DETAILED_DISPLAY_PASS ", checks if errors.is_empty() else errors)
	quit(0 if errors.is_empty() else 1)
