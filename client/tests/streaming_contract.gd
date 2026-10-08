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
	var bytes := mesh_bytes(mesh)
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
					"asset": asset,
					"lod1": asset
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
	await check_distinct_lod1(viewer, folder, bundle.value, bytes)
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


func mesh_bytes(mesh: PrimitiveMesh) -> PackedByteArray:
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
	return bytes


func write_bytes(path: String, bytes: PackedByteArray) -> void:
	var file := FileAccess.open(path, FileAccess.WRITE)
	file.store_buffer(bytes)
	file.close()


func check_distinct_lod1(
	viewer: Node3D, folder: String, bundle: Dictionary, full_bytes: PackedByteArray
) -> void:
	var simplified := mesh_bytes(BoxMesh.new())
	var inspected := Core.invoke("glb.inspect", simplified)
	check(inspected.ok, "Distinct middle-detail fixture passes safety precheck")
	var lod1: Dictionary = inspected.value.descriptor.duplicate()
	lod1.path = Core.invoke("asset.path", lod1.sha256).value
	var full: Dictionary = bundle.entities[0].asset
	check(lod1.sha256 != full.sha256, "Detail tiers have distinct hashes")
	bundle.entities[0].lod1 = lod1
	write_bytes(folder.path_join(lod1.path), simplified)
	write_bytes(folder.path_join("realm.json"), JSON.stringify(bundle).to_utf8_buffer())
	check(viewer.open_realm(folder), "Dual-asset bundle opens")
	check(await Streaming.drain(viewer), "Near asset loads")
	check(viewer.loaded_assets.sphere == full.sha256, "Near tier uses the full hash")
	var original: Node3D = viewer.loaded.sphere
	var before := [viewer.imports, viewer.asset_imports, viewer.unloads]
	for height: float in [48.1, 50.3, 48.2, 50.2, 48.1]:
		viewer.navigate_position(Vector3(0, height, 0))
		check(viewer.queue.is_empty(), "Near hysteresis avoids replacement work in the band")
		check(await Streaming.drain(viewer), "Hysteresis refresh stays idle")
	check(viewer.loaded.sphere == original, "Oscillating in the band preserves the mounted scene")
	check(
		before == [viewer.imports, viewer.asset_imports, viewer.unloads],
		"Oscillating in the band causes no imports or unloads"
	)
	viewer.navigate_position(Vector3(0, 80, 0))
	check(viewer.loaded.has("sphere"), "Keep the previous scene during a valid tier replacement")
	viewer.refresh_assets()
	check(not viewer.impostors.instances.sphere.visible, "Core hides the box during replacement")
	check(await Streaming.drain(viewer), "Middle asset loads independently")
	check(viewer.loaded_assets.sphere == lod1.sha256, "Middle tier uses the lod1 hash")
	check(
		viewer.prototypes.size() == 2 and viewer.asset_imports == 2,
		"Each SHA has its own cache entry"
	)
	check(not viewer.impostors.instances.sphere.visible, "Mounted lod1 hides its box")
	viewer.navigate_position(Vector3(0, 49, 0))
	check(viewer.queue.is_empty(), "Mounted middle tier stays middle in the same hysteresis band")
	check(viewer.loaded_assets.sphere == lod1.sha256, "Hysteresis depends on the mounted SHA")
	viewer.prototypes[full.sha256].used_at = Time.get_ticks_msec() / 1000.0
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(await Streaming.drain(viewer), "Near asset is restored")
	check(
		viewer.loaded_assets.sphere == full.sha256 and viewer.asset_imports == 2,
		"Returning near reuses the full prototype"
	)
	viewer.clear_realm()
	DirAccess.remove_absolute(folder.path_join(lod1.path))
	check(viewer.open_realm(folder), "Metadata opens without lod1 bytes")
	viewer.navigate_position(Vector3(0, 80, 0))
	check(await Streaming.drain(viewer), "Missing middle asset finishes with a visible failure")
	check(
		viewer.loaded.is_empty() and viewer.asset_imports == 0,
		"Missing lod1 never imports the available full GLB"
	)
	check(
		(
			viewer.rejection_codes.sphere == Loader.MISSING_ASSET
			and viewer.impostors.instances.sphere.visible
		),
		"Missing lod1 displays the marked box"
	)
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(await Streaming.drain(viewer), "Full asset remains usable when lod1 is missing")
	check(
		viewer.loaded_assets.sphere == full.sha256 and viewer.rejected.is_empty(),
		"Failures are scoped to the selected SHA"
	)
	viewer.set_radii([16, 4000, 4000])
	viewer.navigate_position(Vector3(0, 4300, 0))
	check(viewer.loaded.is_empty(), "A missing target detaches a scene in the retention band")
	check(
		not viewer.impostors.instances.sphere.visible,
		"Failure-driven detachment beyond the far radius does not reveal a box"
	)
	viewer.set_radii([16, 64, 4000])
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(await Streaming.drain(viewer), "Return near after the far-radius failure")
	viewer.navigate_position(Vector3(0, 80, 0))
	check(
		viewer.loaded.is_empty() and viewer.impostors.instances.sphere.visible,
		"Entering a missing middle tier releases the full scene"
	)
	viewer.clear_realm()
	write_bytes(folder.path_join(lod1.path), simplified)
	DirAccess.remove_absolute(folder.path_join(full.path))
	check(viewer.open_realm(folder), "Metadata opens without full bytes")
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(await Streaming.drain(viewer), "Missing full asset finishes with a visible failure")
	viewer.navigate_position(Vector3(0, 80, 0))
	check(await Streaming.drain(viewer), "Middle tier can load with the full asset missing")
	check(
		(
			viewer.loaded_assets.sphere == lod1.sha256
			and viewer.rejected.is_empty()
			and viewer.asset_failures.has(full.sha256)
		),
		"Missing full asset stays recorded without poisoning lod1"
	)
	viewer.clear_realm()
	write_bytes(folder.path_join(full.path), full_bytes)
	await check_work_order(viewer, folder, bundle)
	DirAccess.remove_absolute(folder.path_join(lod1.path))
	bundle.entities[0].lod1 = full
	write_bytes(folder.path_join("realm.json"), JSON.stringify(bundle).to_utf8_buffer())


func check_work_order(viewer: Node3D, folder: String, bundle: Dictionary) -> void:
	var source: Dictionary = bundle.entities[0]
	var entities: Array = []
	for item: Dictionary in [{"id": "sphere", "x": 0}, {"id": "companion", "x": 64000}]:
		entities.append(
			{
				"id": item.id,
				"position_mm": [item.x, 0, 0],
				"bounds_mm":
				{"min": [item.x - 1000, -1000, -1000], "max": [item.x + 1000, 1000, 1000]},
				"asset": source.asset,
				"lod1": source.lod1
			}
		)
	var result := Core.invoke("preview.create", {"profile": bundle.profile, "entities": entities})
	check(result.ok, "Two-entity queue fixture is valid")
	write_bytes(folder.path_join("realm.json"), JSON.stringify(result.value).to_utf8_buffer())
	check(viewer.open_realm(folder), "Queue fixture opens")
	viewer.navigate_position(Vector3(0, 80, 0))
	check(await Streaming.drain(viewer), "Queue fixture mounts middle assets")
	viewer.unmount_asset("companion")
	viewer.set_pose(viewer.layout.entities.sphere.focus)
	check(viewer.queue == ["sphere", "companion"], "Nearby upgrade precedes a farther new load")
	check(not viewer.impostors.instances.sphere.visible, "Queued upgrade preserves its scene")
	check(viewer.impostors.instances.companion.visible, "New load retains its box")
	check(await Streaming.drain(viewer), "Ordered replacement and new load complete")
	check(viewer.loaded_assets.sphere == source.asset.sha256, "Upgrade mounts the core target SHA")
	# A late worker failure must also reconcile visibility outside the far radius.
	viewer.set_radii([16, 4000, 4000])
	viewer.navigate_position(Vector3(0, 4300, 0))
	viewer.reject_asset(
		source.lod1.sha256, Loader.failure(Loader.MISSING_ASSET, "Injected failure")
	)
	check(viewer.loaded.is_empty(), "Late target failure releases retained scenes")
	check(
		(
			not viewer.impostors.instances.sphere.visible
			and not viewer.impostors.instances.companion.visible
		),
		"Late failure never reveals boxes beyond the far radius"
	)
	viewer.clear_realm()
