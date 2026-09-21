## Map drawing and picking. Rust provides the bounded slice and chunk locations.
extends Control
const Core = preload("core.gd")
var viewer: Node3D
var cells: Array[Dictionary] = []
var map: Dictionary = {}
var layer := 0


func refresh() -> void:
	if viewer == null or viewer.manifest.is_empty():
		map.clear()
	else:
		var result := Core.invoke(
			"viewer.map",
			{
				"profile": viewer.manifest.profile,
				"position": Core.array(viewer.camera.position),
				"layer": layer
			}
		)
		map = result.value if result.ok else {}
	queue_redraw()


func _draw() -> void:
	cells.clear()
	if map.is_empty():
		return
	var step: float = minf((size.x - 24) / map.nx, (size.y - 32) / map.nz)
	draw_string(
		ThemeDB.fallback_font,
		Vector2(12, 18),
		"X \u2192   Z \u2193   Layer Y = %d" % map.layer,
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		14,
		Color("a3bbc2")
	)
	for cell: Dictionary in map.cells:
		var rect := Rect2(
			Vector2(12, 28) + Vector2(cell.x, cell.z) * step, Vector2.ONE * (step - 2)
		)
		var state: String = viewer.chunk_state(cell.key)
		var color := Color("263942")
		if state == "Content available":
			color = Color("408f7d")
		elif state in ["Loading assets", "Asset missing", "Asset failed validation"]:
			color = Color("b38c47")
		draw_rect(rect, color)
		if cell.chunk == map.current:
			draw_rect(rect.grow(-2), Color("e1f6e8"), false, 2)
		cells.append({"rect": rect, "cell": cell})


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		for cell: Dictionary in cells:
			if cell.rect.has_point(event.position):
				viewer.jump_to_chunk(cell.cell.key)
				accept_event()
				return
