extends Node

## Milestone 1 shell: generated biome map, speed control, repaint when dirty (R6).

const OVERLAY_BIOME := 0
## Years per second at each speed step (design: 1x = 1 year per second).
const SPEEDS := [1, 5, 25, 100]
## Sim time budget per frame, in milliseconds (design, Frame and tick scheduling).
const TICK_BUDGET_MS := 8.0

var sim: Sim
var image: Image
var texture: ImageTexture
var label: Label
var last_revision := -1
var speed_index := 0
var paused := false
var year_accumulator := 0.0


func _ready() -> void:
	sim = Sim.create(1)
	add_child(sim)
	image = Image.create(sim.grid_width(), sim.grid_height(), false, Image.FORMAT_RGBA8)
	texture = ImageTexture.create_from_image(image)
	var map := TextureRect.new()
	map.texture = texture
	map.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	map.set_anchors_preset(Control.PRESET_FULL_RECT)
	add_child(map)
	label = Label.new()
	label.set_anchors_preset(Control.PRESET_TOP_LEFT)
	label.position = Vector2(8, 4)
	add_child(label)
	var grid_text := "%d x %d" % [sim.grid_width(), sim.grid_height()]
	print("[godot] sim ready: year %d, %s tiles" % [sim.tick(), grid_text])
	repaint()


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
	label.text = (
		"year %d · %s · %d years/s"
		% [sim.tick(), "paused" if paused else "running", SPEEDS[speed_index]]
	)


func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_SPACE:
				paused = not paused
			KEY_BRACKETLEFT:
				speed_index = maxi(speed_index - 1, 0)
			KEY_BRACKETRIGHT:
				speed_index = mini(speed_index + 1, SPEEDS.size() - 1)


func repaint_if_dirty() -> void:
	if sim.revision() == last_revision:
		return
	repaint()


func repaint() -> void:
	var bytes := sim.paint_overlay(OVERLAY_BIOME)
	image.set_data(sim.grid_width(), sim.grid_height(), false, Image.FORMAT_RGBA8, bytes)
	texture.update(image)
	last_revision = sim.revision()
