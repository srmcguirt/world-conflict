class_name PolityPanel
extends PanelContainer

# The polity panel (design, Meters DR2-D2; M6a plan, Polity panel): the six
# meters as bars with their numbers, unrest's thresholds printed under it,
# Liberty, the government and where its legitimacy comes from, the capital,
# the settlement count, and the kingdom's recent unrest. Rival kingdoms show
# their meters only while one of their people is a witness (design, Open
# question 2, the proposal). No information rides on color alone: every bar
# carries its number.

signal focus_requested(tile: int)

const BAR_WIDTH := 150.0
const PROTEST_AT := 0.5  # society.rs tick_unrest
const RIOT_AT := 0.65
# The six meters in the design table's order, then Liberty (custom).
const METERS := [
	["Prosperity", "prosperity"],
	["Inequality", "inequality"],
	["Legitimacy", "legitimacy"],
	["Piety", "piety"],
	["Cohesion", "cohesion"],
	["Unrest", "unrest"],
]

var _column: VBoxContainer


func _init() -> void:
	custom_minimum_size = Vector2(300, 0)
	_column = VBoxContainer.new()
	_column.add_theme_constant_override("separation", 6)
	add_child(_column)


# Fills the panel for one kingdom; `unrest` is its recent protests and riots,
# newest first.
func show_polity(card: Dictionary, unrest: Array) -> void:
	for child in _column.get_children():
		child.queue_free()
	if card.is_empty():
		_column.add_child(UiTheme.muted("No kingdom yet", 13))
		return
	var title := Label.new()
	title.text = "Kingdom of %s" % card["capital"]
	title.add_theme_font_size_override("font_size", 17)
	title.add_theme_color_override("font_color", UiTheme.GOLD if card["chosen"] else UiTheme.TEXT)
	_column.add_child(title)
	_column.add_child(
		_left(
			UiTheme.muted(
				(
					"the %s · %s · %d settlements · legitimacy from %s"
					% [
						card["people"],
						card["government"],
						card["settlements"],
						card["legitimacy_source"]
					]
				),
				13
			)
		)
	)
	var home := Button.new()
	home.text = "Show the capital"
	home.focus_mode = Control.FOCUS_NONE
	var capital: int = card["capital_tile"]
	home.pressed.connect(func() -> void: focus_requested.emit(capital))
	_column.add_child(home)

	if not card["visible"]:
		_column.add_child(
			_left(
				UiTheme.muted(
					"Their meters are hidden. A witness among them would let you see.", 13
				)
			)
		)
		return
	for meter in METERS:
		_column.add_child(_meter_row(meter[0], card[meter[1]]))
		if meter[1] == "unrest":
			_column.add_child(
				_left(UiTheme.muted("protest at %.2f · riot at %.2f" % [PROTEST_AT, RIOT_AT], 12))
			)
	_column.add_child(_meter_row("Liberty", card["liberty"], UiTheme.MUTED))

	var heading := Label.new()
	heading.text = "Recent unrest"
	heading.add_theme_font_size_override("font_size", 13)
	_column.add_child(heading)
	if unrest.is_empty():
		_column.add_child(_left(UiTheme.muted("None in living memory.", 12)))
	for event in unrest.slice(0, 4):
		_column.add_child(_left(UiTheme.muted("year %d · %s" % [event["tick"], event["text"]], 12)))


func _meter_row(name: String, value: float, color: Color = UiTheme.OCHRE) -> Control:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 8)
	var label := Label.new()
	label.text = name
	label.custom_minimum_size.x = 86
	label.add_theme_font_size_override("font_size", 13)
	row.add_child(label)
	var bar := ProgressBar.new()
	bar.show_percentage = false
	bar.max_value = 1.0
	bar.value = value
	bar.custom_minimum_size = Vector2(BAR_WIDTH, 8)
	bar.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	var fill := StyleBoxFlat.new()
	# Unrest past the protest line turns to the danger color.
	fill.bg_color = UiTheme.DANGER if name == "Unrest" and value >= PROTEST_AT else color
	bar.add_theme_stylebox_override("fill", fill)
	var ground := StyleBoxFlat.new()
	ground.bg_color = UiTheme.UMBER
	bar.add_theme_stylebox_override("background", ground)
	row.add_child(bar)
	var number := Label.new()
	number.text = "%.2f" % value
	number.add_theme_font_size_override("font_size", 13)
	row.add_child(number)
	return row


func _left(label: Label) -> Label:
	label.horizontal_alignment = HORIZONTAL_ALIGNMENT_LEFT
	label.custom_minimum_size.x = 280
	return label
