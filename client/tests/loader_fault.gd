## Isolated negative probe: a real script error must not strand a worker slot.
extends SceneTree
const Loader = preload("res://loader.gd")


func _initialize() -> void:
	run.call_deferred()


func fail_worker() -> void:
	var empty: Dictionary = {}
	var _value: Variant = empty.injected_worker_failure


func run() -> void:
	var loader := Loader.new()
	loader._start("fault", fail_worker)
	var deadline := Time.get_ticks_msec() + 5000
	var results: Array[Dictionary] = []
	while results.is_empty() and Time.get_ticks_msec() < deadline:
		results = loader.take_completed()
		await process_frame
	if results.size() == 1 and results[0].code == "worker_failed" and loader.drained():
		print("LOADER_FAULT_RECOVERED")
		quit(0)
	else:
		push_error("Injected worker failure did not release its slot")
		quit(1)
