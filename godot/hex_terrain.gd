class_name HexTerrain
extends MeshInstance2D

# The world as one mesh of hex columns (design, Rendering). Every tile is a
# pointy-top hexagon in odd-r rows, raised to a terraced height, with its two
# camera-facing sides dropping to the lower neighbor. Light is baked into the
# vertex colors here; terrain.gdshader adds the overlay color, borders, water
# and night from per-tile textures.

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

# Art bible light: from the upper left and above, toward the camera's far side.
const LIGHT := Vector3(-0.62, -0.55, 0.75)
const AMBIENT := 0.38
const DIFFUSE := 0.62
const SIDE_SW := 0.62  # lit side, as a share of a flat top's light
const SIDE_SE := 0.42  # shadow side

# Face kinds, written to COLOR.g for the shader.
const KIND_TOP := 0.0
const KIND_SIDE := 0.5

var grid_w := 0
var grid_h := 0
var steps := PackedInt32Array()  # terrace per tile; -1 for water below the shore
var corner_lift := PackedFloat32Array()  # 6 per tile: each top corner's height in px
var _biomes := PackedByteArray()
var _heights := PackedFloat32Array()


func setup(width: int, height: int, color_tex: Texture2D, code_tex: Texture2D) -> void:
	grid_w = width
	grid_h = height
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	var shader_material := ShaderMaterial.new()
	shader_material.shader = load("res://terrain.gdshader")
	shader_material.set_shader_parameter("color_tex", color_tex)
	shader_material.set_shader_parameter("code_tex", code_tex)
	shader_material.set_shader_parameter("grid", Vector2(grid_w, grid_h))
	shader_material.set_shader_parameter("beacon_radius", 14.0 * RADIUS * 1.732)
	material = shader_material


func set_chosen(culture_id: int) -> void:
	(material as ShaderMaterial).set_shader_parameter("chosen_code", float(culture_id + 1))


# Rebuilds the mesh only when the ground itself moved (Raise, Lower, Meteor).
func update_heights(heights: PackedFloat32Array, biomes: PackedByteArray) -> void:
	if heights == _heights:
		return
	_heights = heights
	_biomes = biomes
	_terrace(heights, biomes)
	_slope_corners()
	_build_mesh()


# The tile's column top in world pixels.
func top(x: int, y: int) -> Vector2:
	var center := hex_center(x, y)
	return Vector2(center.x, center.y * TILT - step_height(steps[y * grid_w + x]))


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
	var east := y & 1
	var offsets := [
		Vector2i(1, 0),
		Vector2i(east, 1),
		Vector2i(east - 1, 1),
		Vector2i(-1, 0),
		Vector2i(east - 1, -1),
		Vector2i(east, -1),
	]
	var d: Vector2i = offsets[side]
	var ny := y + d.y
	if ny < 0 or ny >= grid_h:
		return Vector2i(-1, -1)
	return Vector2i(posmod(x + d.x, grid_w), ny)


# Heights become terraces: land above sea level, softened toward the mean of
# its six neighbors so a lone spike becomes a ledge, then rounded. Water sits
# flat one step below the lowest shore.
func _terrace(heights: PackedFloat32Array, biomes: PackedByteArray) -> void:
	var land := PackedFloat32Array()
	land.resize(heights.size())
	for i in heights.size():
		land[i] = clampf((heights[i] - SEA_LEVEL) / (1.0 - SEA_LEVEL), 0.0, 1.0)
	steps.resize(heights.size())
	for y in grid_h:
		for x in grid_w:
			var i := y * grid_w + x
			if biomes[i] == OCEAN:
				steps[i] = -1
				continue
			var sum := 0.0
			var count := 0
			for side in 6:
				var n := neighbor(x, y, side)
				if n.x >= 0:
					sum += land[n.y * grid_w + n.x]
					count += 1
			var soft := land[i] * 0.55 + (sum / float(maxi(count, 1))) * 0.45
			# Peaks rise faster than plains: a gentle curve keeps lowlands
			# readable as one or two ledges while mountains climb.
			steps[i] = maxi(0, roundi(pow(soft, 1.35) * float(HEIGHT_STEPS)))


# Each top corner is shared by three tiles. Where all three are low land
# within SLOPE_STEPS of each other, the corner sits at their mean height, so
# the tops tilt into one another with no wall; otherwise every tile keeps its
# own height there and a cliff side fills the gap. Mountains and the sea never
# slope: they keep their ledges.
func _slope_corners() -> void:
	corner_lift.resize(steps.size() * 6)
	for y in grid_h:
		for x in grid_w:
			var i := y * grid_w + x
			for k in 6:
				corner_lift[i * 6 + k] = _corner_height(x, y, k)


func _corner_height(x: int, y: int, k: int) -> float:
	var i := y * grid_w + x
	var own := step_height(steps[i])
	if not _slopes(i):
		return own
	var low := steps[i]
	var high := steps[i]
	var total := steps[i]
	# Corner k joins the sides k - 2 and k - 1.
	for side in [(k + 4) % 6, (k + 5) % 6]:
		var n := neighbor(x, y, side)
		if n.x < 0:
			return own
		var j := n.y * grid_w + n.x
		if not _slopes(j):
			return own
		low = mini(low, steps[j])
		high = maxi(high, steps[j])
		total += steps[j]
	if high - low > SLOPE_STEPS:
		return own
	return step_height(total) / 3.0


func _slopes(i: int) -> bool:
	return steps[i] >= 0 and _biomes[i] != MOUNTAIN


func _build_mesh() -> void:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	# Back rows first: a nearer column covers whatever stands behind it.
	for y in grid_h:
		for x in grid_w:
			_emit_column(st, x, y)
	mesh = st.commit()


func _emit_column(st: SurfaceTool, x: int, y: int) -> void:
	var i := y * grid_w + x
	var step := steps[i]
	var water := step < 0
	var center := hex_center(x, y)
	var lift := step_height(step)
	var uv := Vector2((float(x) + 0.5) / float(grid_w), (float(y) + 0.5) / float(grid_h))
	var kind := KIND_TOP + (0.25 if water else 0.0)

	# Top: a fan from the center, so the shader knows how far each fragment
	# sits from each edge (COLOR.ba carries the offset inside the hex). Each
	# facet takes the light its real tilt earns.
	var mid := _project(center, lift)
	for k in 6:
		var k2 := (k + 1) % 6
		var a := corner(k)
		var b := corner(k2)
		var ha := corner_lift[i * 6 + k]
		var hb := corner_lift[i * 6 + k2]
		var light := 0.92 if water else _facet_light(a, b, ha - lift, hb - lift)
		_vertex(st, mid, uv, Color(light, kind, 0.5, 0.5))
		_vertex(st, _project(center + a, ha), uv, _local(light, kind, a))
		_vertex(st, _project(center + b, hb), uv, _local(light, kind, b))

	# Camera-facing sides: south-east (corners 2-3) and south-west (3-4),
	# dropping from this tile's edge to the neighbor's matching edge. A
	# sloped edge meets its neighbor exactly and draws nothing.
	for side in [1, 2]:
		var n := neighbor(x, y, side)
		var c1: int = side + 1
		var c2: int = side + 2
		var top1 := corner_lift[i * 6 + c1]
		var top2 := corner_lift[i * 6 + c2]
		var foot1 := step_height(step - 3)
		var foot2 := foot1
		if n.x >= 0:
			var j := n.y * grid_w + n.x
			# This tile's corner side + 1 is the neighbor's corner side + 5.
			foot1 = minf(top1, corner_lift[j * 6 + (side + 5) % 6])
			foot2 = minf(top2, corner_lift[j * 6 + (side + 4) % 6])
		if top1 - foot1 < 0.5 and top2 - foot2 < 0.5:
			continue
		var a := center + corner(c1)
		var b := center + corner(c2)
		var shade := (SIDE_SE if side == 1 else SIDE_SW) * (0.85 if water else 1.0)
		var top_color := Color(shade, KIND_SIDE, 0.5, 0.5)
		# The foot of a cliff sits in its own shadow.
		var foot_color := Color(shade * 0.72, KIND_SIDE, 0.5, 0.5)
		var ta := _project(a, top1)
		var tb := _project(b, top2)
		var fa := _project(a, foot1)
		var fb := _project(b, foot2)
		_vertex(st, ta, uv, top_color)
		_vertex(st, tb, uv, top_color)
		_vertex(st, fb, uv, foot_color)
		_vertex(st, ta, uv, top_color)
		_vertex(st, fb, uv, foot_color)
		_vertex(st, fa, uv, foot_color)


# One fan facet's light from its real normal: the center at height 0, corners
# a and b raised by da and db pixels.
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


func _vertex(st: SurfaceTool, at: Vector2, uv: Vector2, color: Color) -> void:
	st.set_color(color)
	st.set_uv(uv)
	st.add_vertex(Vector3(at.x, at.y, 0.0))


# Screen point (in this node's space) to tile. A raised column covers rows
# behind it, so rows nearer the camera are tested first.
func tile_at(point: Vector2) -> Vector2i:
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
