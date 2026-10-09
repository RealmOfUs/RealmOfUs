## Materialize online browse sources into ordinary validated local bundles.
extends RefCounted
const Core = preload("core.gd")
const HTTP_TIMEOUT_MS := 30000
const API_BYTES := 8 * 1024 * 1024
var cache_base := "user://cache"
var tasks: Dictionary = {}
var completed: Dictionary = {}
var mutex := Mutex.new()
var download_mutex := Mutex.new()
var active: Dictionary = {}
var source_address := ""
var downloaded_bytes := 0
var cache_hits := 0
var request_count := 0
var failed_requests := 0
var in_flight_bytes := 0
var download_order: Array[String] = []
var object_downloaded_bytes := 0


static func failure(code: String, message: String) -> Dictionary:
	return {"code": code, "error": message}


func start(address: String, force: bool = false) -> Dictionary:
	var parsed := Core.invoke("url.parse", address)
	if not parsed.ok:
		return failure(parsed.error.code, Core.message(parsed))
	if tasks.has("bootstrap"):
		return failure("remote_busy", "Another online source is already opening")
	source_address = address
	active.clear()
	downloaded_bytes = 0
	cache_hits = 0
	request_count = 0
	failed_requests = 0
	in_flight_bytes = 0
	download_order.clear()
	object_downloaded_bytes = 0
	_start("bootstrap", _bootstrap.bind(parsed.value, force))
	return {}


func activate(result: Dictionary) -> void:
	active = result.duplicate(true)
	active.erase("key")
	active.erase("cached_bootstrap")


func request_page(key: String, descriptor: Dictionary) -> void:
	var task_key := "page:" + key
	if active.is_empty() or tasks.has(task_key):
		return
	_start(task_key, _download_page.bind(key, descriptor.duplicate(true)))


func page_pending(key: String) -> bool:
	return tasks.has("page:" + key)


func _start(key: String, work: Callable) -> void:
	tasks[key] = WorkerThreadPool.add_task(work)


func _publish(key: String, result: Dictionary) -> void:
	result.key = key
	mutex.lock()
	completed[key] = result
	mutex.unlock()


func take_completed() -> Array[Dictionary]:
	var ready: Array[Dictionary] = []
	for key: String in tasks.keys():
		var task: int = tasks[key]
		if not WorkerThreadPool.is_task_completed(task):
			continue
		WorkerThreadPool.wait_for_task_completion(task)
		mutex.lock()
		var result: Dictionary = completed.get(
			key, failure("worker_failed", "Online worker ended without a result")
		)
		completed.erase(key)
		mutex.unlock()
		result.key = key
		tasks.erase(key)
		ready.append(result)
	return ready


func shutdown() -> void:
	for task: int in tasks.values():
		WorkerThreadPool.wait_for_task_completion(task)
	tasks.clear()
	mutex.lock()
	completed.clear()
	mutex.unlock()


func stats() -> Dictionary:
	mutex.lock()
	var result := {
		"source": source_address,
		"tag": active.get("tag", ""),
		"downloaded_bytes": downloaded_bytes,
		"cache_hits": cache_hits,
		"requests": request_count,
		"failed_requests": failed_requests,
		"in_flight_bytes": in_flight_bytes,
		"download_order": download_order.duplicate(),
		"object_downloaded_bytes": object_downloaded_bytes
	}
	mutex.unlock()
	return result


func materialize_asset(asset: Dictionary) -> Dictionary:
	if active.is_empty():
		return failure("remote_inactive", "Online source is not ready")
	download_mutex.lock()
	var result := _materialize_locked(asset)
	download_mutex.unlock()
	return result


func _bootstrap(source: Dictionary, force: bool) -> void:
	var result: Dictionary
	if source.kind == "repository":
		result = _bootstrap_repository(source, force)
	else:
		result = _bootstrap_static(source)
	_publish("bootstrap", result)


func _bootstrap_repository(source: Dictionary, force: bool) -> Dictionary:
	var explicit_tag := source.get("tag") is String and not str(source.tag).is_empty()
	if explicit_tag and not force:
		var cached := _read_index(source.address)
		if not cached.is_empty():
			cached.cached_bootstrap = true
			return cached
	var tag_info: Dictionary
	if explicit_tag:
		var response := _json_request(
			HTTPClient.METHOD_GET,
			(
				"%s/api/v1/repos/%s/%s/tags/%s"
				% [source.origin, source.owner, source.repository, _encode_component(source.tag)]
			)
		)
		if response.has("error"):
			return response
		if response.status == 404:
			return failure("release_not_found", "The requested browse tag does not exist")
		if response.status != 200 or not response.value is Dictionary:
			return failure("remote_http", "Could not read the repository tag")
		tag_info = response.value
	else:
		var response := _json_request(
			HTTPClient.METHOD_GET,
			(
				"%s/api/v1/repos/%s/%s/tags?limit=100"
				% [source.origin, source.owner, source.repository]
			)
		)
		if response.has("error"):
			return response
		if response.status != 200 or not response.value is Array:
			return failure("remote_http", "Could not list repository tags")
		# Tags are newest first; only annotated tags carrying a view-root line are browse releases.
		for candidate: Variant in response.value:
			if (
				candidate is Dictionary
				and not _annotation_root(str(candidate.get("message", ""))).is_empty()
			):
				tag_info = candidate
				break
		if tag_info.is_empty():
			return failure("release_not_found", "This repository has no browse release tag")
	var tag: String = str(tag_info.get("name", ""))
	var view_root := _annotation_root(str(tag_info.get("message", "")))
	var commit_info: Variant = tag_info.get("commit", {})
	var commit: String = str(commit_info.get("sha", "")) if commit_info is Dictionary else ""
	var archive_url: String = str(tag_info.get("zipball_url", ""))
	if tag.is_empty() or view_root.is_empty() or commit.is_empty() or archive_url.is_empty():
		return failure("invalid_release", "Tag lacks a view-root annotation, commit or archive")
	if explicit_tag and tag != source.tag:
		return failure("invalid_release", "Repository returned a different tag")
	var archive := _http(
		HTTPClient.METHOD_GET,
		archive_url,
		[],
		PackedByteArray(),
		Core.config().limits.safety.archive_bytes
	)
	if archive.has("error"):
		return archive
	var tree := _read_tree(archive.bytes)
	if tree.has("error"):
		return tree
	# The viewer derives the same catalog rou view writes; the annotation must agree.
	var derived := Core.invoke(
		"tree.bundle", {"commit_oid": commit, "documents": tree.documents, "inputs": tree.inputs}
	)
	if not derived.ok:
		return failure(derived.error.code, Core.message(derived))
	if derived.value.view_root_sha256 != view_root:
		return failure("invalid_release", "Tag annotation and derived view root differ")
	return _install_bootstrap(
		source,
		str(derived.value.root).to_utf8_buffer(),
		str(derived.value.version).to_utf8_buffer(),
		tag,
		commit,
		view_root,
		derived.value.pages
	)


func _read_tree(archive: PackedByteArray) -> Dictionary:
	var download_root := cache_base.path_join(".downloads")
	var directory := _ensure_directory(download_root)
	if not directory.is_empty():
		return directory
	var temporary := "%s/%d-%d.zip" % [download_root, Time.get_ticks_usec(), randi()]
	var file := FileAccess.open(temporary, FileAccess.WRITE)
	if file == null:
		return failure("cache_write_failed", "Could not store the repository archive")
	file.store_buffer(archive)
	file.close()
	var reader := ZIPReader.new()
	var opened := reader.open(temporary)
	if opened != OK:
		DirAccess.remove_absolute(ProjectSettings.globalize_path(temporary))
		return failure("invalid_archive", "Repository archive is not a readable zip")
	var documents: Dictionary = {}
	var inputs: Dictionary = {}
	var budget: int = Core.config().limits.safety.archive_bytes
	var consumed := 0
	var result: Dictionary = {}
	for name: String in reader.get_files():
		if name.ends_with("/") or not name.contains("/"):
			continue
		# Forge archives wrap the tree in one top-level directory named after the repository.
		var relative := name.split("/", true, 1)[1]
		var is_document := (
			relative == "realm.json"
			or (relative.begins_with("realm/") and relative.ends_with(".json"))
			or (relative.begins_with("runtime/") and relative.ends_with(".json"))
		)
		var is_input := relative.begins_with("sources/") or relative.begins_with("runtime/")
		if not is_document and not is_input:
			continue
		var data := reader.read_file(name)
		consumed += data.size()
		if consumed > budget:
			result = failure("archive_budget", "Repository archive exceeds the safety budget")
			break
		if is_document:
			documents[relative] = data.get_string_from_utf8()
			continue
		var pointer := Core.invoke("lfs.pointer", data)
		if pointer.ok and pointer.value is Dictionary:
			inputs[relative] = pointer.value
		else:
			var descriptor := Core.invoke("file.descriptor", data)
			if not descriptor.ok:
				result = failure(descriptor.error.code, Core.message(descriptor))
				break
			inputs[relative] = descriptor.value
	reader.close()
	DirAccess.remove_absolute(ProjectSettings.globalize_path(temporary))
	if not result.is_empty():
		return result
	if not documents.has("realm.json"):
		return failure("invalid_archive", "Repository archive has no realm.json")
	return {"documents": documents, "inputs": inputs}


func _bootstrap_static(source: Dictionary) -> Dictionary:
	var root := _http(
		HTTPClient.METHOD_GET,
		source.manifest_url,
		[],
		PackedByteArray(),
		Core.config().limits.safety.json_bytes
	)
	if root.has("error"):
		return root
	var sidecar := _http(
		HTTPClient.METHOD_GET,
		source.base_url + "/version.json",
		[],
		PackedByteArray(),
		Core.config().limits.safety.json_bytes
	)
	if sidecar.has("error"):
		return sidecar
	return _install_bootstrap(source, root.bytes, sidecar.bytes, "", "", "", {})


func _install_bootstrap(
	source: Dictionary,
	root_bytes: PackedByteArray,
	version_bytes: PackedByteArray,
	tag: String,
	target_commit: String,
	annotation_root: String,
	pages: Dictionary
) -> Dictionary:
	var manifest_result := Core.invoke("manifest.validate", root_bytes)
	if not manifest_result.ok:
		return failure(manifest_result.error.code, Core.message(manifest_result))
	var decoded := Core.invoke("json.strict", version_bytes)
	if not decoded.ok or not decoded.value is Dictionary:
		return failure("invalid_version", "Hosted version.json is invalid")
	var validated := Core.invoke(
		"version.validate", {"version": decoded.value, "catalog": root_bytes.get_string_from_utf8()}
	)
	if not validated.ok:
		return failure(validated.error.code, Core.message(validated))
	var version: Dictionary = validated.value
	if not annotation_root.is_empty() and version.view_root_sha256 != annotation_root:
		return failure("invalid_release", "Release annotation and version view root differ")
	if not target_commit.is_empty() and version.commit_oid != target_commit:
		return failure("invalid_release", "Release tag target and version commit differ")
	if version.commit_oid.is_empty():
		return failure("invalid_release", "Online bundles must contain a fixed Git version")
	var cache_root := "%s/%s/%s" % [cache_base, version.realm_id, version.catalog_sha256]
	var error := _ensure_directory(cache_root.path_join("chunks"))
	if not error.is_empty():
		return error
	error = _ensure_directory(cache_root.path_join("objects"))
	if not error.is_empty():
		return error
	error = _install_bytes(cache_root.path_join("realm.json"), root_bytes)
	if not error.is_empty():
		return error
	error = _install_bytes(cache_root.path_join("version.json"), version_bytes)
	if not error.is_empty():
		return error
	error = _install_pages(cache_root, pages)
	if not error.is_empty():
		return error
	var result := {
		"source": source,
		"cache_root": cache_root,
		"manifest": cache_root.path_join("realm.json"),
		"realm_id": version.realm_id,
		"catalog_sha256": version.catalog_sha256,
		"commit_oid": version.commit_oid,
		"view_root_sha256": version.view_root_sha256,
		"tag": tag,
		"base_url": source.get("base_url", "")
	}
	if source.kind == "repository" and source.get("tag") is String:
		var written := _write_index(source.address, result)
		if not written.is_empty():
			return written
	return result


func _install_pages(cache_root: String, pages: Dictionary) -> Dictionary:
	# Derived pages are installed up front; static sources fetch pages on demand.
	for page_path: String in pages:
		if not page_path.begins_with("chunks/") or page_path.contains(".."):
			return failure("invalid_release", "Derived page has an unexpected path")
		var error := _install_bytes(
			cache_root.path_join(page_path), str(pages[page_path]).to_utf8_buffer()
		)
		if not error.is_empty():
			return error
	return {}


func _download_page(key: String, descriptor: Dictionary) -> void:
	var result: Dictionary
	var destination: String = active.cache_root.path_join(descriptor.page)
	if _descriptor_matches(destination, descriptor):
		_count_cache_hit()
		result = {"page": key, "path": destination, "cached": true}
	else:
		var url: String = ""
		if active.source.kind != "repository":
			url = active.base_url + "/" + descriptor.page
		if url.is_empty():
			# Repository pages were derived at open time; a mismatch means the cache changed.
			result = failure("remote_missing_page", "Derived catalog page is missing or altered")
		else:
			var response := _http(
				HTTPClient.METHOD_GET,
				url,
				[],
				PackedByteArray(),
				Core.config().limits.safety.json_bytes
			)
			if response.has("error"):
				result = response
			elif not _bytes_match(response.bytes, descriptor):
				result = failure("asset_mismatch", "Catalog page hash or size mismatch")
			else:
				result = _install_bytes(destination, response.bytes)
				if result.is_empty():
					result = {"page": key, "path": destination, "cached": false}
	_publish("page:" + key, result)


func _materialize_locked(asset: Dictionary) -> Dictionary:
	var payloads: Array = [asset]
	payloads.append_array(asset.get("textures", []))
	var missing: Array = []
	for payload: Dictionary in payloads:
		var destination: String = active.cache_root.path_join(payload.path)
		var existed := FileAccess.file_exists(destination)
		if _descriptor_matches(destination, payload):
			_count_cache_hit()
			continue
		if existed:
			missing.append(payload)
			continue
		var shared := _shared_object_path(payload.path)
		if _descriptor_matches(shared, payload):
			var shared_file := FileAccess.open(shared, FileAccess.READ)
			if shared_file != null:
				var copied := shared_file.get_buffer(shared_file.get_length())
				shared_file.close()
				var installed := _install_bytes(destination, copied)
				if installed.is_empty():
					_count_cache_hit()
					continue
		missing.append(payload)
	if missing.is_empty():
		return {}
	if active.source.kind == "repository":
		return _download_lfs(missing)
	for payload: Dictionary in missing:
		var response := _http(
			HTTPClient.METHOD_GET,
			active.base_url + "/" + payload.path,
			[],
			PackedByteArray(),
			int(payload.bytes) + 1
		)
		if response.has("error"):
			return response
		var installed := _install_payload(payload, response.bytes)
		if not installed.is_empty():
			return installed
	return {}


func _download_lfs(payloads: Array) -> Dictionary:
	var objects: Array = []
	for payload: Dictionary in payloads:
		objects.append({"oid": payload.sha256, "size": payload.bytes})
	var body := JSON.stringify({"operation": "download", "objects": objects}).to_utf8_buffer()
	var response := _http(
		HTTPClient.METHOD_POST,
		(
			"%s/%s/%s.git/info/lfs/objects/batch"
			% [active.source.origin, active.source.owner, active.source.repository]
		),
		["Content-Type: application/vnd.git-lfs+json", "Accept: application/vnd.git-lfs+json"],
		body,
		API_BYTES
	)
	if response.has("error"):
		return response
	var decoded := Core.invoke("json.strict", response.bytes)
	if not decoded.ok or not decoded.value is Dictionary:
		return failure("invalid_lfs_response", "LFS batch response is invalid")
	var offered: Dictionary = {}
	for item: Variant in decoded.value.get("objects", []):
		if item is Dictionary:
			offered[item.get("oid", "")] = item
	for payload: Dictionary in payloads:
		var item: Dictionary = offered.get(payload.sha256, {})
		var action: Dictionary = item.get("actions", {}).get("download", {})
		var href: String = action.get("href", "")
		if href.is_empty():
			return failure("missing_lfs_object", "LFS server did not offer a required object")
		var headers: Array[String] = []
		for header: String in action.get("header", {}):
			headers.append(header + ": " + str(action.header[header]))
		var download := _http(
			HTTPClient.METHOD_GET, href, headers, PackedByteArray(), int(payload.bytes) + 1
		)
		if download.has("error"):
			return download
		var installed := _install_payload(payload, download.bytes)
		if not installed.is_empty():
			return installed
	return {}


func _install_payload(payload: Dictionary, bytes: PackedByteArray) -> Dictionary:
	if not _bytes_match(bytes, payload):
		return failure("asset_mismatch", "Downloaded object hash or size mismatch")
	var shared := _shared_object_path(payload.path)
	var installed := _install_bytes(shared, bytes)
	if not installed.is_empty():
		return installed
	installed = _install_bytes(active.cache_root.path_join(payload.path), bytes)
	if not installed.is_empty():
		return installed
	if not _descriptor_matches(active.cache_root.path_join(payload.path), payload):
		return failure("asset_mismatch", "Installed object hash or size mismatch")
	mutex.lock()
	download_order.append(str(payload.path).get_file())
	object_downloaded_bytes += bytes.size()
	mutex.unlock()
	return {}


func _shared_object_path(payload_path: String) -> String:
	return "%s/%s/_objects/%s" % [cache_base, active.realm_id, payload_path.get_file()]


func _descriptor_matches(path: String, descriptor: Dictionary) -> bool:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null or file.get_length() != int(descriptor.bytes):
		if file != null:
			file.close()
		return false
	var bytes := file.get_buffer(file.get_length())
	file.close()
	return _bytes_match(bytes, descriptor)


func _bytes_match(bytes: PackedByteArray, descriptor: Dictionary) -> bool:
	var actual := Core.invoke("file.descriptor", bytes)
	return (
		actual.ok
		and actual.value.sha256 == descriptor.sha256
		and int(actual.value.bytes) == int(descriptor.bytes)
	)


func _annotation_root(message: String) -> String:
	var parsed := Core.invoke("release.annotation", {"action": "parse", "annotation": message})
	if not parsed.ok:
		return ""
	var root: Variant = parsed.value.get("view_root_sha256")
	return root if root is String else ""


func _json_request(method: int, url: String) -> Dictionary:
	var response := _http(
		method, url, ["Accept: application/json"], PackedByteArray(), API_BYTES, true
	)
	if response.has("error"):
		return response
	var decoded := Core.invoke("json.strict", response.bytes)
	if not decoded.ok:
		return failure(decoded.error.code, Core.message(decoded))
	return {"status": response.status, "value": decoded.value}


func _http(
	method: int,
	url: String,
	headers: Array[String],
	body: PackedByteArray,
	maximum: int,
	accept_error_status: bool = false,
	redirects: int = 0
) -> Dictionary:
	if redirects > 5:
		return failure("remote_redirect", "Too many HTTP redirects")
	var parsed := _parse_http_url(url)
	if parsed.has("error"):
		return parsed
	var client := HTTPClient.new()
	var tls := TLSOptions.client() if parsed.scheme == "https" else null
	var connected := client.connect_to_host(parsed.host, parsed.port, tls)
	if connected != OK:
		_count_failure()
		return failure("remote_connect", "Could not start HTTP connection")
	var deadline := Time.get_ticks_msec() + HTTP_TIMEOUT_MS
	while client.get_status() in [HTTPClient.STATUS_RESOLVING, HTTPClient.STATUS_CONNECTING]:
		client.poll()
		if Time.get_ticks_msec() >= deadline:
			client.close()
			_count_failure()
			return failure("remote_timeout", "HTTP connection timed out")
		OS.delay_msec(2)
	if client.get_status() != HTTPClient.STATUS_CONNECTED:
		client.close()
		_count_failure()
		return failure("remote_connect", "Could not connect to online source")
	var request_headers: Array[String] = [
		"User-Agent: RealmOfUs-Viewer", "Accept-Encoding: identity"
	]
	request_headers.append_array(headers)
	_count_request()
	var requested := client.request_raw(method, parsed.target, request_headers, body)
	if requested != OK:
		client.close()
		_count_failure()
		return failure("remote_http", "Could not send HTTP request")
	while (
		not client.has_response()
		and client.get_status() in [HTTPClient.STATUS_REQUESTING, HTTPClient.STATUS_CONNECTED]
	):
		client.poll()
		if Time.get_ticks_msec() >= deadline:
			client.close()
			_count_failure()
			return failure("remote_timeout", "HTTP request timed out")
		OS.delay_msec(2)
	if not client.has_response():
		client.close()
		_count_failure()
		return failure("remote_http", "HTTP server returned no response")
	var status := client.get_response_code()
	var response_headers := client.get_response_headers_as_dictionary()
	var expected := client.get_response_body_length()
	if expected > maximum:
		client.close()
		_count_failure()
		return failure("remote_budget", "HTTP response exceeds its byte budget")
	_set_in_flight(maxi(expected, 0))
	var bytes := PackedByteArray()
	while client.get_status() == HTTPClient.STATUS_BODY:
		client.poll()
		var chunk := client.read_response_body_chunk()
		if not chunk.is_empty():
			bytes.append_array(chunk)
			if bytes.size() > maximum:
				client.close()
				_set_in_flight(0)
				_count_failure()
				return failure("remote_budget", "HTTP response exceeds its byte budget")
		elif Time.get_ticks_msec() >= deadline:
			client.close()
			_set_in_flight(0)
			_count_failure()
			return failure("remote_timeout", "HTTP response timed out")
		else:
			OS.delay_msec(2)
	client.close()
	_set_in_flight(0)
	_count_download(bytes.size())
	if status in [301, 302, 303, 307, 308]:
		var location: String = response_headers.get(
			"Location", response_headers.get("location", "")
		)
		if location.is_empty():
			_count_failure()
			return failure("remote_redirect", "HTTP redirect has no location")
		if location.begins_with("/"):
			location = "%s://%s%s" % [parsed.scheme, parsed.authority, location]
		return _http(
			HTTPClient.METHOD_GET if status == 303 else method,
			location,
			headers,
			PackedByteArray() if status == 303 else body,
			maximum,
			accept_error_status,
			redirects + 1
		)
	if (status < 200 or status >= 300) and not accept_error_status:
		_count_failure()
		return failure("remote_http", "HTTP request failed with status %d" % status)
	return {"status": status, "bytes": bytes, "headers": response_headers}


func _parse_http_url(url: String) -> Dictionary:
	var expression := RegEx.create_from_string(
		"^(https?)://(\\[[0-9A-Fa-f:]+\\]|[^/:?#]+)(?::([0-9]+))?(/[^#]*)?$"
	)
	var match := expression.search(url)
	if match == null:
		return failure("invalid_url", "HTTP download URL is invalid")
	var scheme := match.get_string(1)
	var host := match.get_string(2)
	if host.begins_with("["):
		host = host.trim_prefix("[").trim_suffix("]")
	var port_text := match.get_string(3)
	var port := int(port_text) if not port_text.is_empty() else (443 if scheme == "https" else 80)
	if port <= 0 or port > 65535:
		return failure("invalid_url", "HTTP download port is invalid")
	var target := match.get_string(4)
	if target.is_empty():
		target = "/"
	var authority := match.get_string(2) + ((":" + port_text) if not port_text.is_empty() else "")
	return {"scheme": scheme, "host": host, "port": port, "target": target, "authority": authority}


func _ensure_directory(path: String) -> Dictionary:
	var absolute := ProjectSettings.globalize_path(path)
	var code := DirAccess.make_dir_recursive_absolute(absolute)
	return {} if code == OK else failure("cache_write_failed", "Could not create cache directory")


func _install_bytes(path: String, bytes: PackedByteArray) -> Dictionary:
	var parent := _ensure_directory(path.get_base_dir())
	if not parent.is_empty():
		return parent
	var download_root := cache_base.path_join(".downloads")
	var download_directory := _ensure_directory(download_root)
	if not download_directory.is_empty():
		return download_directory
	var temporary := "%s/%d-%d.tmp" % [download_root, Time.get_ticks_usec(), randi()]
	var file := FileAccess.open(temporary, FileAccess.WRITE)
	if file == null:
		return failure("cache_write_failed", "Could not create cache temporary file")
	file.store_buffer(bytes)
	file.close()
	var absolute_path := ProjectSettings.globalize_path(path)
	var absolute_temporary := ProjectSettings.globalize_path(temporary)
	if FileAccess.file_exists(path):
		var removed := DirAccess.remove_absolute(absolute_path)
		if removed != OK:
			DirAccess.remove_absolute(absolute_temporary)
			return failure("cache_write_failed", "Could not replace cache file")
	var renamed := DirAccess.rename_absolute(absolute_temporary, absolute_path)
	if renamed != OK:
		DirAccess.remove_absolute(absolute_temporary)
		return failure("cache_write_failed", "Could not finish cache file")
	return {}


func _index_path(address: String) -> String:
	var digest := Core.invoke("digest.sha256", address.to_utf8_buffer())
	return "%s/sources/%s.json" % [cache_base, digest.value]


func _write_index(address: String, value: Dictionary) -> Dictionary:
	var copy := value.duplicate(true)
	copy.format = "rou-online-cache/0.1"
	copy.address = address
	return _install_bytes(
		_index_path(address), (JSON.stringify(copy, "  ") + "\n").to_utf8_buffer()
	)


func _read_index(address: String) -> Dictionary:
	var path := _index_path(address)
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null or file.get_length() > Core.config().limits.safety.json_bytes:
		if file != null:
			file.close()
		return {}
	var bytes := file.get_buffer(file.get_length())
	file.close()
	var decoded := Core.invoke("json.strict", bytes)
	if not decoded.ok or not decoded.value is Dictionary:
		return {}
	var value: Dictionary = decoded.value
	if (
		value.get("format", "") != "rou-online-cache/0.1"
		or value.get("address", "") != address
		or not FileAccess.file_exists(value.get("manifest", ""))
	):
		return {}
	value.erase("format")
	value.erase("address")
	return value


func _encode_component(value: String) -> String:
	return value.uri_encode().replace("%2F", "%2F")


func _count_download(bytes: int) -> void:
	mutex.lock()
	downloaded_bytes += bytes
	mutex.unlock()


func _count_cache_hit() -> void:
	mutex.lock()
	cache_hits += 1
	mutex.unlock()


func _count_request() -> void:
	mutex.lock()
	request_count += 1
	mutex.unlock()


func _count_failure() -> void:
	mutex.lock()
	failed_requests += 1
	mutex.unlock()


func _set_in_flight(bytes: int) -> void:
	mutex.lock()
	in_flight_bytes = bytes
	mutex.unlock()
