## Deterministic displayed benchmark: overview, flight across the default world, then release.
extends SceneTree
var viewer: Node3D
var last_phase := ""
var last_focused := true


func _initialize() -> void:
	start.call_deferred()


func start() -> void:
	root.size = Vector2i(1440, 810)
	root.title = "Realm of Us - Streaming benchmark"
	if DisplayServer.get_name() == "headless":
		push_error("The displayed benchmark requires a foreground window")
		quit(1)
		return
	DisplayServer.window_move_to_foreground()
	root.grab_focus()
	var focus_deadline := Time.get_ticks_msec() + 15000
	var focused_since := 0
	print("BENCHMARK_WAITING_FOR_FOCUS")
	# Open the world only after focus arrives, preserving all loading and flight frames.
	while focused_since == 0 or Time.get_ticks_msec() - focused_since < 500:
		if Time.get_ticks_msec() >= focus_deadline:
			push_error("The benchmark window did not obtain focus within 15 seconds")
			quit(1)
			return
		await process_frame
		if not root.has_focus():
			focused_since = 0
		elif focused_since == 0:
			focused_since = Time.get_ticks_msec()
	print("BENCHMARK_FOCUSED_START")
	viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)


func _process(_delta: float) -> bool:
	if viewer == null:
		return false
	if root.has_focus() != last_focused:
		last_focused = root.has_focus()
		print("BENCHMARK_FOCUS ", last_focused, " AT_SECONDS ", viewer.age)
	if viewer.first_ready < 0:
		return false
	var elapsed: float = viewer.age - viewer.first_ready
	var duration := float(viewer.options.get("benchmark", "30"))
	var phase := "overview"
	if elapsed >= duration * .67:
		phase = "released"
		if phase != last_phase:
			viewer.navigate_to(Vector3(0, 900, 900), Vector3.ZERO)
	elif elapsed >= 2:
		phase = "flight"
		var progress := clampf((elapsed - 2) / (duration * .67 - 2), 0, 1)
		viewer.camera.position = Vector3(-126, 28, 126).lerp(Vector3(126, 28, -126), progress)
		viewer.camera.look_at(Vector3.ZERO)
	if phase != last_phase:
		last_phase = phase
		print("BENCHMARK_PHASE ", phase)
	return false
