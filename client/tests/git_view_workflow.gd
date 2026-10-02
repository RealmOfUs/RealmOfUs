## Exercise fixed Git view bundles without invoking Git, LFS or Blender.
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


func drain(viewer: Node3D) -> void:
	check(await Streaming.drain(viewer), "Background imports complete before deadline")

	for entity_id: String in viewer.loaded_bounds:
		var actual: Dictionary = viewer.loaded_bounds[entity_id]
		var expected: Dictionary = viewer.catalog[entity_id].bounds_mm
		var matches := true
		for axis: int in 3:
			matches = matches and absf(actual.min[axis] - expected.min[axis] * .001) < .003
			matches = matches and absf(actual.max[axis] - expected.max[axis] * .001) < .003
		check(matches, "Runtime geometry agrees with realm bounds: " + entity_id)


func write_text(path: String, value: String) -> void:
	var file := FileAccess.open(path, FileAccess.WRITE)
	file.store_string(value)


func check_bundle_paths(viewer: Node3D, bundle: String) -> void:
	for folder: String in [bundle, bundle.path_join("objects")]:
		write_text(folder.path_join(".DS_Store"), "Finder metadata")
	check(viewer.open_realm(bundle), "Finder metadata does not prevent opening a bundle")
	var parent := bundle.get_base_dir()
	var root_link := parent.path_join("bundle-root-link")
	check(DirAccess.open(parent).create_link(bundle, root_link) == OK, "Create bundle root link")
	check(not viewer.open_realm(root_link), "Bundle root symlink remains forbidden")
	DirAccess.remove_absolute(root_link)
	var ancestor := parent.path_join("bundle-parent-link")
	check(DirAccess.open(parent).create_link(parent, ancestor) == OK, "Create ancestor link")
	check(
		viewer.open_realm(ancestor.path_join(bundle.get_file())),
		"A symlink outside the bundle does not invalidate it"
	)
	DirAccess.remove_absolute(ancestor)
	var ignored := bundle.path_join(".DS_Store")
	DirAccess.remove_absolute(ignored)
	check(
		DirAccess.open(bundle).create_link(bundle.path_join("realm.json"), ignored) == OK,
		"Create symlink with ignored filename"
	)
	check(not viewer.open_realm(bundle), "Ignored filenames do not bypass symlink rejection")
	DirAccess.remove_absolute(ignored)
	write_text(bundle.path_join(".unexpected"), "unrecognized hidden file")
	check(not viewer.open_realm(bundle), "Other hidden files remain forbidden")
	DirAccess.remove_absolute(bundle.path_join(".unexpected"))
	if OS.get_name() == "macOS":
		for prefix: String in ["/tmp", "/var/tmp"]:
			var temporary := prefix.path_join("rou-bundle-test-" + str(Time.get_ticks_usec()))
			check(
				DirAccess.make_dir_recursive_absolute(temporary.path_join("objects")) == OK,
				"Create a bundle below " + prefix
			)
			for name: String in ["realm.json", "version.json"]:
				DirAccess.copy_absolute(bundle.path_join(name), temporary.path_join(name))
			check(viewer.open_realm(temporary), "Open a real macOS temporary directory bundle")
			for name: String in ["realm.json", "version.json", "objects"]:
				DirAccess.remove_absolute(temporary.path_join(name))
			DirAccess.remove_absolute(temporary)
	check(viewer.open_realm(bundle), "Restore original bundle after layout checks")
	await drain(viewer)
	viewer.error_dialog.hide()


func run_test() -> void:
	var options: Dictionary = {}
	for argument: String in OS.get_cmdline_user_args():
		if argument.begins_with("--") and "=" in argument:
			var parts := argument.trim_prefix("--").split("=", true, 1)
			options[parts[0]] = parts[1]
	check(options.has("initial") and options.has("updated"), "Two view bundles supplied")
	if not errors.is_empty():
		quit(1)
		return
	root.size = Vector2i(1920, 1080)
	var viewer = load("res://main.tscn").instantiate()
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	viewer.set_process(false)
	await process_frame
	check(viewer.manifest.is_empty(), "No hardcoded world")
	check(viewer.file_dialog.visible, "Standalone startup opens the world chooser")
	viewer.file_dialog.hide()
	check(options.has("selected"), "Default selection bundle supplied")
	check(viewer.open_realm(options.selected), "Default selection bundle opens")
	check(viewer.catalog.size() == 4, "Partial bundle retains the complete catalog")
	viewer.set_pose(viewer.layout.entities.bench.focus)
	await drain(viewer)
	check(viewer.rejected.get("bench") == "Asset missing", "Unselected model is missing")
	check(viewer.impostors.instances.bench.visible, "Missing asset retains its box")
	check(viewer.impostors.instances.bench.error == "Asset missing", "Missing box is marked")
	viewer.navigate_position(Vector3(500, 500, 500))
	check(viewer.impostors.instances.bench.visible, "Missing box remains visible in far tier")
	var missing_chunk: String = Core.invoke("spatial.key", viewer.catalog.bench.owner_chunk).value
	check(viewer.chunk_state(missing_chunk) == "Asset missing", "Unselected chunk is not empty")

	check(viewer.open_realm(options.initial), "Initial fixed view opens")
	var first: String = viewer.current_version
	check(not first.is_empty(), "Fixed Git commit presented")
	check(viewer.catalog.has("house") and viewer.catalog.has("bench"), "Authored entities present")
	var lower := Vector3(INF, INF, INF)
	var upper := Vector3(-INF, -INF, -INF)
	for entity: Dictionary in viewer.manifest.entities:
		lower = lower.min(Core.vec(entity.bounds_mm.min) * .001)
		upper = upper.max(Core.vec(entity.bounds_mm.max) * .001)
	var center := (lower + upper) * .5
	var expected_start := center + Vector3.UP * lower.distance_to(upper)
	check(
		viewer.camera.position.distance_to(expected_start) < .0001,
		"Default start uses all world bounds and their diagonal"
	)
	check(not viewer.manifest.has("entries"), "Preview has no viewing entries")
	viewer.world_ui.select_entity("house")
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(viewer.rejected.is_empty(), "Initial GLB payloads validated")
	check(not viewer.loaded.is_empty(), "Nearby world models instantiated")
	var initial_count: int = viewer.loaded.size()
	viewer.reload_button.pressed.emit()
	viewer.world_ui.select_entity("house")
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(viewer.current_version == first, "Reload keeps immutable version")
	check(viewer.loaded.size() == initial_count, "Reload does not duplicate entities")
	check(not viewer.open_realm(options.updated, first), "Version mismatch rejected")
	check(viewer.current_version == first, "Failed open preserves previous world")
	viewer.error_dialog.hide()
	check(viewer.open_realm(options.updated), "Updated fixed version opens")
	check(viewer.current_version != first, "New bundle has a different Git commit")
	viewer.world_ui.select_entity("house")
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(viewer.rejected.is_empty(), "Updated GLB payloads validated")
	var commit: String = viewer.current_version
	var source: String = viewer.realm_path.get_base_dir()
	var temporary: String = (
		options
		. get("report", source.path_join("viewer.json"))
		. get_base_dir()
		. path_join("viewer-negative-cases")
	)
	DirAccess.make_dir_recursive_absolute(temporary.path_join("objects"))
	DirAccess.copy_absolute(source.path_join("realm.json"), temporary.path_join("realm.json"))
	DirAccess.copy_absolute(source.path_join("version.json"), temporary.path_join("version.json"))
	var missing: String = viewer.catalog.house.asset.path
	for entity: Dictionary in viewer.manifest.entities:
		if entity.asset.path != missing:
			DirAccess.copy_absolute(
				source.path_join(entity.asset.path), temporary.path_join(entity.asset.path)
			)
	check(viewer.open_realm(temporary), "Complete metadata opens with missing payload")
	viewer.set_pose(viewer.layout.entities.house.focus)
	await drain(viewer)
	check(viewer.rejected.get("house") == "Asset missing", "Missing payload reported")
	var chunk: String = Core.invoke("spatial.key", viewer.catalog.house.owner_chunk).value
	check(viewer.chunk_state(chunk) == "Asset missing", "Missing model is not an empty chunk")
	var key: String = chunk
	var known: Variant = viewer.known_chunks[key]
	viewer.known_chunks.erase(key)
	check(
		viewer.chunk_state(chunk) == "Unknown metadata", "Unknown directory state remains distinct"
	)
	viewer.known_chunks[key] = known
	var catalog: Dictionary = viewer.manifest.duplicate(true)
	catalog.entities[0].label = "Tampered valid label"
	check(
		Core.invoke("manifest.validate", catalog).ok, "Tampered catalog remains structurally valid"
	)
	write_text(temporary.path_join("realm.json"), JSON.stringify(catalog))
	check(not viewer.open_realm(temporary), "Catalog tampering rejected by bridge digest")
	check("catalog hash mismatch" in viewer.last_error, "Failure comes from catalog_sha256 binding")
	check(viewer.current_version == commit, "Tampering cannot replace current version")
	viewer.error_dialog.hide()
	check(viewer.open_realm(options.updated), "Recovered original complete view")
	viewer.world_ui.select_entity("house")
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(viewer.rejected.is_empty(), "Recovery clears missing/rejected state")
	# Layout failures are detected before replacing the active catalog or camera.
	DirAccess.copy_absolute(source.path_join("realm.json"), temporary.path_join("realm.json"))
	write_text(temporary.path_join("unexpected.txt"), "not part of a bundle")
	check(not viewer.open_realm(temporary), "Extra top-level bundle file rejected")
	check(viewer.current_version == commit, "Layout failure preserves active version")
	DirAccess.remove_absolute(temporary.path_join("unexpected.txt"))
	var object_dir := temporary.path_join("objects")
	var moved_objects := temporary.get_base_dir().path_join("objects-symlink-target")
	DirAccess.rename_absolute(object_dir, moved_objects)
	check(
		DirAccess.open(temporary).create_link(moved_objects, object_dir) == OK,
		"Create objects symlink fixture"
	)
	check(not viewer.open_realm(temporary), "Objects symlink rejected before import")
	check("symlink" in viewer.last_error, "Symlink layout failure is explicit")
	DirAccess.remove_absolute(object_dir)
	DirAccess.rename_absolute(moved_objects, object_dir)
	check(viewer.current_version == commit, "Symlink failure preserves active version")
	viewer.error_dialog.hide()
	await check_bundle_paths(viewer, temporary)
	# Coordinate input is in metres and is passed to Rust without UI conversion.
	viewer.world_ui.coordinate_input.text = "10.25,3,-2.5"
	viewer.world_ui.jump_to_input()
	check(
		viewer.camera.position.distance_to(Vector3(10.25, 3, -2.5)) < .0001,
		"Coordinate input uses metres"
	)
	check(viewer.jump_to_chunk("0,0,0"), "Map chunk selection works")
	check(viewer.camera.position == Vector3(16, 16, 16), "Chunk center uses profile coordinates")
	check(not viewer.jump_to_chunk("999,0,0"), "Outside chunk is rejected")
	viewer.options["at"] = "12,3,5"
	check(viewer.open_realm(options.updated), "Explicit CLI-style start opens")
	check(viewer.camera.position == Vector3(12, 3, 5), "--at specifies metre position")
	viewer.options.erase("at")
	viewer.options.erase("no-recent")
	viewer.recent_storage_path = temporary.get_base_dir().path_join(
		"viewer-local-user/positions.json"
	)
	viewer.save_recent()
	var identity: String = viewer.repository_info.realm_id
	check(
		viewer.read_recent().has(identity),
		"Recent position is keyed by realm ID in local user storage"
	)
	viewer.clear_realm()
	check(
		(
			viewer.manifest.is_empty()
			and viewer.repository_info.is_empty()
			and viewer.asset_root.is_empty()
			and viewer.current_version.is_empty()
			and viewer.known_chunks.is_empty()
			and viewer.prototypes.is_empty()
		),
		"clear_realm removes every catalog, version and asset cache"
	)
	check(viewer.world.get_child_count() == 0, "clear_realm detaches all model nodes")
	check(viewer.open_realm(options.updated), "Reopen fixed realm")
	check(viewer.camera.position == Vector3(12, 3, 5), "Recent location restores on reentry")
	viewer.options["no-recent"] = "true"
	DirAccess.copy_absolute(source.path_join("realm.json"), temporary.path_join("realm.json"))
	DirAccess.remove_absolute(temporary.path_join("version.json"))
	check(viewer.open_realm(temporary), "Unbound local preview opens")
	check(
		viewer.repository_info.is_empty() and viewer.current_version.is_empty(),
		"Sidecar-free preview never inherits fixed version"
	)
	check("Unbound" in viewer.world_ui.version_details.text, "Unbound status is visible")
	check(
		not FileAccess.file_exists(source.path_join("positions.json")),
		"Recent location is never written to bundle"
	)
	# A shared immutable GLB is imported once, while each entity gets its own placement.
	var original: Dictionary = viewer.catalog.house
	var copies: Array = []
	for index: int in 2:
		var position: Array = original.position_mm.duplicate()
		position[0] += index * 2000
		var bounds: Dictionary = original.bounds_mm.duplicate(true)
		bounds.min[0] += index * 2000
		bounds.max[0] += index * 2000
		copies.append(
			{
				"id": "house" if index == 0 else "house_copy",
				"position_mm": position,
				"bounds_mm": bounds,
				"asset": original.asset
			}
		)
	var shared := Core.invoke(
		"preview.create", {"profile": viewer.manifest.profile, "entities": copies}
	)
	check(shared.ok, "Shared-model preview built by core")
	write_text(temporary.path_join("realm.json"), JSON.stringify(shared.value))
	DirAccess.copy_absolute(
		source.path_join(original.asset.path), temporary.path_join(original.asset.path)
	)
	check(viewer.open_realm(temporary), "Shared-model preview opens")
	viewer.world_ui.select_entity("house")
	viewer.world_ui.visit_entity()
	check(viewer.queue.size() == 2, "Both placed instances are queued")
	check(viewer.impostors.instances.size() == 2, "All catalog entities have boxes before import")
	check(viewer.impostors.instances.house.visible, "Loading asset keeps its placeholder")
	viewer.navigate_position(Vector3(300, 300, 300))
	check(
		viewer.queue.is_empty() and viewer.cancellations == 2,
		"Moving away cancels obsolete queued imports"
	)
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(
		viewer.loaded.size() == 2 and viewer.asset_imports == 1,
		"Shared GLB imports once for two instances"
	)
	check(viewer.loaded.house != viewer.loaded.house_copy, "Instances have independent scene nodes")
	check(viewer.background_imports == 1, "Shared asset was read and validated on a worker")
	check(not viewer.impostors.instances.house.visible, "Mounted scene hides its box")
	check(not viewer.impostors.instances.house_copy.visible, "Every mounted instance hides its box")
	check(
		is_equal_approx(viewer.loaded.house_copy.position.x - viewer.loaded.house.position.x, 2),
		"Shared model preserves independent placement"
	)
	viewer.navigate_position(Vector3(300, 300, 300))
	check(
		viewer.loaded.is_empty() and viewer.world.get_child_count() == 0 and viewer.unloads == 2,
		"Leaving the radius detaches both instances"
	)
	check(viewer.impostors.instances.house.visible, "Unloaded model restores its box")
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(viewer.asset_imports == 1, "Reentry reuses the immutable prototype")
	check(viewer.set_radii_text("16,64,4000"), "CLI radius syntax is accepted")
	viewer.navigate_position(Vector3(0, 120, 0))
	check(
		viewer.loaded.is_empty() and viewer.queue.is_empty(), "Smaller middle radius unloads models"
	)
	check(viewer.impostors.instances.house.visible, "Distant models are still visible as boxes")
	check(viewer.set_radii(Core.config().viewer.radii), "Default radii restore")
	check(viewer.queue.size() == 2, "Default radius selects a different loading set")
	check(not viewer.set_radii_text("64,16,4000"), "Unordered radii are rejected")
	viewer.world_ui.radius_inputs[0].get_line_edit().text = "16"
	viewer.world_ui.radius_inputs[1].get_line_edit().text = "64"
	viewer.world_ui.apply_radii()
	check(viewer.radii == [16.0, 64.0, 4000.0], "Settings apply the same Rust radius rules")
	viewer.set_radii(Core.config().viewer.radii)
	var tampered := FileAccess.get_file_as_bytes(temporary.path_join(original.asset.path))
	tampered[tampered.size() - 1] ^= 1
	var tampered_file := FileAccess.open(temporary.path_join(original.asset.path), FileAccess.WRITE)
	tampered_file.store_buffer(tampered)
	tampered_file.close()
	check(viewer.open_realm(temporary), "Metadata still opens before streamed bytes are read")
	viewer.world_ui.select_entity("house")
	viewer.world_ui.visit_entity()
	await drain(viewer)
	check(
		viewer.rejected.size() == 2 and viewer.asset_imports == 0,
		"Corrupt bytes rejected before any engine import"
	)
	check(viewer.impostors.instances.house.visible, "Tampered model retains its box")
	check(not viewer.impostors.instances.house.error.is_empty(), "Tampered box shows rejection")
	# A world switch while workers are pending must not install their old scenes.
	check(viewer.open_realm(options.updated), "Open a valid bundle before switching during import")
	viewer.set_pose(viewer.layout.entities.house.focus)
	viewer.pump_streaming()
	check(viewer.loader.import_count() > 0, "An import task is outstanding")
	viewer.clear_realm()
	var deadline := Time.get_ticks_msec() + 30000
	while not viewer.retired_loaders.is_empty() and Time.get_ticks_msec() < deadline:
		viewer.pump_streaming()
		await process_frame
	check(viewer.retired_loaders.is_empty(), "Retired worker results are drained")
	check(
		viewer.loaded.is_empty() and viewer.world.get_child_count() == 0,
		"Old results never mount after clear"
	)
	var sample: Dictionary = (
		Core
		. invoke("json.strict", FileAccess.get_file_as_bytes("res://samples/villa_anahid.json"))
		. value
	)
	check(
		sample.waypoints.size() == 16 and not viewer.manifest.has("waypoints"),
		"House itinerary is separate local sample data"
	)
	for waypoint: Dictionary in sample.waypoints:
		check(
			(
				Core
				. invoke(
					"viewer.pose", {"position": waypoint.position, "look_at": waypoint.look_at}
				)
				. ok
			),
			"Sample waypoint is a valid metre pose: " + waypoint.name
		)
	var report := {
		"checks": checks,
		"errors": errors,
		"status": "passed" if errors.is_empty() else "failed",
		"godot": Engine.get_version_info().string,
		"initial": first,
		"updated": commit,
	}
	if options.has("report"):
		write_text(options.report, JSON.stringify(report, "  ") + "\n")
	print(
		"GIT_VIEW_WORKFLOW_PASS " + str(checks) if errors.is_empty() else "GIT_VIEW_WORKFLOW_FAIL"
	)
	viewer.queue_free()
	await process_frame
	quit(0 if errors.is_empty() else 1)
