## Validate actual generated GLBs and compare native Godot geometry with recorded bounds.
extends SceneTree
const Streaming = preload("streaming.gd")
const Core = preload("res://core.gd")
var errors: Array[String] = []
var checks := 0


func check(ok: bool, reason: String) -> void:
	checks += 1
	if not ok:
		errors.append(reason)
		push_error(reason)


func _initialize() -> void:
	run_checks.call_deferred()


func run_checks() -> void:
	var paths: Array[String] = []
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--realm="):
			paths.append(arg.trim_prefix("--realm="))
	check(not paths.is_empty(), "At least one actual bundle supplied")
	var viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	viewer.set_process(false)
	for path: String in paths:
		if not viewer.open_realm(path):
			check(false, "Manifest rejected: " + path)
			continue
		for entity: Dictionary in viewer.manifest.entities:
			viewer.set_pose(viewer.layout.entities[entity.id].focus)
			check(await Streaming.drain(viewer), "Background import completes")
			check(viewer.loaded.has(entity.id), "GLB validated and imported: " + entity.id)
			if not viewer.loaded_bounds.has(entity.id):
				continue
			var actual: Dictionary = viewer.loaded_bounds[entity.id]
			for axis: int in 3:
				check(
					absf(actual.min[axis] - entity.bounds_mm.min[axis] * .001) < .003,
					"World minimum: " + entity.id
				)
				check(
					absf(actual.max[axis] - entity.bounds_mm.max[axis] * .001) < .003,
					"World maximum: " + entity.id
				)
		check(viewer.rejected.is_empty(), "No rejected assets")
		if not viewer.manifest.entities.is_empty():
			var asset: Dictionary = viewer.manifest.entities[0].asset
			var bytes := FileAccess.get_file_as_bytes(path.get_base_dir().path_join(asset.path))
			check(not Core.invoke("glb.precheck", bytes.slice(0, 10)).ok, "Truncated GLB rejected")
			bytes[0] ^= 1
			check(not Core.invoke("glb.precheck", bytes).ok, "Bad magic rejected")
	viewer.queue_free()
	await process_frame
	print("S1_CHECKS_PASS ", checks if errors.is_empty() else errors)
	quit(0 if errors.is_empty() else 1)
