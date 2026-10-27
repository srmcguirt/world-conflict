extends Node

## Milestone 2 shell: map, speed control (R6 repaint), power toolbar (D10), and
## the scripture column that shows what the witnesses wrote.

const OVERLAY_BIOME := 0
const OVERLAY_CULTURE := 1
const OVERLAY_BELIEF := 3
const OVERLAYS := [OVERLAY_BIOME, OVERLAY_CULTURE, OVERLAY_BELIEF]
## Years per second at each speed step (design: 1x = 1 year per second).
const SPEEDS := [1, 5, 25, 100]
## Sim time budget per frame, in milliseconds (design, Frame and tick scheduling).
const TICK_BUDGET_MS := 8.0

## Toolbar slots. The design's six powers count Raise and Lower as one, so
## Spawn takes the seventh slot and the seventh key.
const POWERS := ["Raise", "Lower", "Paint", "Bless", "Meteor", "Tornado", "Spawn"]

## D7 tokens (design, Visual direction).
const INK := Color("#121110")
const UMBER := Color("#2A1F17")
const OCHRE := Color("#B9853F")
const GOLD := Color("#D9A845")
const TEXT := Color("#EDE6DA")
const MUTED := Color("#A89A86")
const SCRIPTURE_WIDTH := 360.0

var sim: Sim
var image: Image
var texture: ImageTexture
var map: TextureRect
var hud: Label
var scripture_list: VBoxContainer
var scripture_header: Label
var last_revision := -1
var last_verse := ""
var speed_index := 0
var paused := false
var year_accumulator := 0.0
var overlay_index := 0
var selected_power := -1


func _ready() -> void:
	sim = Sim.create(1)
	add_child(sim)
	image = Image.create(sim.grid_width(), sim.grid_height(), false, Image.FORMAT_RGBA8)
	texture = ImageTexture.create_from_image(image)
	_build_map()
	_build_hud()
	_build_scripture()
	_build_toolbar()
	var grid_text := "%d x %d" % [sim.grid_width(), sim.grid_height()]
	print("[godot] sim ready: year %d, %s tiles" % [sim.tick(), grid_text])
	repaint()
	_refresh_scripture()


func _build_map() -> void:
	map = TextureRect.new()
	map.texture = texture
	map.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	map.set_anchors_preset(Control.PRESET_FULL_RECT)
	map.offset_right = -SCRIPTURE_WIDTH
	map.mouse_filter = Control.MOUSE_FILTER_STOP
	map.gui_input.connect(_on_map_input)
	add_child(map)


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
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	column.add_child(scroll)

	scripture_list = VBoxContainer.new()
	scripture_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scripture_list.add_theme_constant_override("separation", 8)
	scroll.add_child(scripture_list)


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


func _panel_style(color: Color) -> StyleBoxFlat:
	var style := StyleBoxFlat.new()
	style.bg_color = color
	style.border_color = OCHRE
	style.border_width_left = 1
	return style


func _process(delta: float) -> void:
	if not paused:
		year_accumulator += delta * float(SPEEDS[speed_index])
	var started_ms := Time.get_ticks_msec()
	while year_accumulator >= 1.0:
		sim.step(1)
		year_accumulator -= 1.0
		if float(Time.get_ticks_msec() - started_ms) >= TICK_BUDGET_MS:
			break
	repaint_if_dirty()
	_update_hud()


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
		OVERLAY_BELIEF:
			return "belief"
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
			repaint()
		KEY_ESCAPE:
			_select_power(-1)


func _select_power(index: int) -> void:
	selected_power = index


## Casts the selected power on the tile under the mouse.
func _on_map_input(event: InputEvent) -> void:
	if not (event is InputEventMouseButton and event.pressed):
		return
	if event.button_index == MOUSE_BUTTON_RIGHT:
		_select_power(-1)
		return
	if event.button_index != MOUSE_BUTTON_LEFT or selected_power < 0:
		return
	var tile := _tile_at(event.position)
	if tile.x < 0:
		return
	sim.cast_power(selected_power, tile.x, tile.y)
	if paused:
		# A cast applies on the next tick, so a paused world steps one year to
		# let the player see what they just did.
		sim.step(1)
	repaint_if_dirty()
	_refresh_scripture()


## Maps a point in the map control to a tile, or (-1, -1) outside the image.
func _tile_at(point: Vector2) -> Vector2i:
	var size := map.size
	var grid := Vector2(sim.grid_width(), sim.grid_height())
	var scale := minf(size.x / grid.x, size.y / grid.y)
	var origin := (size - grid * scale) * 0.5
	var tile := Vector2i((point - origin) / scale)
	if tile.x < 0 or tile.y < 0 or tile.x >= sim.grid_width() or tile.y >= sim.grid_height():
		return Vector2i(-1, -1)
	return tile


func repaint_if_dirty() -> void:
	if sim.revision() == last_revision:
		return
	repaint()


func repaint() -> void:
	var bytes := sim.paint_overlay(OVERLAYS[overlay_index])
	image.set_data(sim.grid_width(), sim.grid_height(), false, Image.FORMAT_RGBA8, bytes)
	texture.update(image)
	last_revision = sim.revision()


## The scripture column: the Chosen people's newest verses, first at the top.
func _refresh_scripture() -> void:
	var chosen := sim.chosen_culture()
	if chosen < 0:
		return
	var doctrine := sim.culture_doctrine(chosen)
	scripture_header.text = (
		"%s · %s · piety %.2f"
		% [
			sim.culture_name(chosen),
			doctrine if doctrine != "" else "no doctrine",
			sim.culture_piety(chosen),
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


func _unhandled_key_input(event: InputEvent) -> void:
	# Number keys pick a power. Handled here so the map keeps keyboard focus.
	if not (event is InputEventKey and event.pressed and not event.echo):
		return
	var index: int = event.keycode - KEY_1
	if index >= 0 and index < POWERS.size():
		_select_power(index)
