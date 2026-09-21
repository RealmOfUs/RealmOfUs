extends Control
var viewer: Node3D
var selected_floor := 2
var hit_rooms: Array[Dictionary] = []
var map_scale := 1.0
var map_offset := Vector2.ZERO


func _gui_input(event: InputEvent) -> void:
	if (
		not event is InputEventMouseButton
		or not event.pressed
		or event.button_index != MOUSE_BUTTON_LEFT
	):
		return
	if event.position.y >= 53 and event.position.y <= 91:
		selected_floor = clampi(int((event.position.x - 24) / 158) + 1, 1, 4)
		queue_redraw()
		accept_event()
		return
	for hit: Dictionary in hit_rooms:
		if hit.rect.has_point(event.position):
			viewer.focus_room(hit.room)
			accept_event()
			return


func _draw() -> void:
	if viewer.data.is_empty():
		return
	draw_style_box(viewer.style(Color(.045, .055, .060, .97)), Rect2(Vector2.ZERO, size))
	var font := ThemeDB.fallback_font
	draw_string(
		font, Vector2(24, 32), "ROOMS BY FLOOR", HORIZONTAL_ALIGNMENT_LEFT, -1, 17, Color("eadcc4")
	)
	for i: int in 4:
		var r := Rect2(24 + i * 158, 53, 148, 36)
		draw_style_box(
			viewer.style(Color("776c57") if selected_floor == i + 1 else Color("293436")), r
		)
		draw_string(
			font,
			r.position + Vector2(12, 24),
			["L1  Welcome", "L2  Main living", "L3  Private", "Detached ADU"][i],
			HORIZONTAL_ALIGNMENT_LEFT,
			135,
			13,
			Color("f3e7d1")
		)
	var listings: Array = viewer.data.get("listing", {}).get("rooms", [])
	var rooms: Array[Dictionary] = []
	for r: Dictionary in listings:
		var floor_number: int = 4 if r.floor is String and r.floor == "ADU" else int(r.floor)
		if floor_number == selected_floor:
			rooms.append(r)
	if rooms.is_empty():
		draw_string(
			font,
			Vector2(26, 140),
			"Room plans are available in the v3 house study.",
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			15,
			Color.WHITE
		)
		return
	var bounds := Rect2(
		Vector2(rooms[0].rect[0], rooms[0].rect[1]), Vector2(rooms[0].rect[2], rooms[0].rect[3])
	)
	for r: Dictionary in rooms:
		bounds = bounds.merge(Rect2(Vector2(r.rect[0], r.rect[1]), Vector2(r.rect[2], r.rect[3])))
	map_scale = minf(610 / bounds.size.x, 405 / bounds.size.y)
	map_offset = Vector2(341, 320) - bounds.get_center() * map_scale
	hit_rooms.clear()
	for r: Dictionary in rooms:
		var rect := (
			Rect2(
				map_offset + Vector2(r.rect[0], r.rect[1]) * map_scale,
				Vector2(r.rect[2], r.rect[3]) * map_scale
			)
			. grow(-3)
		)
		var col := Color("777361")
		if r.kind == "bedroom":
			col = Color("72615b")
		elif r.kind == "kitchen":
			col = Color("688078")
		elif r.kind == "bath":
			col = Color("667b88")
		draw_rect(rect, col)
		draw_rect(rect, Color("c1baa8"), false, 1)
		var fs := 12 if rect.size.x < 130 else 14
		var lines: Array[String] = []
		var line: String = ""
		for word: String in str(r.name).split(" "):
			var candidate := word if line.is_empty() else line + " " + word
			if (
				(
					font.get_string_size(candidate, HORIZONTAL_ALIGNMENT_LEFT, -1, fs).x
					> rect.size.x - 14
				)
				and not line.is_empty()
			):
				lines.append(line)
				line = word
			else:
				line = candidate
		if not line.is_empty():
			lines.append(line)
		for j: int in mini(lines.size(), 3):
			draw_string(
				font,
				rect.position + Vector2(8, 22 + j * 16),
				lines[j],
				HORIZONTAL_ALIGNMENT_LEFT,
				rect.size.x - 14,
				fs,
				Color("fff4df")
			)
		if r.kind == "bedroom":
			var b := Rect2(rect.get_center() - Vector2(15, 4), Vector2(30, 35))
			draw_rect(b, Color("e1d8c7"))
			draw_line(b.position + Vector2(0, 9), b.position + Vector2(30, 9), col, 2)
		elif r.kind == "kitchen" and r.id == "kitchen":
			for yy: float in [-12, 14]:
				draw_rect(
					Rect2(rect.get_center() + Vector2(-29, yy), Vector2(58, 15)), Color("d8ddd2")
				)
		hit_rooms.append({"rect": rect, "room": r})
	var cam := map_offset + Vector2(viewer.camera.position.x, viewer.camera.position.z) * map_scale
	if Rect2(20, 98, 642, 450).has_point(cam):
		draw_circle(cam, 5, Color("ffdf97"))
		var d := Vector2(-viewer.camera.basis.z.x, -viewer.camera.basis.z.z).normalized()
		draw_line(cam, cam + d * 17, Color("ffdf97"), 2, true)
	draw_string(
		font,
		Vector2(24, 566),
		"CLICK A ROOM TO VISIT  \u00b7  Positions and dimensions are approximate",
		HORIZONTAL_ALIGNMENT_LEFT,
		640,
		12,
		Color("c5c4bb")
	)
	draw_string(
		font,
		Vector2(24, 590),
		"MAIN HOUSE: 1 BED ON L2 + 4 ON L3   /   SEPARATE ADU: 2 BEDS",
		HORIZONTAL_ALIGNMENT_LEFT,
		640,
		12,
		Color("b5ae9e")
	)
