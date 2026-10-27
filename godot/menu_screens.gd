class_name MenuScreens
extends RefCounted

# Builders for the menu screens (design, Screen flow and play layout): title,
# new world, loading, the Esc menu and settings. Each returns a full-window
# Control; app.gd owns the flow between them.


# A centered card on a dimmed (or solid) ground.
static func _card(ground: Color, width: float) -> Array:
	var root := ColorRect.new()
	root.color = ground
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	var center := CenterContainer.new()
	center.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.add_child(center)
	var column := VBoxContainer.new()
	column.custom_minimum_size.x = width
	column.add_theme_constant_override("separation", 12)
	center.add_child(column)
	return [root, column]


static func title(on_new: Callable, on_settings: Callable, on_quit: Callable) -> Control:
	var parts := _card(UiTheme.INK, 320)
	var column: VBoxContainer = parts[1]
	column.add_child(UiTheme.heading("World Conflict", 44, UiTheme.GOLD))
	column.add_child(UiTheme.muted("A god-game of a chosen people.", 16))
	column.add_child(_gap(24))
	var begin := UiTheme.button("New world", on_new)
	column.add_child(begin)
	var resume := UiTheme.button("Continue", func() -> void: pass)
	resume.disabled = true
	resume.tooltip_text = "No saved worlds yet"
	column.add_child(resume)
	column.add_child(UiTheme.muted("No saved worlds yet", 12))
	column.add_child(UiTheme.button("Settings", on_settings))
	column.add_child(UiTheme.button("Quit", on_quit))
	begin.call_deferred("grab_focus")
	return parts[0]


# Seeds are numbers or words; a word hashes to a number, shown beside it so a
# world can be shared either way (design, Screen flow: New world).
static func seed_of(text: String) -> int:
	var trimmed := text.strip_edges()
	if trimmed.is_valid_int():
		return trimmed.to_int()
	return trimmed.hash() & 0x7FFFFFFF


static func new_world(on_begin: Callable, on_back: Callable) -> Control:
	var parts := _card(UiTheme.INK, 360)
	var column: VBoxContainer = parts[1]
	column.add_child(UiTheme.heading("A new world", 30))
	column.add_child(UiTheme.muted("Every seed raises the same world, every time.", 14))
	column.add_child(_gap(12))
	var field := LineEdit.new()
	field.placeholder_text = "a number or a word"
	field.text = str(randi() % 1000000)
	field.alignment = HORIZONTAL_ALIGNMENT_CENTER
	field.custom_minimum_size.y = 40
	column.add_child(field)
	var echo := UiTheme.muted("", 12)
	column.add_child(echo)
	var refresh := func() -> void:
		var text := field.text.strip_edges()
		if text.is_empty():
			echo.text = "seeds are numbers or words"
		elif text.is_valid_int():
			echo.text = " "
		else:
			echo.text = "seed %d" % seed_of(text)
	field.text_changed.connect(func(_t: String) -> void: refresh.call())
	refresh.call()
	var start := func() -> void:
		if field.text.strip_edges().is_empty():
			_shake(field)
			echo.text = "seeds are numbers or words"
			return
		on_begin.call(seed_of(field.text))
	field.text_submitted.connect(func(_t: String) -> void: start.call())
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 8)
	var random := UiTheme.button(
		"Random", func() -> void:
			field.text = str(randi() % 1000000)
			refresh.call()
	)
	random.custom_minimum_size.x = 0
	random.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(random)
	var begin := UiTheme.button("Begin", start)
	begin.custom_minimum_size.x = 0
	begin.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(begin)
	column.add_child(row)
	column.add_child(UiTheme.button("Back", on_back))
	field.call_deferred("grab_focus")
	return parts[0]


static func _shake(control: Control) -> void:
	var tween := control.create_tween()
	var x := control.position.x
	for offset in [8.0, -8.0, 5.0, -5.0, 0.0]:
		tween.tween_property(control, "position:x", x + offset, 0.04)


# The staged loading card; returns [root, stage label, progress bar].
static func loading(seed_value: int) -> Array:
	var parts := _card(UiTheme.INK, 420)
	var column: VBoxContainer = parts[1]
	var stage := UiTheme.heading("Raising the mountains…", 22)
	column.add_child(stage)
	var bar := ProgressBar.new()
	bar.show_percentage = false
	bar.custom_minimum_size.y = 6
	var fill := StyleBoxFlat.new()
	fill.bg_color = UiTheme.GOLD
	bar.add_theme_stylebox_override("fill", fill)
	var ground := StyleBoxFlat.new()
	ground.bg_color = UiTheme.UMBER
	bar.add_theme_stylebox_override("background", ground)
	column.add_child(bar)
	column.add_child(UiTheme.muted("seed %d" % seed_value, 13))
	return [parts[0], stage, bar]


static func pause_menu(actions: Dictionary) -> Control:
	var parts := _card(Color(UiTheme.INK, 0.72), 280)
	var column: VBoxContainer = parts[1]
	column.add_child(UiTheme.heading("Paused", 28))
	column.add_child(_gap(8))
	var first: Button = null
	for label in ["Resume", "Settings", "New world", "Main menu", "Quit"]:
		var b := UiTheme.button(label, actions[label])
		column.add_child(b)
		if first == null:
			first = b
	first.call_deferred("grab_focus")
	return parts[0]


static func settings(values: GameSettings, on_change: Callable, on_back: Callable) -> Control:
	var parts := _card(Color(UiTheme.INK, 0.86), 360)
	var column: VBoxContainer = parts[1]
	column.add_child(UiTheme.heading("Settings", 28))
	column.add_child(_gap(8))

	var pause := CheckButton.new()
	pause.text = "Pause on great events"
	pause.button_pressed = values.pause_on_great_events
	pause.toggled.connect(
		func(on: bool) -> void:
			values.pause_on_great_events = on
			on_change.call()
	)
	column.add_child(pause)

	var frame := CheckButton.new()
	frame.text = "Show frame time"
	frame.button_pressed = values.show_frame_time
	frame.toggled.connect(
		func(on: bool) -> void:
			values.show_frame_time = on
			on_change.call()
	)
	column.add_child(frame)

	var scale_row := HBoxContainer.new()
	var scale_label := Label.new()
	scale_label.text = "Interface size %d%%" % roundi(values.ui_scale * 100.0)
	scale_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scale_row.add_child(scale_label)
	var slider := HSlider.new()
	slider.min_value = 0.8
	slider.max_value = 1.5
	slider.step = 0.05
	slider.value = values.ui_scale
	slider.custom_minimum_size.x = 150
	slider.drag_ended.connect(
		func(_changed: bool) -> void:
			values.ui_scale = slider.value
			on_change.call()
	)
	slider.value_changed.connect(
		func(v: float) -> void: scale_label.text = "Interface size %d%%" % roundi(v * 100.0)
	)
	scale_row.add_child(slider)
	column.add_child(scale_row)

	column.add_child(_gap(8))
	var back := UiTheme.button("Back", on_back)
	column.add_child(back)
	back.call_deferred("grab_focus")
	return parts[0]


static func _gap(height: float) -> Control:
	var gap := Control.new()
	gap.custom_minimum_size.y = height
	return gap
