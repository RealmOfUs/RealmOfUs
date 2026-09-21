extends SceneTree
const Core = preload("res://core.gd")
var checks := 0
var errors: Array[String] = []


func check(ok: bool, message: String) -> void:
	checks += 1
	if not ok:
		errors.append(message)
		push_error(message)


func changed(base: Dictionary, case: Dictionary, field: String = "changes") -> Dictionary:
	var value := base.duplicate(true)
	value.merge(case.get(field, {}), true)
	for key: String in case.get("remove", []):
		value.erase(key)
	return value


func _initialize() -> void:
	run_checks.call_deferred()


func run_checks() -> void:
	var bytes := FileAccess.get_file_as_bytes(
		"res://../../rou/crates/rou-core/tests/fixtures/spatial.json"
	)
	var vectors: Dictionary = Core.invoke("json.strict", bytes).value
	var canonical: String = Core.invoke("json.canonical", vectors.profile).value
	check(
		Core.invoke("digest.sha256", canonical.to_utf8_buffer()).value == vectors.profile_hash,
		"Reviewed profile hash"
	)
	for case: Dictionary in vectors.profiles:
		check(
			Core.invoke("profile.validate", changed(vectors.profile, case)).ok == case.accepted,
			"Profile: " + case.name
		)
	for case: Dictionary in vectors.positions:
		var result := Core.invoke(
			"spatial.owner",
			{
				"profile": changed(vectors.profile, case, "profile_changes"),
				"position_mm": case.position_mm
			}
		)
		check(result.ok == case.accepted, "Position: " + case.name)
		if result.ok:
			check(result.value.chunk == case.chunk, "Reviewed chunk: " + case.name)
			check(result.value.offset == case.offset, "Reviewed offset: " + case.name)
	for case: Dictionary in vectors.bounds:
		var result := Core.invoke(
			"spatial.intersections",
			{
				"profile": changed(vectors.profile, case, "profile_changes"),
				"bounds_mm": {"min": case.min, "max": case.max}
			}
		)
		check(result.ok == case.accepted, "Bounds: " + case.name)
		if result.ok and case.has("chunks"):
			check(result.value == case.chunks, "Reviewed intersections: " + case.name)
		if result.ok and case.has("chunk_count"):
			check(result.value.size() == case.chunk_count, "Reviewed coverage budget")
	for case: Dictionary in vectors.catalogs:
		check(
			Core.invoke("manifest.validate", changed(vectors.catalog, case)).ok == case.accepted,
			"Catalog: " + case.name
		)
	for malformed: PackedByteArray in [
		PackedByteArray([0xff]),
		PackedByteArray([0xef, 0xbb, 0xbf, 123, 125]),
		'{"a":1,"a":2}'.to_utf8_buffer(),
		'{"n":1e999}'.to_utf8_buffer()
	]:
		check(not Core.invoke("json.strict", malformed).ok, "Strict raw JSON bytes rejected")
	check(
		not Core.invoke("glb.precheck", PackedByteArray([0, 1, 2])).ok, "Invalid raw GLB rejected"
	)
	var empty: Dictionary = vectors.catalog.duplicate(true)
	empty.entities = []
	empty.chunks = {}
	check(
		(
			Core.invoke("viewer.start", empty).value
			== {"position": [0.0, 2.0, 0.0], "look_at": [0.0, 0.0, 0.0]}
		),
		"Empty world start"
	)
	print("RUST_SPATIAL_PASS ", checks if errors.is_empty() else errors)
	quit(0 if errors.is_empty() else 1)
