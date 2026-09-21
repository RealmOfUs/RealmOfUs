extends SceneTree
var viewer: Node3D


func _initialize() -> void:
	run_capture.call_deferred()


func run_capture() -> void:
	root.size = Vector2i(1920, 1080)
	viewer = load("res://main.tscn").instantiate()
	viewer.options["sample"] = "villa-anahid"
	viewer.options["no-recent"] = "true"
	root.add_child(viewer)
	while not viewer.queue.is_empty():
		await process_frame
	await create_timer(5.0).timeout
	viewer.ui.visible = not viewer.ui.visible
	var stations: Array = [0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--views="):
			stations = []
			for s: String in arg.substr(8).split(","):
				stations.append(int(s))
	var output := ProjectSettings.globalize_path("res://../../build/walsh-v3-2.1")
	for i: int in stations:
		viewer.presentation.visit(i)
		viewer.refresh_assets()
		while not viewer.queue.is_empty():
			await process_frame
		await create_timer(3.5).timeout
		await RenderingServer.frame_post_draw
		root.get_texture().get_image().save_png(output.path_join("view-%02d.png" % i))
		print("CAPTURED_VIEW ", i)
	viewer.ui.visible = not viewer.ui.visible
	viewer.presentation.visit(4)
	viewer.presentation.mini.show()
	await create_timer(1).timeout
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png(output.path_join("room-map.png"))
	quit(1 if not viewer.rejected.is_empty() else 0)
