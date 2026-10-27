class_name GameSettings
extends RefCounted

# Player settings (design, Screen flow: Settings), kept in user://settings.cfg.

const PATH := "user://settings.cfg"

var pause_on_great_events := true
var ui_scale := 1.0
var show_frame_time := false


static func load_or_default() -> GameSettings:
	var settings := GameSettings.new()
	var file := ConfigFile.new()
	if file.load(PATH) == OK:
		settings.pause_on_great_events = file.get_value("play", "pause_on_great_events", true)
		settings.ui_scale = clampf(file.get_value("display", "ui_scale", 1.0), 0.8, 1.5)
		settings.show_frame_time = file.get_value("display", "show_frame_time", false)
	return settings


func save() -> void:
	var file := ConfigFile.new()
	file.set_value("play", "pause_on_great_events", pause_on_great_events)
	file.set_value("display", "ui_scale", ui_scale)
	file.set_value("display", "show_frame_time", show_frame_time)
	file.save(PATH)
