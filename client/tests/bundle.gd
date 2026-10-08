## Test fixture IO uses the production core for root/page generation.
extends RefCounted
const Core = preload("res://core.gd")


static func write(folder: String, value: Dictionary) -> void:
	var bundle := value
	if value.has("entities"):
		var drafts: Array = []
		for entity: Dictionary in value.entities:
			var draft: Dictionary = {}
			for key: String in [
				"id", "label", "position_mm", "bounds_mm", "asset", "lod1", "textures"
			]:
				if entity.has(key):
					draft[key] = entity[key]
			drafts.append(draft)
		bundle = Core.invoke("preview.create", {"profile": value.profile, "entities": drafts}).value
	DirAccess.make_dir_recursive_absolute(folder.path_join("chunks"))
	for name: String in DirAccess.get_files_at(folder.path_join("chunks")):
		DirAccess.remove_absolute(folder.path_join("chunks").path_join(name))
	for path: String in bundle.pages:
		var page := FileAccess.open(folder.path_join(path), FileAccess.WRITE)
		page.store_string(bundle.pages[path])
		page.close()
	var root := FileAccess.open(folder.path_join("realm.json"), FileAccess.WRITE)
	root.store_string(JSON.stringify(bundle.root))
	root.close()
