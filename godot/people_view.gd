class_name PeopleView
extends Node2D

# The people on the map (D29): each walking band is a small cluster of figures
# in its people's dress and skin, more figures for a bigger band, with a
# fading trail of the steps it walked; townsfolk stand along the front of each
# settlement, more as it grows. Taller peoples stand taller (design,
# Adaptation). One node draws every figure near the camera, shifted a world
# lap east or west to the copy nearest the view, so it needs no ghost copies.
# Zoomed out to map scale a band is one dot in its dress color and townsfolk
# are hidden; the buildings carry the town (art bible, readability gate 1).

const PER_FIGURE := 8.0  # walkers one band figure stands for
const BAND_FIGURES := 7
const TOWN_FIGURES := 6
const TRAIL := 12  # past steps drawn behind a band
const MAP_ZOOM := 0.8  # below this zoom a band is a dot
const TILT := HexTerrain.TILT
const INK := Color("#121110")
# World pixels per sprite pixel: the person, crown to heel 45.5 sprite pixels,
# stands 6.5 world pixels tall at mean stature, a little above a house's eaves.
const FIGURE_PX := 6.5 / 45.5
# Plane radius of a settlement's houses by tier, as in SettlementView.
const TOWN_SPREAD := [0.45, 0.6, 0.8, 0.95]

var terrain: HexTerrain
var grid_w := 0
var zoom := 1.0
var grow := 1.0  # the settlements' zoom scale, so people keep pace with houses
var center := Vector2.ZERO
var half_view := Vector2.ZERO

# band id -> {tile, from, to, since, culture, figures, trail}
var _bands := {}
var _towns: Array[Dictionary] = []  # {at, culture, figures, spread, phase}
var _looks := {}  # culture id -> [dress, skin, height]
var _step_seconds := 0.8
var _sprites := FigureSprites.new()


func setup(map: HexTerrain, width: int) -> void:
	terrain = map
	grid_w = width
	texture_filter = CanvasItem.TEXTURE_FILTER_LINEAR_WITH_MIPMAPS


# Called after each batch of ticks with the settlement sites (stride
# Game.SITE_STRIDE) and the play speed in years per second.
func refresh(sim: Sim, sites: PackedInt32Array, years_per_second: float) -> void:
	_step_seconds = clampf(1.0 / years_per_second, 0.05, 0.8)
	_looks.clear()
	_refresh_bands(sim)
	_towns.clear()
	for s in range(0, sites.size(), Game.SITE_STRIDE):
		var people: int = sites[s + 4]
		if people < 1:
			continue
		var tile: int = sites[s]
		var culture: int = sites[s + 2]
		_look(sim, culture)
		# One figure for the first 8 people, one more each time they double.
		var figures := clampi(1 + int(log(maxf(people / 8.0, 1.0)) / log(2.0)), 1, TOWN_FIGURES)
		(
			_towns
			. append(
				{
					"at": terrain.top(tile % grid_w, tile / grid_w),
					"culture": culture,
					"figures": figures,
					"spread": TOWN_SPREAD[clampi(sites[s + 1], 0, 3)] * HexTerrain.RADIUS,
					"phase": float(tile % 97),
				}
			)
		)


# Called every frame with the camera's view, so only nearby people draw.
func watch(view_center: Vector2, view_half: Vector2) -> void:
	center = view_center
	half_view = view_half
	queue_redraw()


# The walking band nearest `at`, for the screenshot tour; `at` if none walks.
func nearest_band(at: Vector2) -> Vector2:
	var best := at
	for band in _bands.values():
		var spot := _near(band["to"], at.x)
		if best == at or spot.distance_to(at) < best.distance_to(at):
			best = spot
	return best


func _refresh_bands(sim: Sim) -> void:
	var now := _now()
	var bands: PackedInt32Array = sim.band_sites()
	var seen := {}
	for b in range(0, bands.size(), 4):
		var id: int = bands[b]
		var tile: int = bands[b + 1]
		var culture: int = bands[b + 2]
		seen[id] = true
		_look(sim, culture)
		var at := terrain.top(tile % grid_w, tile / grid_w)
		var band: Dictionary = _bands.get(id, {})
		if band.is_empty():
			band = {"tile": tile, "from": at, "to": at, "since": now, "trail": PackedVector2Array()}
			_bands[id] = band
		else:
			# Positions stay continuous across the east-west seam, so a walk
			# and its trail never jump a whole lap.
			at = _near(at, band["to"].x)
			if tile != band["tile"]:
				var trail: PackedVector2Array = band["trail"]
				trail.append(band["to"])
				if trail.size() > TRAIL:
					trail.remove_at(0)
				band["trail"] = trail
				band["from"] = _walked(band, now)
				band["since"] = now
				band["tile"] = tile
			band["to"] = at
		band["culture"] = culture
		band["figures"] = clampi(ceili(bands[b + 3] / PER_FIGURE), 1, BAND_FIGURES)
	for id in _bands.keys():
		if not seen.has(id):
			_bands.erase(id)


func _look(sim: Sim, culture: int) -> void:
	if _looks.has(culture):
		return
	# Stature 0..1 makes a figure 85% to 115% of the mean height.
	var height := 0.85 + 0.3 * sim.culture_stature(culture)
	_looks[culture] = [sim.culture_dress(culture), sim.culture_skin(culture), height]


func _draw() -> void:
	var now := _now()
	var map_scale := zoom < MAP_ZOOM
	for band in _bands.values():
		var at := _shifted(_walked(band, now))
		if not _in_view(at):
			continue
		var shift := at.x - _walked(band, now).x
		var look: Array = _looks[band["culture"]]
		if map_scale:
			draw_circle(at, 3.0 * grow, INK, true, -1.0, true)
			draw_circle(at, 2.2 * grow, look[0], true, -1.0, true)
			continue
		_draw_trail(band["trail"], shift, look[0])
		_draw_band(at, band["figures"], look, now, band["since"] < now - _step_seconds)
	if map_scale:
		return
	for town in _towns:
		var at := _shifted(town["at"])
		if _in_view(at):
			_draw_town(at, town, _looks[town["culture"]], now)


func _draw_trail(trail: PackedVector2Array, shift: float, dress: Color) -> void:
	var count := trail.size()
	for i in count:
		var fade := float(i + 1) / float(count + 1)
		var color := dress.darkened(0.3)
		color.a = 0.25 + 0.6 * fade
		draw_circle(trail[i] + Vector2(shift, 0.0), 1.5 * grow, color, true, -1.0, true)


# A band huddles on its tile, back figures first; walking, it bobs.
func _draw_band(at: Vector2, figures: int, look: Array, now: float, resting: bool) -> void:
	var spots: Array[Vector2] = []
	for i in figures:
		var t := sqrt((float(i) + 0.5) / float(figures))
		var angle := float(i) * 2.39996
		spots.append(Vector2(cos(angle), sin(angle) * TILT) * 0.3 * HexTerrain.RADIUS * t)
	spots.sort_custom(func(a, b): return a.y < b.y)
	for i in figures:
		var bob := 0.0 if resting else absf(sin(now * 9.0 + float(i) * 1.7)) * 0.7
		# Every other walker carries a staff.
		_draw_figure(at + spots[i] * grow - Vector2(0.0, bob * grow), look, i % 2)


# Townsfolk wander slowly along the front of their settlement, never behind
# its houses, since this layer draws above the buildings.
func _draw_town(at: Vector2, town: Dictionary, look: Array, now: float) -> void:
	var figures: int = town["figures"]
	var radius: float = town["spread"] + 0.2 * HexTerrain.RADIUS
	for i in figures:
		var slot := (float(i) + 0.5) / float(figures)
		var drift := sin(now * 0.35 + town["phase"] + float(i) * 2.1) * 0.08
		var angle := PI * (0.12 + 0.76 * clampf(slot + drift, 0.0, 1.0))
		_draw_figure(at + Vector2(cos(angle), sin(angle) * TILT) * radius * grow, look, 0)


# One person standing at `feet`, from the shared sprites: the body tinted by
# their dress, the head by their skin, then hair, outline and shadow.
func _draw_figure(feet: Vector2, look: Array, pose: int) -> void:
	var px: float = FIGURE_PX * grow * look[2]
	var rect := Rect2(
		feet - FigureSprites.FEET * px, Vector2(FigureSprites.W, FigureSprites.H) * px
	)
	var layers: Array = _sprites.poses[pose]
	draw_texture_rect(layers[0], rect, false, look[0])
	draw_texture_rect(layers[1], rect, false, look[1])
	draw_texture_rect(layers[2], rect, false)


func _walked(band: Dictionary, now: float) -> Vector2:
	var t := clampf((now - band["since"]) / _step_seconds, 0.0, 1.0)
	return band["from"].lerp(band["to"], t)


# The copy of `at` one world lap east or west that sits nearest x.
func _near(at: Vector2, x: float) -> Vector2:
	var lap := terrain.world_width()
	return Vector2(at.x + roundf((x - at.x) / lap) * lap, at.y)


func _shifted(at: Vector2) -> Vector2:
	return _near(at, center.x)


func _in_view(at: Vector2) -> bool:
	var margin := HexTerrain.RADIUS * 2.0
	return (
		absf(at.x - center.x) < half_view.x + margin
		and absf(at.y - center.y) < half_view.y + margin
	)


func _now() -> float:
	return float(Time.get_ticks_msec()) / 1000.0
