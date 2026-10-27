class_name EventLog
extends PanelContainer

# The event log strip along the bottom edge (design, D11 and Screen flow): one
# chronological feed of the Chosen's verses and kingdom events. Collapsed it is
# one line, the newest entry, with a gold pip when something new arrives; `L`
# expands it to the recent entries with the people's header and the newest
# witness's portrait. A kingdom event flies the camera to where it happened.

signal focus_requested(tile: int)

const FEED_LENGTH := 7
# Protests already write a verse, so the feed shows the verse, not both.
const SKIPPED_KINDS := ["protest"]

var expanded := false
var _sim: Sim
var _culture := -1
var _events: Array[Dictionary] = []  # the Chosen's kingdom events, oldest first
var _dropped := 0
var _newest_key := ""
var _unseen := false

var _pip: ColorRect
var _line: Label
var _header: Label
var _list: VBoxContainer
var _portrait: TextureRect
var _body: HBoxContainer


func setup(sim: Sim) -> void:
	_sim = sim
	add_theme_stylebox_override("panel", UiTheme.panel_box(UiTheme.INK, 0.88))
	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 6)
	add_child(column)

	var top := HBoxContainer.new()
	top.add_theme_constant_override("separation", 8)
	column.add_child(top)
	_pip = ColorRect.new()
	_pip.color = UiTheme.GOLD
	_pip.custom_minimum_size = Vector2(6, 6)
	_pip.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_pip.visible = false
	top.add_child(_pip)
	_line = Label.new()
	_line.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_line.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	_line.clip_text = true
	top.add_child(_line)
	var hint := UiTheme.muted("L  log", 12)
	hint.autowrap_mode = TextServer.AUTOWRAP_OFF
	top.add_child(hint)

	_body = HBoxContainer.new()
	_body.add_theme_constant_override("separation", 12)
	_body.visible = false
	column.add_child(_body)
	_portrait = TextureRect.new()
	_portrait.custom_minimum_size = Vector2(48, 64)
	_portrait.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	_portrait.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_body.add_child(_portrait)
	var text := VBoxContainer.new()
	text.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_body.add_child(text)
	_header = Label.new()
	_header.add_theme_color_override("font_color", UiTheme.GOLD)
	text.add_child(_header)
	_list = VBoxContainer.new()
	text.add_child(_list)


func set_culture(culture_id: int) -> void:
	_culture = culture_id
	_newest_key = ""
	refresh(true)


# Takes the kingdom events the game drained this frame; only the Chosen's
# reach the feed (design, D11). `dropped` comes from the sim's queue (R4).
func add_events(events: Array, dropped: int) -> void:
	for event in events:
		if event["culture"] != _culture or SKIPPED_KINDS.has(event["kind"]):
			continue
		# A year's finished buildings fold into one line, so building never
		# crowds the people's verses out of the feed.
		var last: Dictionary = _events.back() if not _events.is_empty() else {}
		if event["kind"] == "building" and last.get("kind") == "building" and last["tick"] == event["tick"]:
			last["count"] += 1
			last["text"] = "%d buildings finished, among them %s" % [last["count"], last["first"]]
			continue
		var entry: Dictionary = event.duplicate()
		if event["kind"] == "building":
			entry["count"] = 1
			entry["first"] = event["text"]
		_events.append(entry)
	_dropped += dropped
	if _events.size() > FEED_LENGTH * 4:
		_events = _events.slice(_events.size() - FEED_LENGTH * 4)


func toggle() -> void:
	expanded = not expanded
	_body.visible = expanded
	_unseen = false
	_pip.visible = false
	refresh(true)


func refresh(force: bool = false) -> void:
	if _culture < 0:
		_line.text = "No people yet. The first will rise by the river."
		return
	var feed := _feed()
	var key: String = feed[0]["text"] if not feed.is_empty() else ""
	var fresh := key != _newest_key
	if fresh and _newest_key != "" and not expanded:
		_unseen = true
	_newest_key = key
	_pip.visible = _unseen
	if feed.is_empty():
		_line.text = "year %d" % _sim.tick()
	else:
		_line.text = "year %d · %s" % [feed[0]["tick"], feed[0]["text"]]
	if not expanded or not (fresh or force):
		return
	_header.text = _people_line()
	for child in _list.get_children():
		child.queue_free()
	for entry in feed:
		_list.add_child(_entry_row(entry))
	if _dropped > 0:
		_list.add_child(UiTheme.muted("and %d more events" % _dropped, 12))
	_show_portrait()


# Verses and kingdom events merged by year, newest first.
func _feed() -> Array[Dictionary]:
	var feed: Array[Dictionary] = []
	var verses: PackedStringArray = _sim.scripture(_culture, 0, FEED_LENGTH)
	var years: PackedInt64Array = _sim.scripture_years(_culture, 0, FEED_LENGTH)
	for i in verses.size():
		feed.append({"tick": years[i], "text": verses[i], "tile": -1, "order": i})
	for i in _events.size():
		var event: Dictionary = _events[i].duplicate()
		event["order"] = FEED_LENGTH + _events.size() - i  # newer events first
		feed.append(event)
	# Newest year first; within a year, verses before the events beside them.
	feed.sort_custom(
		func(a, b) -> bool:
			return a["tick"] > b["tick"] or (a["tick"] == b["tick"] and a["order"] < b["order"])
	)
	return feed.slice(0, FEED_LENGTH)


func _entry_row(entry: Dictionary) -> Control:
	if entry["tile"] < 0:
		var verse := Label.new()
		verse.text = entry["text"]
		verse.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		return verse
	var event := LinkButton.new()
	event.underline = LinkButton.UNDERLINE_MODE_ON_HOVER
	event.focus_mode = Control.FOCUS_NONE
	event.text = "year %d · %s" % [entry["tick"], entry["text"]]
	event.add_theme_color_override("font_color", UiTheme.OCHRE)
	event.add_theme_color_override("font_hover_color", UiTheme.GOLD)
	event.tooltip_text = "Show where it happened"
	var tile: int = entry["tile"]
	event.pressed.connect(func() -> void: focus_requested.emit(tile))
	return event


func _people_line() -> String:
	var doctrine: String = _sim.culture_doctrine(_culture)
	var lineage: PackedStringArray = _sim.tongue_lineage(_culture)
	var tongue := ""
	if lineage.size() == 1:
		tongue = " · speaks %s" % lineage[0]
	elif lineage.size() > 1:
		tongue = " · speaks %s of the %s line" % [lineage[lineage.size() - 1], lineage[0]]
	return (
		"%s · %s · piety %.2f%s"
		% [
			_sim.culture_name(_culture),
			doctrine if doctrine != "" else "no doctrine",
			_sim.culture_piety(_culture),
			tongue,
		]
	)


func _show_portrait() -> void:
	var id: int = _sim.newest_witness_id()
	var bytes: PackedByteArray = _sim.portrait(id) if id >= 0 else PackedByteArray()
	if bytes.is_empty():
		_portrait.texture = null
		return
	var w: int = _sim.portrait_width()
	var h: int = _sim.portrait_height()
	var image := Image.create_from_data(w, h, false, Image.FORMAT_RGBA8, bytes)
	_portrait.texture = ImageTexture.create_from_image(image)
