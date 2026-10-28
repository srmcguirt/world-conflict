class_name Game
extends Node2D

# The play screen (design, Screen flow and play layout): the map fills the
# window; a one-line top bar, the power toolbar bottom-center, the event log
# strip along the bottom, and one selection panel on demand. app.gd builds it
# in stages behind the loading screen: prepare(), build_step() until done,
# then finish().

signal menu_requested
signal world_broke(message: String)

const OVERLAY_BIOME := 0
const OVERLAY_CULTURE := 1
const OVERLAY_TRAITS := 2
const OVERLAY_BELIEF := 3
const OVERLAY_POLITY := 4
const OVERLAY_LANGUAGE := 5
const OVERLAYS := [
	OVERLAY_BIOME, OVERLAY_CULTURE, OVERLAY_TRAITS, OVERLAY_BELIEF, OVERLAY_LANGUAGE, OVERLAY_POLITY
]
const OVERLAY_NAMES := {
	OVERLAY_BIOME: "biome",
	OVERLAY_CULTURE: "culture",
	OVERLAY_TRAITS: "traits",
	OVERLAY_BELIEF: "belief",
	OVERLAY_POLITY: "polity",
	OVERLAY_LANGUAGE: "language",
}

const SPEEDS := [1, 5, 25, 100]
# Design, Frame and tick scheduling: whole ticks until the accumulator is
# spent or this budget is used; the rest carries to the next frame.
const TICK_BUDGET_MS := 8.0
const POWERS := ["Raise", "Lower", "Paint", "Bless", "Meteor", "Tornado", "Spawn"]
const GROUND_POWERS := [0, 1, 4]  # Raise, Lower, Meteor reshape the ground
const TIERS := ["Camp", "Village", "Town", "City"]

var sim: Sim
var settings: GameSettings
var terrain: HexTerrain
var camera: Camera2D
var settlement_layer: Node2D
var selection_ring: Line2D
var ui: CanvasLayer
var color_image: Image
var color_texture: ImageTexture
var code_image: Image
var code_texture: ImageTexture

var grid_w := 0
var grid_h := 0
var chosen := -1

var paused := true
var speed_index := 0
var year_accumulator := 0.0
var overlay_index := 0
var selected_power := -1
var selected_tile := Vector2i(-1, -1)
var zoom := 1.6

var _bar: Label
var _frame_label: Label
var _power_buttons: Array[Button] = []
var _selection_panel: PanelContainer
var _selection_label: Label
var _kingdom_row: HBoxContainer
var _kingdom_label: Label
var _polity_panel: PolityPanel
var _shown_polity := -1
var _history: Array[Dictionary] = []  # recent kingdom events, every people's, oldest first
var _log: EventLog

var _pressing := false
var _dragging := false
var _press_at := Vector2.ZERO
var _drag_origin := Vector2.ZERO
var _last_revision := -1
var _last_overlay := -1
var _ground_checks := 0  # repaints left to look for a reshaped ground
var _ghost_layers: Array[Node2D] = []
var _views := {}  # founding tile -> SettlementView, with its ghosts in _ghost_views
var _ghost_views := {}  # founding tile -> [west copy, east copy]
var _view_keys := {}  # founding tile -> its last (tier, culture, built), to redraw only on change
var _last_autosave_year := 0
var _last_autosave_ms := 0
var _broken := false
var _shots: Array[Dictionary] = []
var _shot_timer := 0.0


# Stage 1: textures and terrain data. The bands build in build_step().
func prepare(world: Sim, player_settings: GameSettings) -> void:
	sim = world
	settings = player_settings
	grid_w = sim.grid_width()
	grid_h = sim.grid_height()
	chosen = sim.chosen_culture()
	color_image = Image.create(grid_w, grid_h, false, Image.FORMAT_RGBA8)
	color_texture = ImageTexture.create_from_image(color_image)
	code_image = Image.create(grid_w, grid_h, false, Image.FORMAT_RGBA8)
	code_texture = ImageTexture.create_from_image(code_image)
	terrain = HexTerrain.new()
	terrain.setup(grid_w, grid_h, color_texture, code_texture)
	terrain.set_chosen(chosen)
	add_child(terrain)
	terrain.begin(sim.tile_heights(), sim.tile_biomes())

	# Buildings stand on the terrain; y-sorting puts nearer settlements in front.
	settlement_layer = Node2D.new()
	settlement_layer.y_sort_enabled = true
	add_child(settlement_layer)
	# Copies of the settlements one world west and east, like the terrain's.
	for side in [-1.0, 1.0]:
		var ghost := Node2D.new()
		ghost.y_sort_enabled = true
		ghost.position.x = side * terrain.world_width()
		add_child(ghost)
		_ghost_layers.append(ghost)
	selection_ring = Line2D.new()
	selection_ring.width = 1.6
	selection_ring.default_color = UiTheme.GOLD
	selection_ring.visible = false
	add_child(selection_ring)


# Stage 2: one terrain band per call; returns the share done.
func build_step() -> float:
	return terrain.build_next()


# Stage 3: settlements, camera and the HUD; play opens paused (design, Screen
# flow: Play).
func finish() -> void:
	camera = Camera2D.new()
	camera.position_smoothing_enabled = true
	camera.position_smoothing_speed = 10.0
	add_child(camera)
	camera.make_current()
	_build_hud()
	_set_zoom(zoom)
	camera.position = _chosen_center()
	camera.reset_smoothing()
	repaint_if_dirty()
	_log.set_culture(chosen)
	_drain_events()
	_log.refresh(true)
	apply_settings()
	_last_autosave_year = sim.tick()
	_last_autosave_ms = Time.get_ticks_msec()


func apply_settings() -> void:
	if ui == null:
		return
	for child in ui.get_children():
		if child is Control:
			child.scale = Vector2.ONE * settings.ui_scale
	_layout_hud()
	_frame_label.visible = settings.show_frame_time


func _build_hud() -> void:
	ui = CanvasLayer.new()
	add_child(ui)
	var theme := UiTheme.build()

	var top := PanelContainer.new()
	top.theme = theme
	top.add_theme_stylebox_override("panel", UiTheme.panel_box(UiTheme.INK, 0.78))
	top.set_meta("slot", "top")
	ui.add_child(top)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 16)
	top.add_child(row)
	_bar = Label.new()
	_bar.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(_bar)
	_frame_label = UiTheme.muted("", 12)
	row.add_child(_frame_label)
	var menu := Button.new()
	menu.text = "Menu"
	menu.focus_mode = Control.FOCUS_NONE
	menu.pressed.connect(func() -> void: menu_requested.emit())
	row.add_child(menu)

	var tools := HBoxContainer.new()
	tools.theme = theme
	tools.add_theme_constant_override("separation", 4)
	tools.set_meta("slot", "tools")
	for index in POWERS.size():
		var button := Button.new()
		button.text = "%d %s" % [index + 1, POWERS[index]]
		button.focus_mode = Control.FOCUS_NONE
		button.toggle_mode = true
		button.pressed.connect(_toggle_power.bind(index))
		tools.add_child(button)
		_power_buttons.append(button)
	ui.add_child(tools)

	_log = EventLog.new()
	_log.theme = theme
	_log.setup(sim)
	_log.set_meta("slot", "log")
	ui.add_child(_log)
	_log.minimum_size_changed.connect(_layout_hud)

	_selection_panel = PanelContainer.new()
	_selection_panel.theme = theme
	_selection_panel.visible = false
	_selection_panel.custom_minimum_size = Vector2(240, 0)
	_selection_panel.set_meta("slot", "selection")
	var card := VBoxContainer.new()
	card.add_theme_constant_override("separation", 6)
	_selection_panel.add_child(card)
	_selection_label = Label.new()
	card.add_child(_selection_label)
	# The settlement card's kingdom strip (design, Meters DR2-D2).
	_kingdom_row = HBoxContainer.new()
	_kingdom_row.add_theme_constant_override("separation", 8)
	_kingdom_label = UiTheme.muted("", 12)
	_kingdom_label.autowrap_mode = TextServer.AUTOWRAP_OFF
	_kingdom_row.add_child(_kingdom_label)
	var open := Button.new()
	open.text = "Open kingdom"
	open.focus_mode = Control.FOCUS_NONE
	open.pressed.connect(func() -> void: _show_polity(_shown_polity))
	_kingdom_row.add_child(open)
	card.add_child(_kingdom_row)
	ui.add_child(_selection_panel)

	_polity_panel = PolityPanel.new()
	_polity_panel.theme = theme
	_polity_panel.visible = false
	_polity_panel.set_meta("slot", "selection")
	_polity_panel.focus_requested.connect(_focus_tile)
	ui.add_child(_polity_panel)
	_log.focus_requested.connect(_focus_tile)
	get_viewport().size_changed.connect(_layout_hud)


# Places the HUD pieces by hand, so the interface size setting can scale them.
func _layout_hud() -> void:
	var view := get_viewport().get_visible_rect().size
	var s := settings.ui_scale
	var log_h := _log.get_combined_minimum_size().y * s
	for child in ui.get_children():
		if not child is Control:
			continue
		var c: Control = child
		var size := c.get_combined_minimum_size()
		match c.get_meta("slot", ""):
			"top":
				c.size = Vector2(view.x / s, size.y)
				c.position = Vector2.ZERO
			"log":
				c.size = Vector2(minf(view.x / s - 24.0, 900.0), size.y)
				c.position = Vector2((view.x - c.size.x * s) * 0.5, view.y - log_h - 8.0)
			"tools":
				c.position = Vector2(
					(view.x - size.x * s) * 0.5, view.y - log_h - size.y * s - 16.0
				)
			"selection":
				c.size = size
				c.position = Vector2(12, 52 * s)


func _process(delta: float) -> void:
	if camera == null:
		return
	if sim.is_poisoned():
		_on_poisoned()
		return
	if not paused:
		_run_budgeted(delta)
	_update_bar()
	if settings.show_frame_time:
		_frame_label.text = "%.1f ms" % (delta * 1000.0)
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
	_wrap_camera()


# Panning past either edge comes round the world: the camera jumps one
# world-width and the ghost copies make the jump invisible.
func _wrap_camera() -> void:
	var lap := terrain.world_width()
	if camera.position.x < 0.0 or camera.position.x >= lap:
		var shift := floorf(camera.position.x / lap) * lap
		camera.position.x -= shift
		_drag_origin.x -= shift
		camera.reset_smoothing()


# Whole ticks until the years due are spent or the budget is used. The
# backlog never grows past one second of play, so a slow stretch can't
# snowball into a stall.
func _run_budgeted(delta: float) -> void:
	var speed: float = SPEEDS[speed_index]
	year_accumulator = minf(year_accumulator + delta * speed, speed)
	var started := Time.get_ticks_usec()
	var ran := 0
	while year_accumulator >= 1.0:
		sim.step(1)
		year_accumulator -= 1.0
		ran += 1
		if float(Time.get_ticks_usec() - started) / 1000.0 >= TICK_BUDGET_MS:
			break
	if ran > 0:
		repaint_if_dirty()
		_drain_events()
		_log.refresh()
		_maybe_autosave()


# R12: a rolling autosave once 100 sim-years have passed and at least 60 real
# seconds, always between ticks.
func _maybe_autosave() -> void:
	var years := sim.tick() - _last_autosave_year
	var seconds := float(Time.get_ticks_msec() - _last_autosave_ms) / 1000.0
	if years < SaveStore.AUTOSAVE_YEARS or seconds < SaveStore.AUTOSAVE_SECONDS:
		return
	if SaveStore.autosave(sim):
		_last_autosave_year = sim.tick()
		_last_autosave_ms = Time.get_ticks_msec()


# R5: a caught panic poisons the Sim; the world stops and app.gd offers the
# last autosave.
func _on_poisoned() -> void:
	if _broken:
		return
	_broken = true
	paused = true
	world_broke.emit("The world can't go on from here. Your last autosave can.")


func _update_bar() -> void:
	var state := "paused" if paused else "%d×" % SPEEDS[speed_index]
	var power: String = POWERS[selected_power] if selected_power >= 0 else "none"
	_bar.text = (
		"year %d   ·   %s   ·   overlay %s   ·   peoples %d   ·   power %s"
		% [sim.tick(), state, OVERLAY_NAMES[OVERLAYS[overlay_index]], sim.culture_count(), power]
	)


func repaint_if_dirty() -> void:
	if sim.revision() == _last_revision and overlay_index == _last_overlay:
		return
	_last_revision = sim.revision()
	_last_overlay = overlay_index
	var bytes: PackedByteArray = sim.paint_overlay(OVERLAYS[overlay_index])
	color_image.set_data(grid_w, grid_h, false, Image.FORMAT_RGBA8, bytes)
	color_texture.update(color_image)
	code_image.set_data(grid_w, grid_h, false, Image.FORMAT_RGBA8, sim.tile_codes())
	code_texture.update(code_image)
	# Only powers move the ground, so heights are compared only for a few
	# repaints after one is cast; the terrain rebuilds just the bands nearby.
	if _ground_checks > 0:
		_ground_checks -= 1
		var changed: PackedInt32Array = sim.changed_tiles(terrain.heights())
		if terrain.update_heights(changed, sim.tile_heights()):
			_reseat_settlements()
	_refresh_settlements()


# Settlements update in place: only a site that was founded or changed tier,
# owner or buildings is redrawn, and only a vanished one is freed.
func _refresh_settlements() -> void:
	var sites: PackedInt32Array = sim.settlement_sites()
	var biomes: PackedByteArray = sim.tile_biomes()
	var seen := {}
	for s in range(0, sites.size(), 4):
		var tile: int = sites[s]
		var key := Vector3i(sites[s + 1], sites[s + 2], sites[s + 3])
		seen[tile] = true
		if _view_keys.get(tile) == key:
			continue
		_view_keys[tile] = key
		if not _views.has(tile):
			_views[tile] = _new_view(settlement_layer, tile)
			_ghost_views[tile] = [
				_new_view(_ghost_layers[0], tile), _new_view(_ghost_layers[1], tile)
			]
		for view in [_views[tile]] + _ghost_views[tile]:
			view.configure(
				tile,
				key.x,
				biomes[tile],
				sim.culture_dress(key.y),
				key.y == chosen,
				sim.culture_doctrine(key.y) != "",
				key.z
			)
	for tile in _views.keys():
		if not seen.has(tile):
			for view in [_views[tile]] + _ghost_views[tile]:
				view.queue_free()
			_views.erase(tile)
			_ghost_views.erase(tile)
			_view_keys.erase(tile)


func _new_view(layer: Node2D, tile: int) -> SettlementView:
	var view := SettlementView.new()
	view.position = terrain.top(tile % grid_w, tile / grid_w)
	view.scale = Vector2.ONE * _settlement_scale()
	layer.add_child(view)
	return view


func _reseat_settlements() -> void:
	for tile in _views:
		for view in [_views[tile]] + _ghost_views[tile]:
			view.position = terrain.top(tile % grid_w, tile / grid_w)


# Zoomed out, buildings grow so a town still reads against a camp at 0.5x
# (art bible, readability gate 1); zoomed in they keep their true size.
func _settlement_scale() -> float:
	return clampf(1.1 / zoom, 1.0, 2.4)


func _set_zoom(z: float) -> void:
	zoom = clampf(z, 0.4, 5.0)
	camera.zoom = Vector2(zoom, zoom)
	var grow := _settlement_scale()
	for layer in [settlement_layer] + _ghost_layers:
		for view in layer.get_children():
			view.scale = Vector2(grow, grow)


# Home for the camera (design, Finding your people): the Chosen's largest
# settlement, or the Garden before there is one. Averaging their tiles would land in
# the sea when the people straddle the east-west seam.
func _chosen_center() -> Vector2:
	var sites: PackedInt32Array = sim.settlement_sites()
	var best := -1
	var best_tier := -1
	for s in range(0, sites.size(), 4):
		if sites[s + 2] == chosen and sites[s + 1] > best_tier:
			best_tier = sites[s + 1]
			best = sites[s]
	if best >= 0:
		return terrain.top(best % grid_w, best / grid_w)
	var garden: Vector2i = sim.garden_tile()
	return terrain.top(garden.x, garden.y)


# Esc clears an armed power or a selection first; only an Esc with nothing
# to clear opens the menu (design, Screen flow: Esc menu).
func handle_escape() -> bool:
	if selected_power >= 0 or selected_tile.x >= 0:
		_toggle_power(selected_power)
		_clear_selection()
		return true
	return false


func _unhandled_key_input(event: InputEvent) -> void:
	if camera == null or not (event is InputEventKey and event.pressed and not event.echo):
		return
	var key: int = event.keycode
	match key:
		KEY_SPACE:
			paused = not paused
		KEY_BRACKETLEFT:
			speed_index = maxi(speed_index - 1, 0)
		KEY_BRACKETRIGHT:
			speed_index = mini(speed_index + 1, SPEEDS.size() - 1)
		KEY_M:
			overlay_index = (overlay_index + 1) % OVERLAYS.size()
			repaint_if_dirty()
		KEY_L:
			_log.toggle()
		KEY_H:
			camera.position = _chosen_center()
		_:
			if key >= KEY_1 and key < KEY_1 + POWERS.size():
				_toggle_power(key - KEY_1)
			else:
				return
	get_viewport().set_input_as_handled()


func _unhandled_input(event: InputEvent) -> void:
	if camera == null:
		return
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_set_zoom(zoom * 1.12)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_set_zoom(zoom / 1.12)
		elif event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				_pressing = true
				_dragging = false
				_press_at = event.position
				_drag_origin = camera.position
			else:
				# A press that never moved is a click on the map.
				if _pressing and not _dragging:
					_on_map_click()
				_pressing = false
				_dragging = false
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			if selected_power >= 0:
				_toggle_power(selected_power)
			_clear_selection()
	elif event is InputEventMouseMotion and _pressing:
		if not _dragging and event.position.distance_to(_press_at) > 4.0:
			_dragging = true
		if _dragging:
			camera.position = _drag_origin - (event.position - _press_at) / zoom
			camera.reset_smoothing()


func _on_map_click() -> void:
	var tile := terrain.tile_at(terrain.get_global_mouse_position())
	if tile.x < 0:
		return
	if selected_power >= 0:
		sim.cast_power(selected_power, tile.x, tile.y)
		if GROUND_POWERS.has(selected_power):
			_ground_checks = 3
		if paused:
			sim.step(1)
		repaint_if_dirty()
		_drain_events()
		_log.refresh()
		return
	_select_tile(tile)


# Kingdom events reach the log (the Chosen's) and the polity panel's unrest
# list (any kingdom's) through one drain per frame (design, typed events; R4).
func _drain_events() -> void:
	var drained: Dictionary = sim.drain_events()
	var events: Array = drained["events"]
	_log.add_events(events, drained["dropped"])
	for event in events:
		_history.append(event)
	if _history.size() > 400:
		_history = _history.slice(_history.size() - 400)
	if _polity_panel.visible:
		_show_polity(_shown_polity)


func _focus_tile(tile: int) -> void:
	camera.position = terrain.top(tile % grid_w, tile / grid_w)
	_select_tile(Vector2i(tile % grid_w, tile / grid_w))


# Arms a power, or disarms it when its own key or button is pressed again.
func _toggle_power(index: int) -> void:
	selected_power = -1 if index == selected_power else index
	for i in _power_buttons.size():
		_power_buttons[i].set_pressed_no_signal(i == selected_power)
	_clear_selection()


func _clear_selection() -> void:
	selected_tile = Vector2i(-1, -1)
	_selection_panel.visible = false
	_polity_panel.visible = false
	selection_ring.visible = false


func _select_tile(tile: Vector2i) -> void:
	_clear_selection()
	selected_tile = tile
	selection_ring.points = terrain.hex_outline(tile.x, tile.y)
	selection_ring.visible = true
	var sid: int = sim.settlement_at_tile(tile.x, tile.y)
	if sid >= 0:
		var card: PackedInt32Array = sim.settlement_card(sid)
		if card.size() >= 6:
			_selection_label.text = (
				"%s of the %s\n%d people · %d of %d buildings"
				% [
					TIERS[clampi(card[1], 0, 3)],
					sim.culture_name(card[2]),
					card[4],
					card[3],
					card[5]
				]
			)
			_shown_polity = sim.polity_at_tile(card[0] % grid_w, card[0] / grid_w)
			_kingdom_row.visible = _shown_polity >= 0
			if _shown_polity >= 0:
				var kingdom: Dictionary = sim.polity_card(_shown_polity)
				var strip := "Kingdom of %s · %s" % [kingdom["capital"], kingdom["government"]]
				if kingdom["visible"]:
					strip += " · unrest %.2f" % kingdom["unrest"]
				_kingdom_label.text = strip
			_selection_panel.visible = true
			_layout_hud.call_deferred()
			return
	var pid: int = sim.polity_at_tile(tile.x, tile.y)
	if pid >= 0:
		_show_polity(pid)


func _show_polity(pid: int) -> void:
	_shown_polity = pid
	var unrest: Array = []
	for i in range(_history.size() - 1, -1, -1):
		var event: Dictionary = _history[i]
		if event["polity"] == pid and (event["kind"] == "protest" or event["kind"] == "riot"):
			unrest.append(event)
	_polity_panel.show_polity(sim.polity_card(pid), unrest)
	_selection_panel.visible = false
	_polity_panel.visible = true
	_layout_hud.call_deferred()


# --shots: a fixed tour for milestone screenshots, then quit.
func queue_shots() -> void:
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
	var peak := terrain.top(summit % grid_w, summit / grid_w)
	_shots = [
		{"name": "hex-map-05x", "zoom": 0.5, "at": center},
		{"name": "hex-mountains-3x", "zoom": 3.0, "at": peak},
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
