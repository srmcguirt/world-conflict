extends Node

const OVERLAY_BIOME := 0
const OVERLAY_CULTURE := 1
const OVERLAY_TRAITS := 2
const OVERLAY_BELIEF := 3
const OVERLAY_LANGUAGE := 5
const OVERLAY_POLITY := 4
const OVERLAYS := [
	OVERLAY_BIOME, OVERLAY_CULTURE, OVERLAY_TRAITS, OVERLAY_BELIEF, OVERLAY_LANGUAGE, OVERLAY_POLITY
]

const SPEEDS := [1, 5, 25, 100]
const TICK_BUDGET_MS := 8.0
const POWERS := ["Raise", "Lower", "Paint", "Bless", "Meteor", "Tornado", "Spawn"]
const SCRIPTURE_WIDTH := 360.0

const TILE_W := 34.0
const TILE_H := 17.0
const HEIGHT_SCALE := 26.0

const INK := Color("#121110")
const UMBER := Color("#2A1F17")
const OCHRE := Color("#B9853F")
const GOLD := Color("#D9A845")
const TEXT := Color("#EDE6DA")
const MUTED := Color("#A89A86")

var sim: Sim
var mesh_instance: MeshInstance2D
var terrain_material: ShaderMaterial
var color_texture: ImageTexture
var color_image: Image
var camera: Camera2D

var grid_w := 0
var grid_h := 0
var chosen := -1
var heights: PackedFloat32Array
var biomes: PackedByteArray
var cultures: PackedInt32Array

var hud: Label
var scripture_list: VBoxContainer
var scripture_header: Label
var portrait_view: TextureRect
var last_revision := -1
var last_verse := ""
var speed_index := 0
var paused := false
var year_accumulator := 0.0
var overlay_index := 0
var selected_power := -1
var selected_tile := Vector2i(-1, -1)
var selection_panel: Panel
var selection_label: Label

var zoom := 1.0
var dragging := false
var drag_origin := Vector2.ZERO
var _last_overlay := -1
var _shot_pending := false
var _shot_timer := 0.0


func _ready() -> void:
	sim = Sim.create(1)
	add_child(sim)
	grid_w = sim.grid_width()
	grid_h = sim.grid_height()
	sim.step(100)
	chosen = sim.chosen_culture()

	heights = sim.tile_heights()
	biomes = sim.tile_biomes()
	cultures = sim.tile_cultures()

	_build_terrain()
	_build_settlements()
	_build_camera()
	_build_hud()
	_build_scripture()
	_build_toolbar()
	_build_selection_panel()
	var grid_text := "%d x %d" % [grid_w, grid_h]
	print("[godot] sim ready: year %d, %s tiles" % [sim.tick(), grid_text])
	repaint_if_dirty()
	_refresh_scripture()
	if OS.get_cmdline_user_args().has("--shots"):
		_shot_pending = true
		_shot_timer = 2.0


func _build_terrain() -> void:
	terrain_material = ShaderMaterial.new()
	terrain_material.shader = load("res://terrain.gdshader")
	terrain_material.set_shader_parameter("beacon_radius", 14.0 * TILE_W)
	terrain_material.set_shader_parameter("beacon_pos", Vector2.ZERO)
	terrain_material.set_shader_parameter("night", 0.0)

	# Color texture: updated on revision change from paint_overlay bytes.
	color_image = Image.create(grid_w, grid_h, false, Image.FORMAT_RGBA8)
	color_texture = ImageTexture.create_from_image(color_image)

	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)

	for y in grid_h:
		for x in grid_w:
			var i: int = y * grid_w + x
			var h: float = heights[i]
			var water: int = 1 if biomes[i] == 0 or biomes[i] == 1 else 0
			var is_chosen: int = 1 if cultures[i] == chosen else 0
			var hashv: float = float((x * 73 + y * 151) % 97) / 97.0

			# Height AO.
			var ao := 0.0
			var n := 0
			var nb_sum := 0.0
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= 0 and ny >= 0 and nx < grid_w and ny < grid_h:
					nb_sum += heights[ny * grid_w + nx]
					n += 1
			if n > 0:
				ao = clampf((nb_sum / float(n) - h) * 0.35, 0.0, 0.25)

			# Rim: territory edge or coast.
			var rim := 0.0
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx < 0 or ny < 0 or nx >= grid_w or ny >= grid_h:
					continue
				var ni: int = ny * grid_w + nx
				if cultures[ni] != cultures[i] and (cultures[i] != -1 or cultures[ni] != -1):
					rim = maxf(rim, 0.5 if cultures[i] == chosen else 0.3)
				if biomes[ni] == 0 and water == 0:
					rim = maxf(rim, 0.35)
				if biomes[ni] != 0 and water == 1:
					rim = maxf(rim, 0.35)

			var normal := Vector3(0.0, 0.0, 1.0)
			# Analytic normal from height gradient.
			var gx: float = heights[mini(i + 1, heights.size() - 1)] - heights[maxi(i - 1, 0)]
			var gy: float = (
				heights[mini(i + grid_w, heights.size() - 1)] - heights[maxi(i - grid_w, 0)]
			)
			normal = Vector3(-gx * 3.0, -gy * 3.0, 1.0).normalized()

			var custom := Color(float(water + is_chosen * 2), ao, rim, hashv)
			var uv := Vector2(float(x) / float(grid_w), float(y) / float(grid_h))
			_quad(st, iso(x, y, h), normal, uv, custom)

			# Cliff walls toward higher neighbors.
			for d in [Vector2i(0, 1), Vector2i(1, 0)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= grid_w or ny >= grid_h:
					continue
				var nh: float = heights[ny * grid_w + nx]
				if nh > h + 0.02 and water == 0:
					var a := iso(x, y, h)
					var b := iso(x + d.x, y + d.y, h)
					var c := iso(x + d.x, y + d.y, nh)
					var dd := iso(x, y, nh)
					var wall_normal := Vector3(float(-d.y), float(-d.x), 0.4).normalized()
					st.set_color(Color(float(water + is_chosen * 2), ao, rim, hashv))
					st.set_normal(wall_normal)
					st.set_uv(uv)
					_tri(st, a, b, c)
					_tri(st, a, c, dd)

	var array_mesh := st.commit()
	# Assign the color texture to the mesh's material.
	terrain_material.set_shader_parameter("color_tex", color_texture)
	mesh_instance = MeshInstance2D.new()
	mesh_instance.mesh = array_mesh
	mesh_instance.material = terrain_material
	add_child(mesh_instance)


func iso(x: int, y: int, h: float) -> Vector2:
	var sx := float(x - y) * TILE_W / 2.0
	var sy := float(x + y) * TILE_H / 2.0
	if y % 2 == 1:
		sx += TILE_W / 4.0
	return Vector2(sx, sy - h * HEIGHT_SCALE)


func _quad(st: SurfaceTool, center: Vector2, normal: Vector3, uv: Vector2, custom: Color) -> void:
	var hw := TILE_W / 2.0
	var hh := TILE_H / 2.0
	var pts := [
		Vector2(center.x, center.y - hh),
		Vector2(center.x + hw, center.y),
		Vector2(center.x, center.y + hh),
		Vector2(center.x - hw, center.y)
	]
	st.set_color(custom)
	st.set_normal(normal)
	st.set_uv(uv)
	for k in [0, 1, 2, 0, 2, 3]:
		st.add_vertex(Vector3(pts[k].x, pts[k].y, 0.0))


func _tri(st: SurfaceTool, a: Vector2, b: Vector2, c: Vector2) -> void:
	st.add_vertex(Vector3(a.x, a.y, 0.0))
	st.add_vertex(Vector3(b.x, b.y, 0.0))
	st.add_vertex(Vector3(c.x, c.y, 0.0))


func _build_settlements() -> void:
	var container := Node2D.new()
	add_child(container)
	for y in grid_h:
		for x in grid_w:
			var sid: int = sim.settlement_at_tile(x, y)
			if sid < 0:
				continue
			var h: float = heights[y * grid_w + x]
			var pos := iso(x, y, h)
			var marker := ColorRect.new()
			marker.size = Vector2(6, 6)
			marker.position = pos - Vector2(3, 3)
			if cultures[y * grid_w + x] == chosen:
				marker.color = GOLD
			else:
				marker.color = OCHRE
			container.add_child(marker)


func _build_camera() -> void:
	camera = Camera2D.new()
	camera.zoom = Vector2(zoom, zoom)
	add_child(camera)
	var sx := 0.0
	var sy := 0.0
	var count := 0
	for i in cultures.size():
		if cultures[i] == chosen:
			sx += float(i % grid_w)
			sy += float(i / grid_w)
			count += 1
	if count > 0:
		camera.position = iso(int(sx / float(count)), int(sy / float(count)), 0.5)


func _build_hud() -> void:
	hud = Label.new()
	hud.set_anchors_preset(Control.PRESET_TOP_LEFT)
	hud.position = Vector2(12, 8)
	hud.add_theme_color_override("font_color", TEXT)
	hud.add_theme_color_override("font_shadow_color", INK)
	hud.add_theme_font_size_override("font_size", 14)
	add_child(hud)


func _build_scripture() -> void:
	var panel := Panel.new()
	panel.set_anchors_preset(Control.PRESET_RIGHT_WIDE)
	panel.offset_left = -SCRIPTURE_WIDTH
	panel.offset_right = 0
	panel.add_theme_stylebox_override("panel", _panel_style(UMBER))
	add_child(panel)

	var column := VBoxContainer.new()
	column.set_anchors_preset(Control.PRESET_FULL_RECT)
	column.offset_left = 16
	column.offset_top = 12
	column.offset_right = -16
	column.offset_bottom = -12
	column.add_theme_constant_override("separation", 12)
	panel.add_child(column)

	scripture_header = Label.new()
	scripture_header.add_theme_color_override("font_color", GOLD)
	scripture_header.add_theme_font_size_override("font_size", 16)
	column.add_child(scripture_header)

	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	column.add_child(scroll)

	scripture_list = VBoxContainer.new()
	scroll.add_child(scripture_list)

	portrait_view = TextureRect.new()
	portrait_view.custom_minimum_size = Vector2(48, 64)
	portrait_view.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	portrait_view.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	column.add_child(portrait_view)


func _build_toolbar() -> void:
	var bar := HBoxContainer.new()
	bar.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
	bar.position = Vector2(12, -44)
	bar.add_theme_constant_override("separation", 4)
	add_child(bar)

	for index in POWERS.size():
		var button := Button.new()
		button.text = "%d %s" % [index + 1, POWERS[index]]
		button.focus_mode = Control.FOCUS_NONE
		button.add_theme_color_override("font_color", TEXT)
		button.pressed.connect(_select_power.bind(index))
		bar.add_child(button)


func _build_selection_panel() -> void:
	selection_panel = Panel.new()
	selection_panel.visible = false
	selection_panel.position = Vector2(12, 40)
	selection_panel.custom_minimum_size = Vector2(220, 80)
	add_child(selection_panel)
	selection_label = Label.new()
	selection_label.position = Vector2(10, 8)
	selection_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	selection_label.add_theme_color_override("font_color", TEXT)
	selection_label.add_theme_font_size_override("font_size", 12)
	selection_panel.add_child(selection_label)


func _panel_style(color: Color) -> StyleBoxFlat:
	var style := StyleBoxFlat.new()
	style.bg_color = color
	style.border_color = OCHRE
	style.border_width_left = 1
	return style


func _process(delta: float) -> void:
	if not paused:
		year_accumulator += delta * float(SPEEDS[speed_index])
		while year_accumulator >= 1.0:
			year_accumulator -= 1.0
			sim.step(1)
			repaint_if_dirty()
			_refresh_scripture()
	_update_hud()
	if _shot_pending:
		_shot_timer -= delta
		if _shot_timer <= 0.0:
			_shot_pending = false
			var img := get_viewport().get_texture().get_image()
			var path := ProjectSettings.globalize_path("res://../docs/designs/game-iso.png")
			img.save_png(path)
			print("[shot] saved %s" % path)
			get_tree().quit()

	# Camera movement.
	var move := Vector2.ZERO
	if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):
		move.x -= 1.0
	if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT):
		move.x += 1.0
	if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):
		move.y -= 1.0
	if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):
		move.y += 1.0
	if move != Vector2.ZERO:
		camera.position += move.normalized() * 400.0 * delta / zoom


func _update_hud() -> void:
	var power: String = "none" if selected_power < 0 else POWERS[selected_power]
	hud.text = (
		"year %d · %s · %d years/s · %s · peoples %d · power %s"
		% [
			sim.tick(),
			"paused" if paused else "running",
			SPEEDS[speed_index],
			_overlay_name(),
			sim.culture_count(),
			power,
		]
	)


func _overlay_name() -> String:
	match OVERLAYS[overlay_index]:
		OVERLAY_CULTURE:
			return "culture"
		OVERLAY_TRAITS:
			return "traits"
		OVERLAY_BELIEF:
			return "belief"
		OVERLAY_LANGUAGE:
			return "language"
		OVERLAY_POLITY:
			return "polity"
		_:
			return "biome"


func _input(event: InputEvent) -> void:
	if not (event is InputEventKey and event.pressed and not event.echo):
		return
	match event.keycode:
		KEY_SPACE:
			paused = not paused
		KEY_BRACKETLEFT:
			speed_index = maxi(speed_index - 1, 0)
		KEY_BRACKETRIGHT:
			speed_index = mini(speed_index + 1, SPEEDS.size() - 1)
		KEY_M:
			overlay_index = (overlay_index + 1) % OVERLAYS.size()
			repaint_if_dirty()
		KEY_ESCAPE:
			_select_power(-1)
			_clear_selection()


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_set_zoom(zoom * 1.12)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_set_zoom(zoom / 1.12)
		elif event.button_index == MOUSE_BUTTON_LEFT and event.pressed:
			dragging = true
			drag_origin = camera.position - event.position
		elif not event.pressed:
			dragging = false
	elif event is InputEventMouseMotion and dragging:
		camera.position = drag_origin + event.position


func _set_zoom(z: float) -> void:
	zoom = clampf(z, 0.5, 4.0)
	camera.zoom = Vector2(zoom, zoom)


func _on_map_input(event: InputEvent) -> void:
	if not (event is InputEventMouseButton and event.pressed):
		return
	if event.button_index == MOUSE_BUTTON_RIGHT:
		_select_power(-1)
		_clear_selection()
		return
	var tile := _tile_at(event.position)
	if tile.x < 0:
		return
	if selected_power >= 0:
		sim.cast_power(selected_power, tile.x, tile.y)
		if paused:
			sim.step(1)
		repaint_if_dirty()
		return
	_select_tile(tile)


func _select_power(index: int) -> void:
	selected_power = index
	_clear_selection()


func _clear_selection() -> void:
	selected_tile = Vector2i(-1, -1)
	selection_panel.visible = false


func _select_tile(tile: Vector2i) -> void:
	selected_tile = tile
	var sid: int = sim.settlement_at_tile(tile.x, tile.y)
	if sid >= 0:
		selection_label.text = "settlement %d\ntile %d, %d" % [sid, tile.x, tile.y]
		selection_panel.visible = true
		return
	var pid: int = sim.polity_at_tile(tile.x, tile.y)
	if pid >= 0:
		selection_label.text = "polity %d\ntile %d, %d" % [pid, tile.x, tile.y]
		selection_panel.visible = true
		return
	selection_panel.visible = false


func _tile_at(point: Vector2) -> Vector2i:
	var size := get_viewport().get_visible_rect().size
	var grid := Vector2(grid_w, grid_h)
	var scale := minf(size.x / grid.x, size.y / grid.y)
	var origin := (size - grid * scale) * 0.5
	var tile := Vector2i((point - origin) / scale)
	if tile.x < 0 or tile.y < 0 or tile.x >= grid_w or tile.y >= grid_h:
		return Vector2i(-1, -1)
	return tile
	return tile


func repaint_if_dirty() -> void:
	if sim.revision() == last_revision and overlay_index == _last_overlay:
		return
	last_revision = sim.revision()
	_last_overlay = overlay_index
	# Rebuild the color texture from paint_overlay bytes.
	var bytes: PackedByteArray = sim.paint_overlay(OVERLAYS[overlay_index])
	color_image.set_data(grid_w, grid_h, false, Image.FORMAT_RGBA8, bytes)
	color_texture.update(color_image)
	# The mesh geometry is fixed (heights don't change except on Raise/Lower,
	# which apply at tick boundaries and bump revision). Rebuild geometry only
	# when needed — for M6a, rebuild on every revision is acceptable (12.8k
	# tiles, ~77k verts, well within the frame budget).


func _refresh_scripture() -> void:
	var chosen := sim.chosen_culture()
	if chosen < 0:
		return
	_show_newest_portrait()
	var doctrine := sim.culture_doctrine(chosen)
	var lineage: PackedStringArray = sim.tongue_lineage(chosen)
	var tongue_text := ""
	var own: String = lineage[lineage.size() - 1] if lineage.size() > 0 else ""
	if lineage.size() == 1:
		tongue_text = " · speaks %s" % own
	elif lineage.size() > 1:
		tongue_text = " · speaks %s of the %s line" % [own, lineage[0]]
	scripture_header.text = (
		"%s · %s · piety %.2f%s"
		% [
			sim.culture_name(chosen),
			doctrine if doctrine != "" else "no doctrine",
			sim.culture_piety(chosen),
			tongue_text,
		]
	)

	var verses: PackedStringArray = sim.scripture(chosen, 0, 12)
	if verses.size() > 0 and verses[0] == last_verse:
		return
	last_verse = verses[0] if verses.size() > 0 else ""

	for child in scripture_list.get_children():
		child.queue_free()
	for verse in verses:
		var label := Label.new()
		label.text = verse
		label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		label.add_theme_color_override("font_color", TEXT)
		label.add_theme_font_size_override("font_size", 16)
		scripture_list.add_child(label)

	var witness := sim.newest_witness()
	if witness != "":
		var byline := Label.new()
		byline.text = "witness: %s" % witness
		byline.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		byline.add_theme_color_override("font_color", MUTED)
		byline.add_theme_font_size_override("font_size", 12)
		scripture_list.add_child(byline)


func _show_newest_portrait() -> void:
	var id := sim.newest_witness_id()
	if id < 0:
		portrait_view.texture = null
		return
	var bytes: PackedByteArray = sim.portrait(id)
	if bytes.size() == 0:
		portrait_view.texture = null
		return
	var img := Image.create(sim.portrait_width(), sim.portrait_height(), false, Image.FORMAT_RGBA8)
	img.set_data(sim.portrait_width(), sim.portrait_height(), false, Image.FORMAT_RGBA8, bytes)
	portrait_view.texture = ImageTexture.create_from_image(img)
