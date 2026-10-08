## Headless probe: main-thread cost of one refresh_assets() on a browse bundle, split
## into the native catalog_query call and the GDScript bookkeeping around it.
## Usage: godot --headless --path client/app --script ../tests/refresh_cost.gd -- \
##   --realm=<bundle>/realm.json [--rounds=30]
## Prints one REFRESH_COST JSON line; it is a CPU probe, not a rendering benchmark.
extends SceneTree
const Core = preload("res://core.gd")

var realm := ""
var rounds := 40


func _initialize() -> void:
	for argument: String in OS.get_cmdline_user_args():
		if argument.begins_with("--realm="):
			realm = argument.trim_prefix("--realm=")
		elif argument.begins_with("--rounds="):
			rounds = int(argument.trim_prefix("--rounds="))
	run_probe.call_deferred()


func p95(samples: Array) -> float:
	var sorted := samples.duplicate()
	sorted.sort()
	return sorted[int(floor(0.95 * (sorted.size() - 1)))]


func run_probe() -> void:
	var viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	viewer.set_process(false)
	if not viewer.open_realm(realm):
		push_error("open_realm failed")
		quit(1)
		return
	viewer.set_radii(Core.config().viewer.radii)
	# Measure at the world center so every entity is inside the middle radius
	# (load list full), and at a far corner (no candidates, even though pages remain cached).
	var results := {}
	for label: String in ["center", "far"]:
		viewer.camera.position = Vector3(0, 3, 0) if label == "center" else Vector3(3000, 3, 3000)
		var query_ms: Array = []
		var refresh_ms: Array = []
		var payload := 0
		for i in rounds:
			var request := (
				JSON
				. stringify(
					{
						"position": Core.array(viewer.camera.position),
						"radii": viewer.radii,
						"loaded": viewer.loaded_assets
					}
				)
				. to_utf8_buffer()
			)
			var t0 := Time.get_ticks_usec()
			var response: Dictionary = viewer.native_catalog.catalog_query(request)
			query_ms.append((Time.get_ticks_usec() - t0) / 1000.0)
			if i == 0:
				payload = JSON.stringify(response.value).length()
			t0 = Time.get_ticks_usec()
			viewer.refresh_assets()
			refresh_ms.append((Time.get_ticks_usec() - t0) / 1000.0)
		results[label] = {
			"catalog_query_p95_ms": p95(query_ms),
			"refresh_assets_p95_ms": p95(refresh_ms),
			"response_chars": payload,
			"near_ids": viewer.near_ids.size(),
			"queue": viewer.queue.size()
		}
	print(
		"REFRESH_COST ",
		JSON.stringify({"entities": viewer.catalog.size(), "rounds": rounds, "results": results})
	)
	viewer.queue_free()
	await process_frame
	quit(0)
