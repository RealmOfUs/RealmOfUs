## Workers read and validate bytes; display backends also parse glTF and generate LODs.
## The main thread creates scenes. Godot 4.7's dummy backend parses there too.
extends RefCounted
const Core = preload("core.gd")
const MAX_IMPORT_TASKS := 2
const TASK_TIMEOUT_MS := 30000
const MISSING_ASSET := "missing_asset"
var tasks: Dictionary = {}
var retired_tasks: Dictionary = {}
var terminal_error: Dictionary = {}
var mutex := Mutex.new()
var completed: Dictionary = {}
var pending_scan: Dictionary = {}
var scan_root := ""
var parse_in_worker := DisplayServer.get_name() != "headless"
var remote_source: RefCounted


static func failure(code: String, message: String) -> Dictionary:
	return {"code": code, "error": message}


func _start(key: String, work: Callable) -> void:
	tasks[key] = {"id": WorkerThreadPool.add_task(work), "started": Time.get_ticks_msec()}


func scan(root: String, assets: Dictionary) -> void:
	scan_root = root
	pending_scan.merge(assets, true)
	_start_scan()


func _start_scan() -> void:
	if not pending_scan.is_empty() and not tasks.has("availability"):
		var batch := pending_scan.duplicate(true)
		pending_scan.clear()
		_start("availability", _scan.bind(scan_root, batch))


func submit(root: String, asset: Dictionary) -> void:
	var sha: String = asset.sha256
	if tasks.has(sha) or not terminal_error.is_empty():
		return
	_start(sha, _import.bind(root, asset.duplicate(true)))


func import_count() -> int:
	return tasks.size() - int(tasks.has("availability"))


func _publish(key: String, result: Dictionary) -> void:
	result.key = key
	mutex.lock()
	completed[key] = result
	mutex.unlock()


func take_completed() -> Array[Dictionary]:
	var ready: Array[Dictionary] = []
	for key: String in tasks.keys():
		var task: Dictionary = tasks[key]
		var result: Dictionary
		if WorkerThreadPool.is_task_completed(task.id):
			WorkerThreadPool.wait_for_task_completion(task.id)
			mutex.lock()
			result = completed.get(
				key, failure("worker_failed", "Background task ended without a result")
			)
			completed.erase(key)
			mutex.unlock()
		elif Time.get_ticks_msec() - task.started >= TASK_TIMEOUT_MS:
			result = failure("worker_timeout", "Background task exceeded its time budget")
			terminal_error = result.duplicate()
			# Native calls cannot be interrupted safely. Stop new work and reap late results.
			retired_tasks[key] = task
		else:
			continue
		result.key = key
		tasks.erase(key)
		ready.append(result)
	for key: String in retired_tasks.keys():
		if WorkerThreadPool.is_task_completed(retired_tasks[key].id):
			WorkerThreadPool.wait_for_task_completion(retired_tasks[key].id)
			retired_tasks.erase(key)
			mutex.lock()
			completed.erase(key)
			mutex.unlock()
	_start_scan()
	return ready


func discard_completed() -> void:
	take_completed()


func drained() -> bool:
	return tasks.is_empty() and retired_tasks.is_empty()


func shutdown() -> void:
	# Only application exit waits for running tasks. World switches retire this loader.
	for task: Dictionary in tasks.values() + retired_tasks.values():
		WorkerThreadPool.wait_for_task_completion(task.id)
	tasks.clear()
	retired_tasks.clear()
	mutex.lock()
	completed.clear()
	mutex.unlock()


func _file_availability(root: String, asset: Dictionary) -> Dictionary:
	var path := root.path_join(str(asset.path))
	var directory := DirAccess.open(root)
	var folder := DirAccess.open(path.get_base_dir())
	if (
		(directory != null and directory.is_link("objects"))
		or (folder != null and folder.is_link(path.get_file()))
	):
		return failure("unsafe_asset_path", "Symlink asset forbidden")
	return {} if FileAccess.file_exists(path) else failure(MISSING_ASSET, "Asset missing")


func _availability(root: String, asset: Dictionary) -> Dictionary:
	for payload: Dictionary in [asset] + asset.get("textures", []):
		var error := _file_availability(root, payload)
		if not error.is_empty():
			return error
	return {}


static func payload_bytes(asset: Dictionary) -> int:
	var total: int = asset.bytes
	for texture: Dictionary in asset.get("textures", []):
		total += int(texture.bytes)
	return total


func _scan(root: String, assets: Dictionary) -> void:
	var errors: Dictionary = {}
	for sha: String in assets:
		var error := _availability(root, assets[sha])
		if not error.is_empty() and (remote_source == null or error.code != MISSING_ASSET):
			errors[sha] = error
	_publish("availability", {"errors": errors})


func _import(root: String, asset: Dictionary) -> void:
	var started := Time.get_ticks_usec()
	var result := _read_asset(root, asset)
	if not result.has("error") and parse_in_worker:
		var metrics: Dictionary = result.metrics
		result = prepare(result.bytes, result.base_path)
		result.metrics = metrics
	result.import_bytes = payload_bytes(asset)
	result.seconds = (Time.get_ticks_usec() - started) / 1000000.0
	result.worker_task = WorkerThreadPool.get_caller_task_id()
	_publish(asset.sha256, result)


func _read_asset(root: String, asset: Dictionary) -> Dictionary:
	if remote_source != null:
		var downloaded: Dictionary = remote_source.materialize_asset(asset)
		if not downloaded.is_empty():
			return downloaded
	var error := _availability(root, asset)
	if not error.is_empty():
		return error
	var file := FileAccess.open(root.path_join(str(asset.path)), FileAccess.READ)
	if file == null:
		return failure(
			"asset_read_failed", "Cannot open asset: " + str(FileAccess.get_open_error())
		)
	if file.get_length() > Core.config().limits.safety.glb_bytes:
		return failure("asset_budget", "Asset exceeds safety limit: GLB_BYTES")
	var bytes := file.get_buffer(file.get_length())
	file.close()
	var inspected := Core.invoke("glb.inspect", bytes)
	if not inspected.ok:
		return failure(inspected.error.code, Core.message(inspected))
	var matches := Core.invoke(
		"asset.match",
		{
			"asset": {"path": asset.path, "sha256": asset.sha256, "bytes": asset.bytes},
			"actual": inspected.value.descriptor
		}
	)
	if not matches.ok:
		return failure(matches.error.code, Core.message(matches))
	var images: Dictionary = {}
	for texture: Dictionary in asset.get("textures", []):
		var image_file := FileAccess.open(root.path_join(texture.path), FileAccess.READ)
		if image_file == null:
			return failure("asset_read_failed", "Cannot open texture")
		if image_file.get_length() > Core.config().limits.safety.image_bytes:
			return failure("asset_budget", "Texture exceeds safety limit: GLB_BYTES")
		var image_bytes := image_file.get_buffer(image_file.get_length())
		image_file.close()
		var image_result := Core.invoke("image.inspect", image_bytes)
		if not image_result.ok:
			return failure(image_result.error.code, Core.message(image_result))
		var matched := Core.invoke(
			"texture.match",
			{
				"texture": texture,
				"actual": image_result.value.descriptor,
				"metrics": image_result.value.metrics
			}
		)
		if not matched.ok:
			return failure(matched.error.code, Core.message(matched))
		images[texture.path.get_file()] = image_result.value.metrics
	var combined := Core.invoke(
		"glb.textures",
		{
			"metrics": inspected.value.metrics,
			"textures": asset.get("textures", []),
			"images": images
		}
	)
	if not combined.ok:
		return failure(combined.error.code, Core.message(combined))
	return {
		"bytes": bytes,
		"metrics": combined.value,
		"base_path": ProjectSettings.globalize_path(root.path_join(str(asset.path)).get_base_dir())
	}


static func prepare(bytes: PackedByteArray, base_path: String = "") -> Dictionary:
	var document := GLTFDocument.new()
	var state := GLTFState.new()
	state.base_path = base_path
	# append_from_buffer derives a directory from this argument, including its final slash.
	var import_path := base_path + "/" if not base_path.is_empty() else ""
	if document.append_from_buffer(bytes, import_path, state) != OK:
		return failure("gltf_import_failed", "glTF import failed")
	var lods := 0
	for mesh: GLTFMesh in state.get_meshes():
		if mesh.mesh != null:
			mesh.mesh.generate_lods(25.0, 60.0, [])
			for surface: int in mesh.mesh.get_surface_count():
				lods += mesh.mesh.get_surface_lod_count(surface)
	return {"document": document, "state": state, "lods": lods}


static func materialize(result: Dictionary) -> Dictionary:
	# Called only when mounting the first instance, at most once per frame.
	var prepared := (
		prepare(result.bytes, result.get("base_path", "")) if result.has("bytes") else result
	)
	if prepared.has("error"):
		return prepared
	var scene: Node3D = prepared.document.generate_scene(prepared.state)
	if scene == null:
		return failure("empty_gltf_scene", "Empty glTF scene")
	result.scene = scene
	result.lods = prepared.lods
	result.erase("document")
	result.erase("state")
	result.erase("bytes")
	return {}


static func dispose(result: Dictionary) -> void:
	if result.has("scene"):
		result.scene.free()
	result.clear()
