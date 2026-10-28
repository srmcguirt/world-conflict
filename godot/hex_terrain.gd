class_name HexTerrain
extends Node2D

# The world as meshes of hex columns (design, Rendering). Every tile is a
# pointy-top hexagon in odd-r rows, raised to a terraced height, with its two
# camera-facing sides dropping to the lower neighbor. Light is baked into the
# vertex colors here; terrain.gdshader adds the overlay color, borders, water
# and night from per-tile textures.
#
# The map is cut into full-width bands of BAND_ROWS rows, one MeshInstance2D
# each, added back band first. Columns only ever cover rows behind them, so
# bands in order keep the painter's order exact. When a power reshapes the
# ground only the bands near it rebuild, and Godot skips drawing bands
# outside the view.

const RADIUS := 16.0  # hex center to corner, in world pixels
const TILT := 0.6  # vertical foreshortening of the hex plane (three-quarter view)
const HEIGHT_STEPS := 10  # terraces between sea level and the highest peak
const STEP_PX := 7.0  # screen height of one terrace
const SEA_LEVEL := 0.5  # sim-core worldgen::SEA_LEVEL
const OCEAN := 0
const MOUNTAIN := 5
# Land within this many terraces of its neighbors slopes to meet them; a
# bigger drop, a mountain or the sea keeps a cliff.
const SLOPE_STEPS := 2
# Mountains slope only into other mountains, across a wider range, so a range
# reads as one massif with cliffs where it meets the lowland.
const MOUNTAIN_SLOPE_STEPS := 4
# A mountain's center rises into a peak this many pixels above its base, and an
# edge shared by two mountains rises into a ridge between their peaks.
const PEAK_PX := 18.0
const RIDGE_SHARE := 0.7
const SNOW_PX := 52.0  # height where snow starts on a mountain's faces
const SNOW_FADE_PX := 14.0

# Art bible light: from the upper left and above, toward the camera's far side.
const LIGHT := Vector3(-0.62, -0.55, 0.75)
const AMBIENT := 0.38
const DIFFUSE := 0.62
const SIDE_SW := 0.62  # lit side, as a share of a flat top's light
const SIDE_SE := 0.42  # shadow side

# Face kinds, written to COLOR.g for the shader.
# 0 to KIND_SNOW is a land top with that much snow; KIND_WATER is a water top.
const KIND_TOP := 0.0
const KIND_SNOW := 0.2
const KIND_WATER := 0.3
const KIND_SIDE := 0.5

const BAND_ROWS := 4
# A height change reaches this far: terraces soften over one ring, corners
# over two, ridges over three, and a side wall reads its neighbor's ridge.
const REACH := 4

# Odd-r neighbor offsets on the six sides E, SE, SW, W, NW, NE, for even rows
# and odd rows (sim-core hex.rs).
const EVEN_SIDES := [
	Vector2i(1, 0),
	Vector2i(0, 1),
	Vector2i(-1, 1),
	Vector2i(-1, 0),
	Vector2i(-1, -1),
	Vector2i(0, -1)
]
const ODD_SIDES := [
	Vector2i(1, 0),
	Vector2i(1, 1),
	Vector2i(0, 1),
	Vector2i(-1, 0),
	Vector2i(0, -1),
	Vector2i(1, -1)
]

var grid_w := 0
var grid_h := 0
var steps := PackedInt32Array()  # terrace per tile; -1 for water below the shore
var corner_lift := PackedFloat32Array()  # 6 per tile: each top corner's height in px
var center_lift := PackedFloat32Array()  # per tile: the center's height, a peak on mountains
# 6 per tile: each side's midpoint height, a ridge between mountains
var mid_lift := PackedFloat32Array()
var _biomes := PackedByteArray()
var _heights := PackedFloat32Array()
var _material: ShaderMaterial
var _bands: Array[MeshInstance2D] = []
var _ghosts: Array[MeshInstance2D] = []  # each band's copies one world west and east
var _pending: Array[int] = []  # bands still to build during loading
# Packed arrays straight into an ArrayMesh: much cheaper than SurfaceTool,
# which builds a vertex object per add_vertex call.
var _verts := PackedVector2Array()
var _colors := PackedColorArray()
var _uvs := PackedVector2Array()


func setup(width: int, height: int, color_tex: Texture2D, code_tex: Texture2D) -> void:
	grid_w = width
	grid_h = height
	_material = ShaderMaterial.new()
	_material.shader = load("res://terrain.gdshader")
	_material.set_shader_parameter("color_tex", color_tex)
	_material.set_shader_parameter("code_tex", code_tex)
	_material.set_shader_parameter("grid", Vector2(grid_w, grid_h))
	_material.set_shader_parameter("beacon_radius", 14.0 * RADIUS * 1.732)
	for b in ceili(float(grid_h) / float(BAND_ROWS)):
		var band := MeshInstance2D.new()
		band.material = _material
		band.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		add_child(band)
		_bands.append(band)
		# The world wraps east to west (design, Grid): a copy of every band
		# one world-width to each side shares its mesh, so the seam never
		# shows. Godot culls the copies that are out of view.
		for side in [-1.0, 1.0]:
			var ghost := MeshInstance2D.new()
			ghost.material = _material
			ghost.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
			ghost.position.x = side * world_width()
			add_child(ghost)
			_ghosts.append(ghost)


func set_chosen(culture_id: int) -> void:
	_material.set_shader_parameter("chosen_code", float(culture_id + 1))


# Takes the world's shape and queues every band; the loading screen calls
# build_next() until it reports 1.0, so the build spreads across frames.
func begin(heights: PackedFloat32Array, biomes: PackedByteArray) -> void:
	_heights = heights
	_biomes = biomes
	var count := heights.size()
	steps.resize(count)
	corner_lift.resize(count * 6)
	center_lift.resize(count)
	mid_lift.resize(count * 6)
	var all := range(count)
	_terrace(all)
	_slope_corners(all)
	_raise_peaks(all)
	_pending.assign(range(_bands.size()))


# The heights the terrain was last built from, for Sim.changed_tiles().
func heights() -> PackedFloat32Array:
	return _heights


# Builds one queued band and returns the share of the terrain that is done.
func build_next() -> float:
	if not _pending.is_empty():
		_build_band(_pending.pop_front())
	return 1.0 - float(_pending.size()) / float(_bands.size())


# Rebuilds only what a power reshaped (Raise, Lower, Meteor): the changed
# tiles, the tiles within REACH of them, and the bands those tiles sit in.
# Returns true when anything changed, so settlements can re-seat themselves.
func update_heights(
	changed: PackedInt32Array, heights: PackedFloat32Array, biomes: PackedByteArray
) -> bool:
	if changed.is_empty():
		return false
	_heights = heights
	_biomes = biomes
	var region := _around(changed)
	_terrace(region)
	_slope_corners(region)
	_raise_peaks(region)
	var dirty := {}
	for i in region:
		dirty[(i / grid_w) / BAND_ROWS] = true
	for b in dirty:
		_build_band(b)
	return true


# Every tile within REACH steps of a changed one, wrapping east to west.
func _around(changed: PackedInt32Array) -> PackedInt32Array:
	var mark := PackedByteArray()
	mark.resize(grid_w * grid_h)
	var region := PackedInt32Array()
	for tile in changed:
		var cx := tile % grid_w
		var cy := tile / grid_w
		for y in range(maxi(cy - REACH, 0), mini(cy + REACH + 1, grid_h)):
			for dx in range(-REACH - 1, REACH + 2):
				var i := y * grid_w + posmod(cx + dx, grid_w)
				if mark[i] == 0:
					mark[i] = 1
					region.append(i)
	region.sort()
	return region


# The tile's column top in world pixels.
func top(x: int, y: int) -> Vector2:
	var center := hex_center(x, y)
	return Vector2(center.x, center.y * TILT - center_lift[y * grid_w + x])


# One full lap of the world east to west, in world pixels.
func world_width() -> float:
	return RADIUS * sqrt(3.0) * float(grid_w)


static func hex_center(x: int, y: int) -> Vector2:
	var w := RADIUS * sqrt(3.0)
	return Vector2(w * (float(x) + 0.5 * float(y & 1)), RADIUS * 1.5 * float(y))


static func step_height(step: int) -> float:
	return float(step) * STEP_PX


# Pointy-top corner k, clockwise from the top, as an offset in the hex plane.
static func corner(k: int) -> Vector2:
	var angle := deg_to_rad(60.0 * float(k) - 90.0)
	return Vector2(cos(angle), sin(angle)) * RADIUS


# Odd-r neighbors on the six sides: E, SE, SW, W, NW, NE (sim-core hex.rs).
func neighbor(x: int, y: int, side: int) -> Vector2i:
	var d: Vector2i = ODD_SIDES[side] if y & 1 else EVEN_SIDES[side]
	var ny := y + d.y
	if ny < 0 or ny >= grid_h:
		return Vector2i(-1, -1)
	return Vector2i(posmod(x + d.x, grid_w), ny)


# Heights become terraces: land above sea level, softened toward the mean of
# its six neighbors so a lone spike becomes a ledge, then rounded. Water sits
# flat one step below the lowest shore.
func _terrace(tiles) -> void:
	for i in tiles:
		if _biomes[i] == OCEAN:
			steps[i] = -1
			continue
		var x: int = i % grid_w
		var y: int = i / grid_w
		var sum := 0.0
		var count := 0
		for side in 6:
			var n := neighbor(x, y, side)
			if n.x >= 0:
				sum += _land(n.y * grid_w + n.x)
				count += 1
		var soft := _land(i) * 0.55 + (sum / float(maxi(count, 1))) * 0.45
		# Peaks rise faster than plains: a gentle curve keeps lowlands
		# readable as one or two ledges while mountains climb.
		steps[i] = maxi(0, roundi(pow(soft, 1.35) * float(HEIGHT_STEPS)))


func _land(i: int) -> float:
	return clampf((_heights[i] - SEA_LEVEL) / (1.0 - SEA_LEVEL), 0.0, 1.0)


# Each top corner is shared by three tiles. Where all three are low land
# within SLOPE_STEPS of each other, the corner sits at their mean height, so
# the tops tilt into one another with no wall; otherwise every tile keeps its
# own height there and a cliff side fills the gap. Mountains and the sea never
# slope: they keep their ledges.
func _slope_corners(tiles) -> void:
	for i in tiles:
		for k in 6:
			corner_lift[i * 6 + k] = _corner_height(i % grid_w, i / grid_w, k)


func _corner_height(x: int, y: int, k: int) -> float:
	var i := y * grid_w + x
	var own := step_height(steps[i])
	if steps[i] < 0:
		return own
	var mountain := _biomes[i] == MOUNTAIN
	var low := steps[i]
	var high := steps[i]
	var total := steps[i]
	# Corner k joins the sides k - 2 and k - 1.
	for side in [(k + 4) % 6, (k + 5) % 6]:
		var n := neighbor(x, y, side)
		if n.x < 0:
			return own
		var j := n.y * grid_w + n.x
		if steps[j] < 0 or (_biomes[j] == MOUNTAIN) != mountain:
			return own
		low = mini(low, steps[j])
		high = maxi(high, steps[j])
		total += steps[j]
	if high - low > (MOUNTAIN_SLOPE_STEPS if mountain else SLOPE_STEPS):
		return own
	return step_height(total) / 3.0


# Peaks and ridges: a mountain's center rises by its own peak height, which
# varies by tile and grows with altitude; the midpoint of an edge between two
# mountains rises by a share of the lower peak, so neighboring peaks join in a
# ridge line and a lone mountain stands as a pointed summit.
func _raise_peaks(tiles) -> void:
	for i in tiles:
		var x: int = i % grid_w
		var y: int = i / grid_w
		var peak := _peak(i)
		center_lift[i] = step_height(steps[i]) + peak
		for side in 6:
			var edge := (
				(corner_lift[i * 6 + (side + 1) % 6] + corner_lift[i * 6 + (side + 2) % 6]) * 0.5
			)
			var n := neighbor(x, y, side)
			if n.x >= 0 and peak > 0.0:
				edge += RIDGE_SHARE * minf(peak, _peak(n.y * grid_w + n.x))
			mid_lift[i * 6 + side] = edge


func _peak(i: int) -> float:
	if _biomes[i] != MOUNTAIN or steps[i] < 0:
		return 0.0
	# A per-tile hash, so a range has taller and shorter summits.
	var roll := float((i * 2654435761) % 1000) / 1000.0
	var altitude := float(steps[i]) / float(HEIGHT_STEPS)
	return PEAK_PX * (0.55 + 0.45 * roll) * (0.6 + 0.6 * altitude)


func _build_band(b: int) -> void:
	_verts.clear()
	_colors.clear()
	_uvs.clear()
	# Back rows first: a nearer column covers whatever stands behind it.
	for y in range(b * BAND_ROWS, mini((b + 1) * BAND_ROWS, grid_h)):
		for x in grid_w:
			_emit_column(x, y)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = _verts
	arrays[Mesh.ARRAY_COLOR] = _colors
	arrays[Mesh.ARRAY_TEX_UV] = _uvs
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	_bands[b].mesh = mesh
	_ghosts[b * 2].mesh = mesh
	_ghosts[b * 2 + 1].mesh = mesh


func _emit_column(x: int, y: int) -> void:
	var i := y * grid_w + x
	var water := steps[i] < 0
	var center := hex_center(x, y)
	var peak := center_lift[i]
	var mountain := _biomes[i] == MOUNTAIN
	var uv := Vector2((float(x) + 0.5) / float(grid_w), (float(y) + 0.5) / float(grid_h))

	# Top: two facets per side, center to corner to the side's midpoint to the
	# next corner, so the shader knows how far each fragment sits from each
	# edge (COLOR.ba carries the offset inside the hex) and mountains can lift
	# the center into a peak and the midpoints into ridges. Each facet takes
	# the light its real tilt earns.
	for side in 6:
		var k1 := (side + 1) % 6
		var k2 := (side + 2) % 6
		var a := corner(k1)
		var b := corner(k2)
		var m := (a + b) * 0.5
		var ha := corner_lift[i * 6 + k1]
		var hb := corner_lift[i * 6 + k2]
		var hm := mid_lift[i * 6 + side]
		for facet in [[a, ha, m, hm], [m, hm, b, hb]]:
			var p: Vector2 = facet[0]
			var hp: float = facet[1]
			var q: Vector2 = facet[2]
			var hq: float = facet[3]
			var light := 0.92 if water else _facet_light(p, q, hp - peak, hq - peak)
			_vertex(
				_project(center, peak), uv, _top_color(light, Vector2.ZERO, peak, water, mountain)
			)
			_vertex(_project(center + p, hp), uv, _top_color(light, p, hp, water, mountain))
			_vertex(_project(center + q, hq), uv, _top_color(light, q, hq, water, mountain))

	# Camera-facing sides: south-east (corners 2-3) and south-west (3-4),
	# dropping from this tile's edge to the neighbor's matching edge through
	# the side's midpoint. A sloped edge meets its neighbor exactly and draws
	# nothing.
	for side in [1, 2]:
		var n := neighbor(x, y, side)
		var c1: int = side + 1
		var c2: int = side + 2
		var tops := [corner_lift[i * 6 + c1], mid_lift[i * 6 + side], corner_lift[i * 6 + c2]]
		var floor_lift := step_height(steps[i] - 3)
		var feet := [floor_lift, floor_lift, floor_lift]
		if n.x >= 0:
			var j := n.y * grid_w + n.x
			# This tile's corner side + 1 is the neighbor's corner side + 5,
			# and both share the side's midpoint.
			feet = [
				corner_lift[j * 6 + (side + 5) % 6],
				mid_lift[j * 6 + (side + 3) % 6],
				corner_lift[j * 6 + (side + 4) % 6],
			]
		for e in 3:
			feet[e] = minf(tops[e], feet[e])
		if tops[0] - feet[0] < 0.5 and tops[1] - feet[1] < 0.5 and tops[2] - feet[2] < 0.5:
			continue
		var a := corner(c1)
		var b := corner(c2)
		var points := [a, (a + b) * 0.5, b]
		var shade := (SIDE_SE if side == 1 else SIDE_SW) * (0.85 if water else 1.0)
		var top_color := Color(shade, KIND_SIDE, 0.5, 0.5)
		# The foot of a cliff sits in its own shadow.
		var foot_color := Color(shade * 0.72, KIND_SIDE, 0.5, 0.5)
		for e in 2:
			var ta := _project(center + points[e], tops[e])
			var tb := _project(center + points[e + 1], tops[e + 1])
			var fa := _project(center + points[e], feet[e])
			var fb := _project(center + points[e + 1], feet[e + 1])
			_vertex(ta, uv, top_color)
			_vertex(tb, uv, top_color)
			_vertex(fb, uv, foot_color)
			_vertex(ta, uv, top_color)
			_vertex(fb, uv, foot_color)
			_vertex(fa, uv, foot_color)


# A top vertex: baked light, its kind (water, or land with snow on high
# mountain faces) and its offset inside the hex.
func _top_color(light: float, offset: Vector2, lift: float, water: bool, mountain: bool) -> Color:
	var kind := KIND_WATER if water else KIND_TOP
	if mountain:
		kind = KIND_SNOW * clampf((lift - SNOW_PX) / SNOW_FADE_PX, 0.0, 1.0)
	return _local(light, kind, offset)


# One facet's light from its real normal: the hex center at height 0, the
# other two points at offsets a and b raised by da and db pixels.
func _facet_light(a: Vector2, b: Vector2, da: float, db: float) -> float:
	var normal := Vector3(a.x, a.y, da).cross(Vector3(b.x, b.y, db)).normalized()
	if normal.z < 0.0:
		normal = -normal
	var to_light := LIGHT.normalized()
	# A flat facet lands at the light the flat columns had.
	return AMBIENT + DIFFUSE * maxf(0.0, normal.dot(to_light)) / to_light.z * 0.85


func _local(light: float, kind: float, offset: Vector2) -> Color:
	var unit := offset / RADIUS
	return Color(light, kind, unit.x * 0.5 + 0.5, unit.y * 0.5 + 0.5)


func _project(plane: Vector2, lift: float) -> Vector2:
	return Vector2(plane.x, plane.y * TILT - lift)


func _vertex(at: Vector2, uv: Vector2, color: Color) -> void:
	_verts.append(at)
	_colors.append(color)
	_uvs.append(uv)


# Screen point (in this node's space) to tile. A raised column covers rows
# behind it, so rows nearer the camera are tested first.
func tile_at(at: Vector2) -> Vector2i:
	var point := Vector2(fposmod(at.x, world_width()), at.y)
	var row_px := RADIUS * 1.5 * TILT
	var max_lift := step_height(HEIGHT_STEPS)
	var first := clampi(int((point.y + max_lift) / row_px) + 1, 0, grid_h - 1)
	var last := clampi(int(point.y / row_px) - 2, 0, grid_h - 1)
	var col_w := RADIUS * sqrt(3.0)
	for y in range(first, last - 1, -1):
		var guess := int(floor(point.x / col_w - 0.5 * float(y & 1)))
		for x in [guess, guess + 1, guess - 1]:
			if x < 0 or x >= grid_w:
				continue
			if Geometry2D.is_point_in_polygon(point, _silhouette(x, y)):
				return Vector2i(x, y)
	return Vector2i(-1, -1)


# The column's outline on screen: its top hex plus the sides below it.
func _silhouette(x: int, y: int) -> PackedVector2Array:
	var i := y * grid_w + x
	var center := hex_center(x, y)
	var foot := steps[i]
	for side in [1, 2]:
		var n := neighbor(x, y, side)
		if n.x >= 0:
			foot = mini(foot, steps[n.y * grid_w + n.x])
	var drop := minf(step_height(foot), _lowest_corner(i))
	var outline := PackedVector2Array()
	for k in [4, 5, 0, 1, 2]:
		outline.append(_project(center + corner(k), corner_lift[i * 6 + k]))
	outline.append(_project(center + corner(2), drop))
	outline.append(_project(center + corner(3), drop))
	outline.append(_project(center + corner(4), drop))
	return outline


func _lowest_corner(i: int) -> float:
	var low := corner_lift[i * 6]
	for k in range(1, 6):
		low = minf(low, corner_lift[i * 6 + k])
	return low


# The top hex's outline, for the selection ring.
func hex_outline(x: int, y: int) -> PackedVector2Array:
	var i := y * grid_w + x
	var center := hex_center(x, y)
	var outline := PackedVector2Array()
	for k in 7:
		outline.append(_project(center + corner(k % 6), corner_lift[i * 6 + k % 6]))
	return outline
