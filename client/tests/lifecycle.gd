## Native validation, camera choices, streaming residency and optional local sample routes.
extends SceneTree
const Streaming = preload("streaming.gd")
const Core = preload("res://core.gd")
var errors: Array[String] = []
var checks := 0


func check(value: bool, reason: String) -> void:
	checks += 1
	if not value:
		errors.append(reason)
		push_error(reason)


func _initialize() -> void:
	run_test.call_deferred()


func run_test() -> void:
	root.size = Vector2i(1920, 1080)
	var viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	viewer.set_process(false)
	await process_frame
	check(not viewer.catalog.is_empty(), "Supplied world contains entities")
	if viewer.catalog.is_empty():
		quit(1)
		return
	var identity: String = viewer.catalog.keys()[0]
	viewer.world_ui.select_entity(identity)
	viewer.world_ui.visit_entity()
	check(not viewer.queue.is_empty(), "Entity list provides a valid destination")
	viewer.navigate_position(Vector3(10000, 10000, 10000))
	check(viewer.queue.is_empty() and viewer.cancellations > 0, "Obsolete queued imports cancelled")
	viewer.world_ui.visit_entity()
	check(await Streaming.drain(viewer), "Background imports complete before deadline")
	check(not viewer.loaded.is_empty() and viewer.rejected.is_empty(), "Real models imported")
	viewer.navigate_position(Vector3(10000, 10000, 10000))
	check(viewer.loaded.is_empty() and viewer.unloads > 0, "Residency released")
	await process_frame
	check(viewer.world.get_child_count() == 0, "Removed instances are freed")
	var broken: Dictionary = viewer.manifest.duplicate(true)
	broken.index_complete = false
	check(
		not Core.invoke("manifest.validate", broken).ok,
		"Incomplete metadata rejected by native core"
	)
	if viewer.presentation != null:
		var sample: Node3D = viewer.presentation
		check(
			sample.active and sample.data.waypoints.size() == 16,
			"House sample reads its own local itinerary"
		)
		sample.visit(4)
		check(
			viewer.camera.position.distance_to(Vector3(-5.1, 5.85, -5.4)) < .0001,
			"Kitchen route uses metre position"
		)
		var key := InputEventKey.new()
		key.keycode = KEY_BRACKETRIGHT
		key.pressed = true
		sample._unhandled_input(key)
		check(sample.station == 5, "Sample owns bracket navigation")
		for room: Dictionary in sample.data.listing.rooms:
			if room.id == "suite_l3_2":
				sample.focus_room(room)
				check(
					viewer.camera.position.y > 9.5 and viewer.camera.position.z < 0,
					"Room plan enters the actual upper suite"
				)
			if room.id == "adu_bed_1":
				sample.focus_room(room)
				check(
					viewer.camera.position.z < -24 and viewer.camera.position.x < 8,
					"ADU room target clears its bath wall"
				)
		sample.toggle_tour()
		check(sample.autoplay, "Local tour starts explicitly")
	viewer.clear_realm()
	check(
		viewer.catalog.is_empty() and viewer.prototypes.is_empty() and viewer.queue.is_empty(),
		"Clear releases metadata and prototypes"
	)
	if viewer.presentation != null:
		check(
			not viewer.presentation.autoplay and not viewer.presentation.active,
			"Clear also stops the optional sample"
		)
	viewer.queue_free()
	await process_frame
	print("LIFECYCLE_PASS ", checks if errors.is_empty() else errors)
	quit(0 if errors.is_empty() else 1)
