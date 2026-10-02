## Dense geometry exercises actual runtime LOD generation and in-flight cancellation.
extends SceneTree
const Core = preload("res://core.gd")
const Streaming = preload("streaming.gd")
const Loader = preload("res://loader.gd")
var checks := 0
var errors: Array[String] = []


func check(value: bool, reason: String) -> void:
	checks += 1
	if not value:
		errors.append(reason)
		push_error(reason)


func _initialize() -> void:
	run.call_deferred()


func run() -> void:
	await check_task_recovery()
	var folder := "user://streaming-contract-" + str(Time.get_ticks_usec())
	DirAccess.make_dir_recursive_absolute(folder.path_join("objects"))
	var mesh := SphereMesh.new()
	mesh.radius = 1
	mesh.height = 2
	mesh.radial_segments = 64
	mesh.rings = 32
	var arrays := mesh.get_mesh_arrays()
	var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	var positions := vertices.to_byte_array()
	var binary := positions + indices.to_byte_array()
	var geometry := {
		"asset": {"version": "2.0"},
		"buffers": [{"byteLength": binary.size()}],
		"bufferViews":
		[
			{"buffer": 0, "byteLength": positions.size()},
			{"buffer": 0, "byteOffset": positions.size(), "byteLength": indices.size() * 4}
		],
		"accessors":
		[
			{
				"bufferView": 0,
				"componentType": 5126,
				"count": vertices.size(),
				"type": "VEC3",
				"min": [-1, -1, -1],
				"max": [1, 1, 1]
			},
			{"bufferView": 1, "componentType": 5125, "count": indices.size(), "type": "SCALAR"}
		],
		"meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
		"nodes": [{"mesh": 0}],
		"scenes": [{"nodes": [0]}],
		"scene": 0
	}
	var text := JSON.stringify(geometry).to_utf8_buffer()
	while text.size() % 4 != 0:
		text.append(32)
	var bytes := (
		PackedInt32Array([0x46546c67, 2, 28 + text.size() + binary.size(), text.size(), 0x4e4f534a])
		. to_byte_array()
	)
	bytes.append_array(text)
	bytes.append_array(PackedInt32Array([binary.size(), 0x004e4942]).to_byte_array())
	bytes.append_array(binary)
	var inspected := Core.invoke("glb.inspect", bytes)
	check(inspected.ok, "Dense GLB satisfies the Rust budget: " + Core.message(inspected))
	if not inspected.ok:
		quit(1)
		return
	var asset: Dictionary = inspected.value.descriptor.duplicate()
	asset.path = Core.invoke("asset.path", asset.sha256).value
	var file := FileAccess.open(folder.path_join(asset.path), FileAccess.WRITE)
	file.store_buffer(bytes)
	file.close()
	var bundle := Core.invoke(
		"preview.create",
		{
			"profile": Core.config().profile,
			"entities":
			[
				{
					"id": "sphere",
					"position_mm": [0, 0, 0],
					"bounds_mm": {"min": [-1000, -1000, -1000], "max": [1000, 1000, 1000]},
					"asset": asset
				}
			]
		}
	)
	check(bundle.ok, "Dense fixture catalog is valid")
	file = FileAccess.open(folder.path_join("realm.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(bundle.value))
	file.close()
	var viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	viewer.set_process(false)
	viewer.file_dialog.hide()
	check(viewer.radii == [16.0, 64.0, 4000.0], "Actual --radius argument reaches the viewer")
	check(viewer.open_realm(folder), "Dense fixture opens")
	viewer.pump_streaming()
	check(viewer.loader.import_count() == 1, "Exactly one background task was submitted")
	viewer.navigate_position(Vector3(500, 500, 500))
	check(await Streaming.drain(viewer), "Cancelled worker completes without blocking main thread")
	check(
		viewer.loaded.is_empty() and viewer.prototypes.has(asset.sha256),
		"A cancelled request keeps a bounded reusable prepared asset"
	)
	check(not viewer.prototypes[asset.sha256].has("scene"), "Cancelled import creates no scene")
	check(viewer.impostors.instances.sphere.visible, "Cancellation preserves far box")
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(await Streaming.drain(viewer), "Dense model finishes background import")
	check(viewer.generated_lods > 0, "Dense importer mesh has generated LODs")
	check(
		viewer.background_imports == 1, "Returning to a cancelled import reuses the worker result"
	)
	check(viewer.scene_times.size() == 1, "Scene generation runs only on first main-thread mount")
	check(
		viewer.loader.parse_in_worker == (DisplayServer.get_name() != "headless"),
		"The dummy backend keeps all rendering resource creation on the main thread"
	)
	check(not viewer.impostors.instances.sphere.visible, "LOD scene replaces its box atomically")
	check(
		viewer.mesh_instances.sphere[0].lod_bias == 1.0, "Near detail keeps normal screen threshold"
	)
	viewer.navigate_position(Vector3(0, 80, 0))
	check(viewer.loaded.has("sphere"), "Middle tier retains the same GLB")
	check(
		viewer.mesh_instances.sphere[0].lod_bias == .25, "Middle tier switches mesh detail earlier"
	)
	check(
		(
			viewer.mesh_instances.sphere[0].cast_shadow
			== GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		),
		"Middle tier omits distant shadows"
	)
	check(
		viewer.mesh_instances.sphere[0].gi_mode == GeometryInstance3D.GI_MODE_DISABLED,
		"Middle tier omits GI voxelization"
	)
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(
		viewer.mesh_instances.sphere[0].cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_ON,
		"Returning to near detail restores shadows"
	)
	check(
		viewer.mesh_instances.sphere[0].gi_mode == GeometryInstance3D.GI_MODE_STATIC,
		"Returning to near detail restores GI"
	)
	viewer.navigate_position(Vector3(0, 100, 0))
	check(viewer.loaded.has("sphere"), "Hysteresis retains a model beyond the middle radius")
	viewer.navigate_position(Vector3(0, 106, 0))
	check(viewer.loaded.is_empty(), "Leaving hysteresis unloads the scene")
	check(viewer.impostors.instances.sphere.visible, "Unloading restores the box")
	viewer.navigate_position(Vector3(0, 5000, 0))
	check(not viewer.impostors.instances.sphere.visible, "Far radius clips metadata boxes")
	# Cache age is host policy; advance the timestamp without a slow wall-clock sleep.
	for prototype: Dictionary in viewer.prototypes.values():
		prototype.used_at -= 10
	viewer.prune_prototypes()
	check(viewer.prototypes.is_empty(), "Unused prototypes expire and release resource references")
	check(viewer.open_realm(folder), "Reopen before testing a cancelled failed task")
	viewer.loader._start(asset.sha256, func(): pass)
	viewer.navigate_position(Vector3(500, 500, 500))
	check(await Streaming.drain(viewer), "A worker ending without publication does not hang")
	check(viewer.rejection_codes.sphere == "worker_failed", "Failure survives camera cancellation")
	check(viewer.impostors.instances.sphere.visible, "Failed task keeps a marked box")
	viewer.reject_asset(asset.sha256, Loader.failure(Loader.MISSING_ASSET, "Different wording"))
	var chunk: String = Core.invoke("spatial.key", viewer.catalog.sphere.owner_chunk).value
	check(viewer.chunk_state(chunk) == "Asset missing", "Missing status uses its code, not wording")
	check(
		viewer.impostors.instances.sphere.error_code == Loader.MISSING_ASSET,
		"Missing box retains a code independent of its error wording"
	)
	check(viewer.open_realm(folder), "Reopen before the live timeout check")
	var gate := Semaphore.new()
	var stalled: RefCounted = viewer.loader
	stalled._start(asset.sha256, func(): gate.wait())
	stalled.tasks[asset.sha256].started -= Loader.TASK_TIMEOUT_MS
	check(await Streaming.drain(viewer), "An overdue live import does not keep the viewer busy")
	check(viewer.rejection_codes.sphere == "worker_timeout", "The viewer exposes the task timeout")
	check(viewer.open_realm(folder), "Switch worlds while the timed-out worker is still running")
	check(
		viewer.retired_loaders.has(stalled), "Keep the old loader alive until its worker finishes"
	)
	gate.post()
	check(await Streaming.drain(viewer), "A new world imports normally after a task timeout")
	viewer.pump_streaming()
	check(
		stalled.drained() and not viewer.retired_loaders.has(stalled),
		"Late retired work is released"
	)
	viewer.clear_realm()
	viewer.queue_free()
	await process_frame
	if "--keep-fixture" in OS.get_cmdline_user_args():
		print("STREAMING_FIXTURE ", ProjectSettings.globalize_path(folder))
	else:
		DirAccess.remove_absolute(folder.path_join(asset.path))
		DirAccess.remove_absolute(folder.path_join("objects"))
		DirAccess.remove_absolute(folder.path_join("realm.json"))
		DirAccess.remove_absolute(folder)
	print("STREAMING_CONTRACT_PASS ", checks if errors.is_empty() else errors)
	quit(0 if errors.is_empty() else 1)


func check_task_recovery() -> void:
	var worker := Loader.new()
	worker._start("orphan", func(): pass)
	var deadline := Time.get_ticks_msec() + 5000
	while not WorkerThreadPool.is_task_completed(worker.tasks.orphan.id):
		if Time.get_ticks_msec() >= deadline:
			check(false, "Fault simulation task completes")
			return
		await process_frame
	var results := worker.take_completed()
	check(
		results.size() == 1 and results[0].code == "worker_failed",
		"Missing publication becomes failure"
	)
	check(worker.drained() and worker.import_count() == 0, "Failed task releases its import slot")
	var gate := Semaphore.new()
	worker._start(
		"late",
		func():
			gate.wait()
			worker._publish("late", {"bytes": PackedByteArray([1])})
	)
	worker.tasks.late.started -= Loader.TASK_TIMEOUT_MS
	results = worker.take_completed()
	check(
		results.size() == 1 and results[0].code == "worker_timeout",
		"Overdue task reports a timeout"
	)
	check(
		worker.tasks.is_empty() and not worker.drained(),
		"Late worker is quarantined without waiting"
	)
	worker.submit("", {"sha256": "new"})
	check(worker.tasks.is_empty(), "A stalled loader does not start more tasks")
	gate.post()
	deadline = Time.get_ticks_msec() + 5000
	while not worker.drained() and Time.get_ticks_msec() < deadline:
		worker.discard_completed()
		await process_frame
	check(worker.drained() and worker.completed.is_empty(), "Retired late results are reaped")
	worker.shutdown()
