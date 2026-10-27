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


const INK := Color("#121110")
const UMBER := Color("#2A1F17")
const OCHRE := Color("#B9853F")
const GOLD := Color("#D9A845")
const TEXT := Color("#EDE6DA")
const MUTED := Color("#A89A86")

var sim: Sim
var terrain: HexTerrain
var settlement_layer: Node2D
var selection_ring: Line2D
var ui: CanvasLayer
var color_texture: ImageTexture
var color_image: Image
var code_texture: ImageTexture
var code_image: Image
var camera: Camera2D

var grid_w := 0
var grid_h := 0
var chosen := -1

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

var zoom := 1.6
var pressing := false
var dragging := false
var press_at := Vector2.ZERO
var drag_origin := Vector2.ZERO
var _last_overlay := -1
var _last_sites := PackedInt32Array()
var _shots: Array[Dictionary] = []
var _shot_timer := 0.0


func _ready() -> void:
	sim = Sim.create(1)
	add_child(sim)
	grid_w = sim.grid_width()
	grid_h = sim.grid_height()
	var years := 100
	var args := OS.get_cmdline_user_args()
	var years_at := args.find("--years")
	if years_at >= 0 and years_at + 1 < args.size():
		years = int(args[years_at + 1])
	sim.step(years)
	chosen = sim.chosen_culture()

	ui = CanvasLayer.new()
	add_child(ui)
	_build_terrain()
	_build_camera()
	_build_hud()
	_build_scripture()
	_build_toolbar()
	_build_selection_panel()
	var grid_text := "%d x %d" % [grid_w, grid_h]
	print("[godot] sim ready: year %d, %s tiles" % [sim.tick(), grid_text])
	repaint_if_dirty()
	_refresh_scripture()
	if args.has("--shots"):
		_queue_shots()


func _build_terrain() -> void:
	color_image = Image.create(grid_w, grid_h, false, Image.FORMAT_RGBA8)
	color_texture = ImageTexture.create_from_image(color_image)
	code_image = Image.create(grid_w, grid_h, false, Image.FORMAT_RGBA8)
	code_texture = ImageTexture.create_from_image(code_image)
	terrain = HexTerrain.new()
	terrain.setup(grid_w, grid_h, color_texture, code_texture)
	terrain.set_chosen(chosen)
	add_child(terrain)
	terrain.update_heights(sim.tile_heights(), sim.tile_biomes())

	# Buildings stand on the terrain; y-sorting puts nearer settlements in front.
	settlement_layer = Node2D.new()
	settlement_layer.y_sort_enabled = true
	add_child(settlement_layer)

	selection_ring = Line2D.new()
	selection_ring.width = 1.6
	selection_ring.default_color = GOLD
	selection_ring.visible = false
	add_child(selection_ring)


# Rebuilds the settlement objects when any settlement was founded, grew a
# tier, changed hands or finished a building.
func _refresh_settlements() -> void:
	var sites: PackedInt32Array = sim.settlement_sites()
	if sites == _last_sites:
		return
	_last_sites = sites
	for child in settlement_layer.get_children():
		child.queue_free()
	var biomes: PackedByteArray = sim.tile_biomes()
	for s in range(0, sites.size(), 4):
		var tile: int = sites[s]
		var culture: int = sites[s + 2]
		var view := SettlementView.new()
		view.position = terrain.top(tile % grid_w, tile / grid_w)
		view.configure(
			tile,
			sites[s + 1],
			biomes[tile],
			sim.culture_dress(culture),
			culture == chosen,
			sim.culture_doctrine(culture) != "",
			sites[s + 3]
		)
		settlement_layer.add_child(view)
	_scale_settlements()


func _build_camera() -> void:
	camera = Camera2D.new()
	camera.position_smoothing_enabled = true
	camera.position_smoothing_speed = 10.0
	add_child(camera)
	_set_zoom(zoom)
	camera.position = _chosen_center()
	camera.reset_smoothing()


# The middle of the Chosen's land, or the map's middle before they exist.
func _chosen_center() -> Vector2:
	var cultures: PackedInt32Array = sim.tile_cultures()
	var sum := Vector2.ZERO
	var count := 0
	for i in cultures.size():
		if cultures[i] == chosen:
			sum += Vector2(i % grid_w, i / grid_w)
			count += 1
	var tile := Vector2i(grid_w / 2, grid_h / 2)
	if count > 0:
		tile = Vector2i(sum / float(count))
	return terrain.top(tile.x, tile.y)


func _build_hud() -> void:
	hud = Label.new()
	hud.set_anchors_preset(Control.PRESET_TOP_LEFT)
	hud.position = Vector2(12, 8)
	hud.add_theme_color_override("font_color", TEXT)
	hud.add_theme_color_override("font_shadow_color", INK)
	hud.add_theme_font_size_override("font_size", 14)
	ui.add_child(hud)


func _build_scripture() -> void:
	var panel := Panel.new()
	panel.set_anchors_preset(Control.PRESET_RIGHT_WIDE)
	panel.offset_left = -SCRIPTURE_WIDTH
	panel.offset_right = 0
	panel.add_theme_stylebox_override("panel", _panel_style(UMBER))
	ui.add_child(panel)

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

	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	scripture_list = VBoxContainer.new()
	scripture_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scripture_list.custom_minimum_size.x = SCRIPTURE_WIDTH - 40.0
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
	ui.add_child(bar)

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
	ui.add_child(selection_panel)
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
		var years := int(year_accumulator)
		if years > 0:
			year_accumulator -= float(years)
			sim.step(years)
			repaint_if_dirty()
			_refresh_scripture()
	_update_hud()
	if not _shots.is_empty():
		_advance_shots(delta)

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
		camera.position += move.normalized() * 600.0 * delta / zoom


# --shots: a fixed tour for milestone screenshots, then quit.
func _queue_shots() -> void:
	paused = true
	var center := _chosen_center()
	var sites: PackedInt32Array = sim.settlement_sites()
	var town := center
	var best := -1
	for s in range(0, sites.size(), 4):
		if sites[s + 2] == chosen and sites[s + 1] > best:
			best = sites[s + 1]
			town = terrain.top(sites[s] % grid_w, sites[s] / grid_w)
	# The highest summit, for the mountain close-up.
	var heights: PackedFloat32Array = sim.tile_heights()
	var summit := 0
	for i in heights.size():
		if heights[i] > heights[summit]:
			summit = i
	_shots = [
		{"name": "hex-map-05x", "zoom": 0.5, "at": center},
		{"name": "hex-mountains-3x", "zoom": 3.0, "at": terrain.top(summit % grid_w, summit / grid_w)},
		{"name": "hex-region-15x", "zoom": 1.5, "at": center},
		{"name": "hex-town-4x", "zoom": 4.0, "at": town},
	]
	_begin_shot()


func _begin_shot() -> void:
	var shot: Dictionary = _shots[0]
	_set_zoom(shot["zoom"])
	camera.position = shot["at"]
	camera.reset_smoothing()
	_shot_timer = 0.6


func _advance_shots(delta: float) -> void:
	_shot_timer -= delta
	if _shot_timer > 0.0:
		return
	var shot: Dictionary = _shots.pop_front()
	var path := ProjectSettings.globalize_path("res://../docs/designs/%s.png" % shot["name"])
	get_viewport().get_texture().get_image().save_png(path)
	print("[shot] saved %s" % path)
	if _shots.is_empty():
		get_tree().quit()
	else:
		_begin_shot()


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
		elif event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				pressing = true
				dragging = false
				press_at = event.position
				drag_origin = camera.position
			else:
				# A press that never moved is a click on the map.
				if pressing and not dragging:
					_on_map_click(event.position)
				pressing = false
				dragging = false
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			_select_power(-1)
			_clear_selection()
	elif event is InputEventMouseMotion and pressing:
		if not dragging and event.position.distance_to(press_at) > 4.0:
			dragging = true
		if dragging:
			camera.position = drag_origin - (event.position - press_at) / zoom
			camera.reset_smoothing()


func _set_zoom(z: float) -> void:
	zoom = clampf(z, 0.4, 5.0)
	camera.zoom = Vector2(zoom, zoom)
	_scale_settlements()


# Zoomed out, buildings grow so a town still reads against a camp at 0.5x
# (art bible, readability gate 1); zoomed in they keep their true size.
func _scale_settlements() -> void:
	var grow := clampf(1.1 / zoom, 1.0, 2.4)
	for view in settlement_layer.get_children():
		view.scale = Vector2(grow, grow)


func _on_map_click(screen_point: Vector2) -> void:
	var tile := terrain.tile_at(terrain.get_global_mouse_position())
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
	selection_ring.visible = false
	for view in settlement_layer.get_children():
		if view.selected:
			view.selected = false
			view.queue_redraw()


func _select_tile(tile: Vector2i) -> void:
	_clear_selection()
	selected_tile = tile
	selection_ring.points = terrain.hex_outline(tile.x, tile.y)
	selection_ring.visible = true
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


func repaint_if_dirty() -> void:
	if sim.revision() == last_revision and overlay_index == _last_overlay:
		return
	last_revision = sim.revision()
	_last_overlay = overlay_index
	var bytes: PackedByteArray = sim.paint_overlay(OVERLAYS[overlay_index])
	color_image.set_data(grid_w, grid_h, false, Image.FORMAT_RGBA8, bytes)
	color_texture.update(color_image)
	code_image.set_data(grid_w, grid_h, false, Image.FORMAT_RGBA8, sim.tile_codes())
	code_texture.update(code_image)
	# Geometry only rebuilds when a power moved the ground; the terrain
	# compares heights itself.
	terrain.update_heights(sim.tile_heights(), sim.tile_biomes())
	_refresh_settlements()


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
