extends SceneTree


func _initialize() -> void:
	var output := ""
	for argument: String in OS.get_cmdline_user_args():
		if argument.begins_with("--output="):
			output = argument.trim_prefix("--output=")
	if output.is_empty():
		push_error("Pass --output=PATH for the generated license notice")
		quit(1)
		return
	DirAccess.make_dir_recursive_absolute(output.get_base_dir())
	var file := FileAccess.open(output, FileAccess.WRITE)
	if file == null:
		push_error("Cannot write license notice: " + output)
		quit(1)
		return
	file.store_string(
		(
			"Godot Engine\n\n"
			+ Engine.get_license_text()
			+ "\n\n"
			+ JSON.stringify(Engine.get_copyright_info(), "  ")
			+ "\n\n"
			+ JSON.stringify(Engine.get_license_info(), "  ")
			+ "\n"
		)
	)
	quit()
