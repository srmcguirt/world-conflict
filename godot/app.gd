extends Node

# The screen flow (design, Screen flow and play layout): Title → New world →
# Loading → Play, with the Esc menu and settings over Play. Menus live on a
# CanvasLayer above the game; the game itself is one Game node, rebuilt for
# every new world.
#
# Flags: `-- --shots [--years N]` loads seed 1 and captures the map tour;
# `-- --ui-shots` captures every screen; both quit when done.

signal world_ready

var settings: GameSettings
var theme: Theme
var game: Game
var _layer: CanvasLayer
var _screen: Control  # the menu screen showing, if any
var _settings_from: Callable  # where Settings' Back returns
var _was_paused := true
var _loading := false


func _ready() -> void:
	settings = GameSettings.load_or_default()
	theme = UiTheme.build()
	RenderingServer.set_default_clear_color(UiTheme.INK)
	_layer = CanvasLayer.new()
	_layer.layer = 10
	add_child(_layer)
	var args := OS.get_cmdline_user_args()
	if args.has("--shots"):
		await start_world(1, _years_arg(args))
		game.queue_shots()
	elif args.has("--ui-shots"):
		_ui_tour()
	else:
		show_title()


func _years_arg(args: PackedStringArray) -> int:
	var at := args.find("--years")
	return int(args[at + 1]) if at >= 0 and at + 1 < args.size() else 0


func _show(screen: Control) -> void:
	if _screen != null:
		_screen.queue_free()
	_screen = screen
	if screen != null:
		screen.theme = theme
		_layer.add_child(screen)


func show_title() -> void:
	_end_game()
	_show(MenuScreens.title(show_new_world, _open_settings.bind(show_title), _quit))


func show_new_world() -> void:
	_end_game()
	_show(MenuScreens.new_world(func(seed_value: int) -> void: start_world.call(seed_value, 0), show_title))


func _open_settings(back: Callable) -> void:
	_settings_from = back
	_show(MenuScreens.settings(settings, _settings_changed, func() -> void: _settings_from.call()))


func _settings_changed() -> void:
	settings.save()
	if game != null:
		game.apply_settings()


func _open_menu() -> void:
	if game == null or _loading:
		return
	_was_paused = game.paused
	game.paused = true
	_show(
		MenuScreens.pause_menu(
			{
				"Resume": _resume,
				"Settings": _open_settings.bind(_open_menu_again),
				"New world": show_new_world,
				"Main menu": show_title,
				"Quit": _quit,
			}
		)
	)


# Back from settings into the Esc menu keeps the paused state it saved.
func _open_menu_again() -> void:
	_show(
		MenuScreens.pause_menu(
			{
				"Resume": _resume,
				"Settings": _open_settings.bind(_open_menu_again),
				"New world": show_new_world,
				"Main menu": show_title,
				"Quit": _quit,
			}
		)
	)


func _resume() -> void:
	_show(null)
	if game != null:
		game.paused = _was_paused


func _quit() -> void:
	get_tree().quit()


func _end_game() -> void:
	if game != null:
		game.queue_free()
		game = null


# Loading in the design's real stages; each await lets a frame draw so the
# bar moves (design, Screen flow: Loading).
func start_world(seed_value: int, years: int) -> void:
	_end_game()
	_loading = true
	var parts := MenuScreens.loading(seed_value)
	_show(parts[0])
	var stage: Label = parts[1]
	var bar: ProgressBar = parts[2]
	bar.max_value = 1.0
	await get_tree().process_frame
	await get_tree().process_frame

	var sim := Sim.create(seed_value)
	game = Game.new()
	add_child(game)
	game.add_child(sim)
	if years > 0:
		stage.text = "The first %d years…" % years
		var done := 0
		while done < years:
			var batch := mini(50, years - done)
			sim.step(batch)
			done += batch
			bar.value = 0.5 * float(done) / float(years)
			await get_tree().process_frame

	stage.text = "Shaping the land…"
	game.prepare(sim, settings)
	var base := bar.value
	var progress := 0.0
	while progress < 1.0:
		progress = game.build_step()
		bar.value = base + (1.0 - base) * progress
		await get_tree().process_frame

	stage.text = "Setting the first people by the river…"
	await get_tree().process_frame
	game.finish()
	game.menu_requested.connect(_open_menu)
	_loading = false
	_show(null)
	world_ready.emit()


func _unhandled_key_input(event: InputEvent) -> void:
	if not (event is InputEventKey and event.pressed and not event.echo):
		return
	if event.keycode != KEY_ESCAPE or _loading:
		return
	get_viewport().set_input_as_handled()
	if game == null:
		return
	if _screen != null:
		_resume()
	elif not game.handle_escape():
		_open_menu()


# --ui-shots: every screen once, for review.
func _ui_tour() -> void:
	var size := Vector2i(1280, 720)
	get_window().size = size
	show_title()
	await _capture("ui-title")
	show_new_world()
	await _capture("ui-new-world")
	start_world.call(1, 1200)
	await get_tree().create_timer(0.5).timeout
	await _capture("ui-loading", 0.0)
	if _loading:
		await world_ready
	await _capture("ui-play")
	game._log.toggle()
	var tile := _first_settlement_tile()
	game._select_tile(tile)

	await _capture("ui-play-log-selection")
	var kingdom: int = game.sim.polity_at_tile(tile.x, tile.y)
	if kingdom >= 0:
		game._show_polity(kingdom)
		await _capture("ui-polity")
	_open_menu()
	await _capture("ui-esc-menu")
	_open_settings(_open_menu_again)
	await _capture("ui-settings")
	get_tree().quit()


func _first_settlement_tile() -> Vector2i:
	# The Chosen's biggest settlement, which is the likeliest to head a kingdom.
	var sites: PackedInt32Array = game.sim.settlement_sites()
	var w: int = game.grid_w
	var best := Vector2i(w / 2, 10)
	var best_tier := -1
	for s in range(0, sites.size(), 4):
		if sites[s + 2] == game.chosen and sites[s + 1] > best_tier:
			best_tier = sites[s + 1]
			best = Vector2i(sites[s] % w, sites[s] / w)
	return best


func _capture(name: String, settle: float = 0.4) -> void:
	if settle > 0.0:
		await get_tree().create_timer(settle).timeout
	await RenderingServer.frame_post_draw
	var path := ProjectSettings.globalize_path("res://../docs/designs/%s.png" % name)
	get_viewport().get_texture().get_image().save_png(path)
	print("[shot] saved %s" % path)
