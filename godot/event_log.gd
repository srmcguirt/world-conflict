class_name EventLog
extends PanelContainer

# The event log strip along the bottom edge (design, D11 and Screen flow):
# collapsed it is one line, the newest verse with a gold pip when something
# new arrives; `L` expands it to the recent verses with the people's header,
# their tongue and the newest witness's portrait.

const COLLAPSED_LINES := 1
const EXPANDED_VERSES := 10

var expanded := false
var _sim: Sim
var _culture := -1
var _last_verse := ""
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
	_last_verse = ""
	refresh()


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
	var verses: PackedStringArray = _sim.scripture(_culture, 0, EXPANDED_VERSES)
	var newest: String = verses[0] if verses.size() > 0 else ""
	var fresh := newest != _last_verse
	if fresh and _last_verse != "" and not expanded:
		_unseen = true
	_last_verse = newest
	_pip.visible = _unseen
	_line.text = "year %d · %s" % [_sim.tick(), newest] if newest != "" else "year %d" % _sim.tick()
	if not expanded or not (fresh or force):
		return
	_header.text = _people_line()
	for child in _list.get_children():
		child.queue_free()
	for verse in verses:
		var label := Label.new()
		label.text = verse
		label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		_list.add_child(label)
	_show_portrait()


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
