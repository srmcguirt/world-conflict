extends Node2D

# Look-dev spike for the isometric renderer (docs/graphics.md, docs/art-bible.md).
# Builds the real terrain mesh from the sim's tile data, flies the camera, and
# captures the gate screenshots. Not the game shell: main.gd keeps the old
# top-down map until the renderer is proven.
#
# Run: godot --path godot lookdev.tscn -- --years 2000 [--shots]

const TILE_W := 34.0
const TILE_H := 17.0
const HEIGHT_SCALE := 26.0
const WALL_CUSTOM := Color(0.0, 0.0, 0.0, 0.0)

var sim: Sim
var mesh_instance: MeshInstance2D
var camera: Camera2D
var terrain_material: ShaderMaterial

var heights: PackedFloat32Array
var biomes: PackedByteArray
var cultures: PackedInt32Array
var colors: PackedByteArray
var grid_w := 0
var grid_h := 0
var chosen := -1

var zoom := 1.0
var dragging := false
var drag_origin := Vector2.ZERO

var shots := false
var shot_queue: Array[Dictionary] = []
var shot_dir := ""


func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	shots = args.has("--shots")
	var years := 2000
	var years_at := args.find("--years")
	if years_at >= 0 and years_at + 1 < args.size():
		years = int(args[years_at + 1])
	shot_dir = ProjectSettings.globalize_path("res://../docs/designs/")

	sim = Sim.create(1)
	add_child(sim)
	grid_w = sim.grid_width()
	grid_h = sim.grid_height()
	sim.step(years)
	chosen = sim.chosen_culture()

	heights = sim.tile_heights()
	biomes = sim.tile_biomes()
	cultures = sim.tile_cultures()
	var overlay := 0  # biome: the natural world is the default look
	var overlay_at := args.find("--overlay")
	if overlay_at >= 0 and overlay_at + 1 < args.size():
		overlay = int(args[overlay_at + 1])
	colors = sim.paint_overlay(overlay)

	_build_terrain()
	_build_camera()
	if shots:
		_queue_shots()
	print(
		"[lookdev] year %d, %dx%d tiles, chosen culture %d" % [sim.tick(), grid_w, grid_h, chosen]
	)


func _build_terrain() -> void:
	terrain_material = ShaderMaterial.new()
	terrain_material.shader = load("res://terrain.gdshader")
	# Uniform defaults do not always survive the load; set them explicitly.
	terrain_material.set_shader_parameter("beacon_radius", 14.0 * TILE_W)
	terrain_material.set_shader_parameter("beacon_pos", Vector2.ZERO)
	terrain_material.set_shader_parameter("night", 0.0)

	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)

	for y in grid_h:
		for x in grid_w:
			var i: int = y * grid_w + x
			var h: float = heights[i]
			var water: int = 1 if biomes[i] == 0 or biomes[i] == 1 else 0
			var is_chosen: int = 1 if cultures[i] == chosen else 0
			var col := Color(
				colors[i * 4] / 255.0, colors[i * 4 + 1] / 255.0, colors[i * 4 + 2] / 255.0
			)
			var hashv: float = float((x * 73 + y * 151) % 97) / 97.0

			# Height AO: how far this tile sits below its neighbors' mean.
			var ao := 0.0
			var n := 0
			var nb_sum := 0.0
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= 0 and ny >= 0 and nx < grid_w and ny < grid_h:
					nb_sum += heights[ny * grid_w + nx]
					n += 1
			if n > 0:
				ao = clampf((nb_sum / float(n) - h) * 0.35, 0.0, 0.25)

			# Rim: a territory edge, or the coast.
			var rim := 0.0
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx < 0 or ny < 0 or nx >= grid_w or ny >= grid_h:
					continue
				var ni: int = ny * grid_w + nx
				if cultures[ni] != cultures[i] and (cultures[i] != -1 or cultures[ni] != -1):
					rim = maxf(rim, 0.5 if cultures[i] == chosen else 0.3)
				if biomes[ni] == 0 and water == 0:
					rim = maxf(rim, 0.35)
				if biomes[ni] != 0 and water == 1:
					rim = maxf(rim, 0.35)

			# Analytic normal from the height gradient: light has something to
			# bite on without authoring normal maps (art bible: one light).
			var gx: float = heights[mini(i + 1, heights.size() - 1)] - heights[maxi(i - 1, 0)]
			var gy: float = (
				heights[mini(i + grid_w, heights.size() - 1)] - heights[maxi(i - grid_w, 0)]
			)
			var normal := Vector3(-gx * 3.0, -gy * 3.0, 1.0).normalized()

			# Per-tile data rides in COLOR.a (hash) and UV (flags, AO, rim):
			# Godot's custom-channel API caps formats, and these are free.
			col.a = hashv
			_quad(st, iso(x, y, h), col, normal, float(water + is_chosen * 2), ao, rim)

			# Cliff walls toward higher neighbors: this is what makes the world
			# read as raised relief rather than flat diamonds.
			for d in [Vector2i(0, 1), Vector2i(1, 0)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= grid_w or ny >= grid_h:
					continue
				var nh: float = heights[ny * grid_w + nx]
				if nh > h + 0.02 and water == 0:
					var a := iso(x, y, h)
					var b := iso(x + d.x, y + d.y, h)
					var c := iso(x + d.x, y + d.y, nh)
					var dd := iso(x, y, nh)
					var wall_col := col.darkened(0.42)
					wall_col.a = 0.0  # walls: no hash
					st.set_color(wall_col)
					st.set_normal(Vector3(float(-d.y), float(-d.x), 0.4).normalized())
					st.set_uv(Vector2(4.0, 0.0))  # flags 4 = wall face
					_tri(st, a, b, c)
					_tri(st, a, c, dd)

	var array_mesh := st.commit()
	mesh_instance = MeshInstance2D.new()
	mesh_instance.mesh = array_mesh
	mesh_instance.material = terrain_material
	add_child(mesh_instance)


func iso(x: int, y: int, h: float) -> Vector2:
	# Odd-r flat-top: odd rows sit half a tile right. The hex grid reads as a
	# diamond grid whose odd rows are shifted, which is how every hex iso
	# game renders on a 2:1 camera.
	var sx := float(x - y) * TILE_W / 2.0
	var sy := float(x + y) * TILE_H / 2.0
	if y % 2 == 1:
		sx += TILE_W / 4.0
	return Vector2(sx, sy - h * HEIGHT_SCALE)


func _quad(
	st: SurfaceTool,
	center: Vector2,
	col: Color,
	normal: Vector3,
	flags: float,
	ao: float,
	rim: float
) -> void:
	var hw := TILE_W / 2.0
	var hh := TILE_H / 2.0
	var pts := [
		Vector2(center.x, center.y - hh),
		Vector2(center.x + hw, center.y),
		Vector2(center.x, center.y + hh),
		Vector2(center.x - hw, center.y)
	]
	st.set_color(col)
	st.set_normal(normal)
	st.set_uv(Vector2(flags, round(ao * 100.0) + round(rim * 10.0) * 100.0))
	for k in [0, 1, 2, 0, 2, 3]:
		st.add_vertex(Vector3(pts[k].x, pts[k].y, 0.0))


func _tri(st: SurfaceTool, a: Vector2, b: Vector2, c: Vector2) -> void:
	st.add_vertex(Vector3(a.x, a.y, 0.0))
	st.add_vertex(Vector3(b.x, b.y, 0.0))
	st.add_vertex(Vector3(c.x, c.y, 0.0))


func _build_camera() -> void:
	camera = Camera2D.new()
	camera.zoom = Vector2(zoom, zoom)
	add_child(camera)
	# Frame the Chosen's lands: the mean of their tiles.
	var sx := 0.0
	var sy := 0.0
	var count := 0
	for i in cultures.size():
		if cultures[i] == chosen:
			sx += float(i % grid_w)
			sy += float(i / grid_w)
			count += 1
	if count > 0:
		camera.position = iso(int(sx / float(count)), int(sy / float(count)), 0.5)


func _process(delta: float) -> void:
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
		camera.position += move.normalized() * 400.0 * delta / zoom
	if shots:
		_process_shots()


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_set_zoom(zoom * 1.12)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_set_zoom(zoom / 1.12)
		elif event.button_index == MOUSE_BUTTON_LEFT and event.pressed:
			dragging = true
			drag_origin = camera.position - event.position
		elif not event.pressed:
			dragging = false
	elif event is InputEventMouseMotion and dragging:
		camera.position = drag_origin + event.position


func _set_zoom(z: float) -> void:
	zoom = clampf(z, 0.5, 4.0)
	camera.zoom = Vector2(zoom, zoom)


# --- screenshot mode: the art bible's gates, captured from a real run ---


func _queue_shots() -> void:
	shot_queue = [
		{"name": "lookdev-map-05x", "zoom": 0.5, "night": 0.0},
		{"name": "lookdev-close-4x", "zoom": 4.0, "night": 0.0},
		{"name": "lookdev-night-2x", "zoom": 2.0, "night": 1.0},
	]


func _process_shots() -> void:
	if shot_queue.is_empty():
		return
	if Engine.get_process_frames() % 20 != 19:
		return
	var shot: Dictionary = shot_queue.pop_front()
	_set_zoom(shot["zoom"])
	terrain_material.set_shader_parameter("night", shot["night"])
	print(
		(
			"[lookdev] night now %s radius %s"
			% [
				terrain_material.get_shader_parameter("night"),
				terrain_material.get_shader_parameter("beacon_radius")
			]
		)
	)
	# The beacon sits at the Chosen's capital: the mean of their tiles.
	terrain_material.set_shader_parameter(
		"beacon_pos", mesh_instance.to_local(camera.global_position)
	)
	var img := get_viewport().get_texture().get_image()
	var path := "%s%s.png" % [shot_dir, shot["name"]]
	var err := img.save_png(path)
	print("[lookdev] shot %s (err %d)" % [shot["name"], err])
	if shot_queue.is_empty():
		get_tree().quit()
