class_name SettlementView
extends Node2D

# One settlement as a cluster of small buildings standing on its tile
# (design, Rendering: settlements are buildings). Houses are boxes with a
# pitched roof, drawn with polygons in the same three-quarter view as the
# terrain: the plane is foreshortened by TILT and height lifts straight up.
# The parent layer y-sorts settlements; inside one, buildings draw back to front.

const TIER_CAMP := 0
const TIER_VILLAGE := 1
const TIER_TOWN := 2
const TIER_CITY := 3

const TILT := HexTerrain.TILT
const SIZE := 1.7  # building scale against the hex, tuned on the 4x screenshot
const LIGHT := Vector3(-0.62, -0.55, 0.75)  # art bible: upper left, from above
const GOLD := Color("#D9A845")
const INK := Color("#121110")

# Wall materials by biome (sim-core worldgen::Biome order).
const MATERIALS := [
	Color("#8E8A80"),  # ocean (unused; stone)
	Color("#C9B48A"),  # beach: pale mud brick
	Color("#B8956A"),  # grassland: mud brick
	Color("#8A6A48"),  # forest: timber
	Color("#D2B07A"),  # desert: sun-dried brick
	Color("#9A958B"),  # mountain: stone
	Color("#C8CCD0"),  # ice: packed snow and stone
]

var tier := 0
var wall := Color.WHITE
var roof := Color.WHITE
var chosen := false
var faithful := false
var seed_value := 0
var built := 0
var selected := false

# Each part: {kind, at (plane offset), w, d, h, yaw}.
var _parts: Array[Dictionary] = []


func configure(
	tile: int,
	tier_in: int,
	biome: int,
	dress: Color,
	is_chosen: bool,
	has_faith: bool,
	n_built: int
) -> void:
	tier = tier_in
	seed_value = tile
	built = n_built
	wall = MATERIALS[clampi(biome, 0, MATERIALS.size() - 1)]
	roof = dress.darkened(0.15)
	chosen = is_chosen
	faithful = has_faith
	_layout()
	queue_redraw()


func _layout() -> void:
	_parts.clear()
	var rng := RandomNumberGenerator.new()
	rng.seed = seed_value * 2654435761 + tier
	var r := HexTerrain.RADIUS
	var houses: int = [3, 4, 7, 10][clampi(tier, 0, 3)] + mini(built / 2, 3)
	var spread: float = [0.45, 0.6, 0.8, 0.95][clampi(tier, 0, 3)] * r
	var kind := "tent" if tier == TIER_CAMP else "house"
	# Houses face the hex's camera sides: yaw 30 degrees, like the columns.
	var yaw := deg_to_rad(30.0)
	if tier >= TIER_TOWN:
		_parts.append(
			{
				"kind": "tower",
				"at": Vector2.ZERO,
				"w": 3.2 * SIZE,
				"d": 3.2 * SIZE,
				"h": (9.0 + 2.0 * tier) * SIZE,
				"yaw": yaw
			}
		)
	if tier == TIER_CITY:
		_parts.append(
			{
				"kind": "tower",
				"at": Vector2(r * 0.55, -r * 0.35),
				"w": 2.6 * SIZE,
				"d": 2.6 * SIZE,
				"h": 11.0 * SIZE,
				"yaw": yaw
			}
		)
	var phase := rng.randf() * TAU
	for i in houses:
		# Sunflower spiral: even spacing without a grid look.
		var t := sqrt((float(i) + 0.6) / float(houses))
		var angle := phase + float(i) * 2.39996
		var at := Vector2(cos(angle), sin(angle)) * spread * t
		if tier >= TIER_TOWN and at.length() < 3.0 * SIZE:
			continue
		var w := (rng.randf_range(2.6, 3.6) + (0.6 if tier == TIER_CITY else 0.0)) * SIZE
		(
			_parts
			. append(
				{
					"kind": kind,
					"at": at,
					"w": w,
					"d": w * rng.randf_range(0.7, 0.9),
					"h": (rng.randf_range(2.6, 3.4) + (1.2 if tier == TIER_CITY else 0.0)) * SIZE,
					"yaw": yaw + (0.0 if rng.randf() < 0.75 else PI / 2.0),
				}
			)
		)
	# Back to front: smaller plane y is farther from the camera.
	_parts.sort_custom(func(a, b): return a["at"].y < b["at"].y)


func _draw() -> void:
	if tier == TIER_CITY:
		_draw_ring_wall(true)
	if selected:
		_draw_selection()
	var tallest := Vector2.ZERO
	var tallest_h := -1.0
	for part in _parts:
		match part["kind"]:
			"tent":
				_draw_tent(part)
			"tower":
				_draw_box(part, wall.darkened(0.08), roof.darkened(0.25), true)
			_:
				_draw_box(part, wall, roof, false)
		var top_h: float = part["h"] + part["w"] * 0.55
		if top_h > tallest_h:
			tallest_h = top_h
			tallest = part["at"]
	if tier == TIER_CITY:
		_draw_ring_wall(false)
	if faithful:
		var mark := _p(tallest, tallest_h + 3.0)
		draw_circle(mark, 1.3, Color("#EDE6DA"))
	if chosen:
		var base := _p(tallest + Vector2(1.0, 0.0), tallest_h)
		var tip := base + Vector2(0.0, -7.0)
		draw_line(base, tip, INK, 0.6)
		draw_colored_polygon(
			PackedVector2Array([tip, tip + Vector2(4.5, 1.2), tip + Vector2(0.0, 2.6)]), GOLD
		)


# Plane offset plus height to this node's screen space.
func _p(plane: Vector2, height: float) -> Vector2:
	return Vector2(plane.x, plane.y * TILT - height)


func _shade(base: Color, normal: Vector3) -> Color:
	var facing := maxf(0.0, normal.normalized().dot(LIGHT.normalized()))
	return base * (0.55 + 0.6 * facing)


func _draw_box(part: Dictionary, wall_color: Color, roof_color: Color, tower: bool) -> void:
	var at: Vector2 = part["at"]
	var yaw: float = part["yaw"]
	var u: Vector2 = Vector2(cos(yaw), sin(yaw)) * (part["w"] * 0.5)
	var v: Vector2 = Vector2(-sin(yaw), cos(yaw)) * (part["d"] * 0.5)
	var h: float = part["h"]
	var ridge: float = h + (part["w"] * 0.9 if tower else part["d"] * 0.9)
	# Footprint corners, counter-clockwise from +u+v.
	var c := [at + u + v, at - u + v, at - u - v, at + u - v]

	# Shadow on the ground, offset away from the light.
	var shadow := PackedVector2Array()
	for p in c:
		shadow.append(_p(p + Vector2(1.2, 1.0), 0.0))
	draw_colored_polygon(shadow, Color(0, 0, 0, 0.28))

	# Walls whose outward normal faces the camera (+y in the plane).
	var faces := [[0, 1, v], [1, 2, -u], [2, 3, -v], [3, 0, u]]
	for f in faces:
		var n: Vector2 = f[2]
		if n.y <= 0.0:
			continue
		var a: Vector2 = c[f[0]]
		var b: Vector2 = c[f[1]]
		var poly := PackedVector2Array([_p(a, 0.0), _p(b, 0.0), _p(b, h), _p(a, h)])
		var color := _shade(wall_color, Vector3(n.x, n.y, 0.0))
		draw_colored_polygon(poly, color)
		# Gable end under a pitched roof, on the faces along the ridge's ends.
		if not tower and (f[2] == u or f[2] == -u):
			var mid := (a + b) * 0.5
			draw_colored_polygon(
				PackedVector2Array([_p(a, h), _p(b, h), _p(mid, ridge)]), color.darkened(0.05)
			)
		# A door on the first lit face of a house.
		if not tower and n.y > 0.0 and (f[2] == v or f[2] == -v):
			var door := (a + b) * 0.5
			draw_line(_p(door, 0.0), _p(door, h * 0.55), INK.lerp(wall_color, 0.3), 0.9)

	if tower:
		# Pyramid cap.
		var apex := _p(at, ridge)
		for f in faces:
			var a2: Vector2 = c[f[0]]
			var b2: Vector2 = c[f[1]]
			var n2: Vector2 = f[2]
			var color2 := _shade(roof_color, Vector3(n2.x, n2.y, 1.2))
			draw_colored_polygon(PackedVector2Array([_p(a2, h), _p(b2, h), apex]), color2)
		return

	# Pitched roof, ridge along u: back slope first, then the front slope.
	var r0 := _p(at + u, ridge)
	var r1 := _p(at - u, ridge)
	var back: Vector2 = v if v.y < 0.0 else -v
	var front: Vector2 = -back
	for side in [back, front]:
		var e0 := _p(at + u + side * 1.12, h - 0.4)
		var e1 := _p(at - u + side * 1.12, h - 0.4)
		var normal := Vector3(side.x, side.y, 1.1)
		draw_colored_polygon(PackedVector2Array([e0, e1, r1, r0]), _shade(roof_color, normal))
	draw_line(r0, r1, roof_color.lightened(0.25), 0.6)


func _draw_tent(part: Dictionary) -> void:
	var at: Vector2 = part["at"]
	var r: float = part["w"] * 0.55
	var apex := _p(at, part["h"] + 1.4)
	draw_colored_polygon(
		PackedVector2Array(
			[
				_p(at + Vector2(-r, 0), 0.0),
				_p(at + Vector2(r, 0), 0.0),
				_p(at, 0.0) + Vector2(2.0, 1.0)
			]
		),
		Color(0, 0, 0, 0.25)
	)
	var hide := Color("#C9B48A").lerp(roof, 0.35)
	var left := PackedVector2Array(
		[_p(at + Vector2(-r, 0), 0.0), _p(at + Vector2(0, r), 0.0), apex]
	)
	var right := PackedVector2Array(
		[_p(at + Vector2(0, r), 0.0), _p(at + Vector2(r, 0), 0.0), apex]
	)
	draw_colored_polygon(left, _shade(hide, Vector3(-1, 1, 0.6)))
	draw_colored_polygon(right, _shade(hide, Vector3(1, 1, 0.6)))
	draw_line(_p(at + Vector2(0, r), 0.0), apex, INK.lerp(hide, 0.4), 0.5)


# A low ring wall around a city: the back half draws before the buildings,
# the front half after.
func _draw_ring_wall(back: bool) -> void:
	var r := HexTerrain.RADIUS * 1.05
	var height := 2.4
	for k in 6:
		var a := HexTerrain.corner(k) * (r / HexTerrain.RADIUS)
		var b := HexTerrain.corner((k + 1) % 6) * (r / HexTerrain.RADIUS)
		var normal := ((a + b) * 0.5).normalized()
		if (normal.y > 0.05) == back:
			continue
		var poly := PackedVector2Array([_p(a, 0.0), _p(b, 0.0), _p(b, height), _p(a, height)])
		draw_colored_polygon(poly, _shade(wall.darkened(0.1), Vector3(normal.x, normal.y, 0.2)))
		draw_line(_p(a, height), _p(b, height), wall.lightened(0.15), 0.8)


func _draw_selection() -> void:
	var ring := PackedVector2Array()
	for k in 7:
		ring.append(_p(HexTerrain.corner(k % 6) * 1.2, 0.0))
	draw_polyline(ring, GOLD, 1.2, true)
