## Bundle IO only. Byte decoding, budgets and document rules come from Rust.
extends RefCounted
const Core = preload("core.gd")


static func read_bytes(path: String, maximum: int) -> Dictionary:
	var directory := DirAccess.open(path.get_base_dir())
	if directory != null and directory.is_link(path.get_file()):
		return {"error": "Catalog symlink forbidden"}
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		return {"error": "Could not open world: " + path}
	if file.get_length() > maximum:
		return {"error": "Catalog exceeds byte budget"}
	return {"bytes": file.get_buffer(file.get_length())}


static func check_layout(path: String) -> String:
	if path.get_file() != "realm.json":
		return "Browse bundle catalog must be realm.json"
	var root := ProjectSettings.globalize_path(path.get_base_dir())
	var parent := DirAccess.open(root.get_base_dir())
	if parent != null and parent.is_link(root.get_file()):
		return "Bundle symlink forbidden"
	var directory := DirAccess.open(root)
	if directory == null:
		return "Could not open bundle directory"
	directory.include_hidden = true
	directory.list_dir_begin()
	var name := directory.get_next()
	while not name.is_empty():
		if directory.is_link(name):
			return "Bundle symlink forbidden"
		if name == ".DS_Store" and not directory.current_is_dir():
			name = directory.get_next()
			continue
		if name in ["objects", "chunks"]:
			if not directory.current_is_dir():
				return "Bundle payload paths must be directories"
		elif name in ["realm.json", "version.json"]:
			if directory.current_is_dir():
				return "Bundle catalog and sidecar must be files"
		else:
			return "Unexpected bundle path: " + name
		name = directory.get_next()
	var objects := DirAccess.open(root.path_join("objects"))
	if objects == null:
		return "Bundle objects directory missing"
	objects.include_hidden = true
	objects.list_dir_begin()
	name = objects.get_next()
	var filename := RegEx.create_from_string("^[0-9a-f]{64}\\.(glb|png|jpg)$")
	while not name.is_empty():
		if objects.is_link(name):
			return "Bundle object symlink forbidden"
		if name == ".DS_Store" and not objects.current_is_dir():
			name = objects.get_next()
			continue
		if objects.current_is_dir() or filename.search(name) == null:
			return "Unexpected bundle object: " + name
		name = objects.get_next()
	var chunks := DirAccess.open(root.path_join("chunks"))
	if chunks == null:
		return ""
	chunks.include_hidden = true
	chunks.list_dir_begin()
	name = chunks.get_next()
	while not name.is_empty():
		if chunks.is_link(name) or chunks.current_is_dir():
			return "Bundle page must be a regular file, not a symlink"
		if name != ".DS_Store" and not name.ends_with(".json"):
			return "Unexpected bundle page: " + name
		name = chunks.get_next()
	return ""


static func page(root: String, descriptor: Dictionary) -> Dictionary:
	var directory := DirAccess.open(root)
	if directory == null or directory.is_link("chunks"):
		return {"error": "Catalog page directory unavailable or symlinked"}
	return read_bytes(root.path_join(descriptor.page), Core.config().limits.safety.json_bytes)


static func read(path: String, ref: String = "") -> Dictionary:
	var config := Core.config()
	if DirAccess.dir_exists_absolute(path):
		path = path.path_join(config.paths.realm)
	var error := check_layout(path)
	if not error.is_empty():
		return {"error": error}
	var bytes := read_bytes(path, config.limits.safety.json_bytes)
	if bytes.has("error"):
		return bytes
	var parsed := Core.invoke("manifest.validate", bytes.bytes)
	if not parsed.ok:
		return {"error": Core.message(parsed)}
	var manifest: Dictionary = parsed.value
	if not DirAccess.dir_exists_absolute(path.get_base_dir().path_join("chunks")):
		return {"error": "Bundle chunks directory missing"}
	for name: String in DirAccess.get_files_at(path.get_base_dir().path_join("chunks")):
		if name == ".DS_Store":
			continue
		var key := Core.invoke(
			"viewer.chunk", {"profile": manifest.profile, "selector": name.trim_suffix(".json")}
		)
		if not key.ok:
			return {"error": "Unexpected bundle page: " + name}
	var version: Dictionary = {}
	var sidecar := path.get_base_dir().path_join(config.paths.version)
	if FileAccess.file_exists(sidecar):
		var stored := read_bytes(sidecar, config.limits.safety.json_bytes)
		if stored.has("error"):
			return stored
		var decoded := Core.invoke("json.strict", stored.bytes)
		if not decoded.ok:
			return {"error": Core.message(decoded)}
		var valid := Core.invoke(
			"version.validate",
			{"version": decoded.value, "catalog": bytes.bytes.get_string_from_utf8()}
		)
		if not valid.ok:
			return {"error": Core.message(valid)}
		version = valid.value
	if not ref.is_empty() and version.get("commit_oid", "") != ref:
		return {"error": "This bundle does not contain the requested fixed Git version"}
	var native_catalog := Rou.new()
	var layout := native_catalog.load_catalog(bytes.bytes)
	if not layout.ok:
		return {"error": Core.message(layout)}
	return {
		"manifest": manifest,
		"native_catalog": native_catalog,
		"repository": version,
		"layout": layout.value,
		"path": path,
		"asset_root": path.get_base_dir()
	}
