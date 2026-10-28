class_name SaveStore
extends RefCounted

# Where saved worlds live on disk (design, Screen flow: Continue; R12). One
# world slot, written by Save and by leaving a world, plus three rolling
# autosaves. Continue loads whichever is newest. The bytes come from Rust
# (Sim.save_bytes); this file only names, rotates and finds them.

const DIR := "user://saves"
const WORLD := "world.wcsave"
const AUTOSAVES := ["autosave-1.wcsave", "autosave-2.wcsave", "autosave-3.wcsave"]
# R12: an autosave when 100 sim-years have passed and at least 60 real seconds.
const AUTOSAVE_YEARS := 100
const AUTOSAVE_SECONDS := 60.0


static func save_world(sim: Sim) -> bool:
	return _write(WORLD, sim.save_bytes())


# Rolls the autosaves: the oldest drops, the rest move down one.
static func autosave(sim: Sim) -> bool:
	var bytes: PackedByteArray = sim.save_bytes()
	if bytes.is_empty():
		return false
	_ensure_dir()
	for i in range(AUTOSAVES.size() - 1, 0, -1):
		var older := _path(AUTOSAVES[i - 1])
		if FileAccess.file_exists(older):
			DirAccess.rename_absolute(older, _path(AUTOSAVES[i]))
	return _write(AUTOSAVES[0], bytes)


# The newest save on disk as {path, header}, or {} when there is none.
static func newest() -> Dictionary:
	var best := {}
	var best_time := -1
	for name in [WORLD] + AUTOSAVES:
		var path := _path(name)
		if not FileAccess.file_exists(path):
			continue
		var time := FileAccess.get_modified_time(path)
		if time > best_time:
			best_time = time
			best = {"path": path, "header": Sim.save_header(read(path))}
	return best


# The newest autosave's path, or "" (R12: the crash dialog's Load).
static func newest_autosave() -> String:
	for name in AUTOSAVES:
		if FileAccess.file_exists(_path(name)):
			return _path(name)
	return ""


static func read(path: String) -> PackedByteArray:
	return FileAccess.get_file_as_bytes(path)


static func _write(name: String, bytes: PackedByteArray) -> bool:
	if bytes.is_empty():
		return false
	_ensure_dir()
	# Write beside the slot, then swap, so a crash mid-write never leaves a
	# half-written save where a good one was.
	var temp := _path(name + ".tmp")
	var file := FileAccess.open(temp, FileAccess.WRITE)
	if file == null:
		return false
	file.store_buffer(bytes)
	file.close()
	return DirAccess.rename_absolute(temp, _path(name)) == OK


static func _ensure_dir() -> void:
	DirAccess.make_dir_recursive_absolute(DIR)


static func _path(name: String) -> String:
	return "%s/%s" % [DIR, name]
