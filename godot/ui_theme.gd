class_name UiTheme
extends RefCounted

# The one Godot Theme every screen uses (design, Visual direction and tokens:
# Kinetic Sand Glyphs). Built in code so the palette lives beside the rules
# that use it; antique gold is reserved for the Chosen and for focus.

const INK := Color("#121110")
const UMBER := Color("#2A1F17")
const OCHRE := Color("#B9853F")
const GOLD := Color("#D9A845")
const TEXT := Color("#EDE6DA")
const MUTED := Color("#A89A86")
const DANGER := Color("#C2553A")


static func build() -> Theme:
	var theme := Theme.new()
	theme.default_font_size = 15

	theme.set_color("font_color", "Label", TEXT)
	theme.set_color("font_shadow_color", "Label", Color(INK, 0.8))
	theme.set_constant("shadow_offset_x", "Label", 1)
	theme.set_constant("shadow_offset_y", "Label", 1)

	theme.set_stylebox("panel", "Panel", panel_box(UMBER, 0.94))
	theme.set_stylebox("panel", "PanelContainer", panel_box(UMBER, 0.94))

	theme.set_stylebox("normal", "Button", _button_box(Color(INK, 0.85), Color(OCHRE, 0.45)))
	theme.set_stylebox("hover", "Button", _button_box(UMBER, OCHRE))
	theme.set_stylebox("pressed", "Button", _button_box(Color(GOLD, 0.25), GOLD))
	theme.set_stylebox("focus", "Button", _button_box(Color(0, 0, 0, 0), GOLD))
	theme.set_stylebox("disabled", "Button", _button_box(Color(INK, 0.6), Color(MUTED, 0.25)))
	theme.set_color("font_color", "Button", TEXT)
	theme.set_color("font_hover_color", "Button", GOLD)
	theme.set_color("font_pressed_color", "Button", GOLD)
	theme.set_color("font_focus_color", "Button", TEXT)
	theme.set_color("font_disabled_color", "Button", Color(MUTED, 0.6))

	theme.set_stylebox("normal", "LineEdit", _button_box(INK, OCHRE))
	theme.set_stylebox("focus", "LineEdit", _button_box(INK, GOLD))
	theme.set_color("font_color", "LineEdit", TEXT)
	theme.set_color("caret_color", "LineEdit", GOLD)
	theme.set_color("font_placeholder_color", "LineEdit", Color(MUTED, 0.7))

	theme.set_color("font_color", "CheckButton", TEXT)
	theme.set_color("font_hover_color", "CheckButton", GOLD)
	theme.set_color("font_color", "CheckBox", TEXT)
	theme.set_color("font_hover_color", "CheckBox", GOLD)
	return theme


static func panel_box(color: Color, alpha: float) -> StyleBoxFlat:
	var box := StyleBoxFlat.new()
	box.bg_color = Color(color, alpha)
	box.border_color = Color(OCHRE, 0.5)
	box.set_border_width_all(1)
	box.set_corner_radius_all(2)
	box.set_content_margin_all(12)
	return box


static func _button_box(fill: Color, border: Color) -> StyleBoxFlat:
	var box := StyleBoxFlat.new()
	box.bg_color = fill
	box.border_color = border
	box.set_border_width_all(1)
	box.set_corner_radius_all(2)
	box.content_margin_left = 14
	box.content_margin_right = 14
	box.content_margin_top = 6
	box.content_margin_bottom = 6
	return box


# A title in the display size, gold for the game's own name.
static func heading(text: String, size: int, color: Color = TEXT) -> Label:
	var label := Label.new()
	label.text = text
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	return label


static func muted(text: String, size: int = 13) -> Label:
	var label := Label.new()
	label.text = text
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", MUTED)
	label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	return label


static func button(text: String, action: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(240, 38)
	b.pressed.connect(action)
	return b
