class_name FigureSprites
extends RefCounted

# The figure sprites for the people on the map (D29; art bible, People). Each
# pose is drawn once at startup from signed distances, so every edge carries
# exact coverage and the sprite stays smooth at any zoom through its mipmaps.
# A pose is three layers: the body in grey, tinted by the people's dress; the
# head in grey, tinted by their skin; and a finished overlay of hair, outline,
# staff and ground shadow that is drawn untinted on top.

const W := 24
const H := 64
const FEET := Vector2(12.0, 59.0)  # the point that stands on the ground
const LIGHT_SIDE := 0.33  # how much darker the right of the body is than its left
const OUTLINE := 0.9  # outline width in sprite pixels
const INK := Color("#121110")
const HAIR := Color("#2A1E16")
const WOOD := Color("#5A4128")

# One pose per entry: [body, head, overlay] textures.
var poses: Array[Array] = []


func _init() -> void:
	poses.append(_pose(false))
	poses.append(_pose(true))


func _pose(staff: bool) -> Array:
	var body := Image.create(W, H, false, Image.FORMAT_RGBA8)
	var head := Image.create(W, H, false, Image.FORMAT_RGBA8)
	var over := Image.create(W, H, false, Image.FORMAT_RGBA8)
	for y in H:
		for x in W:
			var p := Vector2(x + 0.5, y + 0.5)
			var parts := _body(p)
			var body_cover := _cover(parts.x)
			var head_d := p.distance_to(Vector2(12.0, 17.5)) - 4.0
			var head_cover := _cover(head_d)
			var lit := 1.05 - LIGHT_SIDE * clampf((p.x - 6.0) / 12.0, 0.0, 1.0)
			body.set_pixel(x, y, Color(lit * parts.y, lit * parts.y, lit * parts.y, body_cover))
			head.set_pixel(x, y, Color(lit, lit, lit, head_cover))
			var solid := maxf(body_cover, head_cover)
			# Hair caps the back of the head; the outline rings everything.
			var hair := _cover(maxf(head_d + 0.3, p.y - 15.0)) * head_cover
			var ring := _cover(minf(parts.x, head_d) - OUTLINE) * (1.0 - solid)
			var color := Color(0, 0, 0, 0)
			var shadow := Vector2((p.x - FEET.x) / 7.5, (p.y - FEET.y - 0.5) / 2.2)
			color = _over(color, INK, 0.3 * _cover((shadow.length() - 1.0) * 2.2) * (1.0 - solid))
			if staff:
				var pole := _segment(p, Vector2(19.5, 10.0), Vector2(18.5, 59.0)) - 0.75
				color = _over(color, WOOD, _cover(pole))
			color = _over(color, INK, 0.75 * ring)
			color = _over(color, HAIR, hair)
			over.set_pixel(x, y, color)
	return [_texture(body), _texture(head), _texture(over)]


# The body's signed distance and its grey: tunic, sleeves, belt and legs.
func _body(p: Vector2) -> Vector2:
	var t := clampf((p.y - 24.0) / 24.0, 0.0, 1.0)
	var tunic := maxf(absf(p.x - 12.0) - lerpf(4.2, 5.8, t), maxf(23.5 - p.y, p.y - 47.0))
	var shoulders := _segment(p, Vector2(8.5, 25.0), Vector2(15.5, 25.0)) - 2.6
	var sleeves := minf(
		_segment(p, Vector2(7.2, 26.0), Vector2(6.4, 39.0)) - 1.35,
		_segment(p, Vector2(16.8, 26.0), Vector2(17.6, 39.0)) - 1.35
	)
	var legs := minf(
		_segment(p, Vector2(10.3, 45.0), Vector2(10.0, 58.5)) - 1.5,
		_segment(p, Vector2(13.7, 45.0), Vector2(14.0, 58.5)) - 1.5
	)
	var cloth := minf(tunic, shoulders)
	var d := minf(minf(cloth, sleeves), legs)
	var grey := 1.0
	if legs < cloth and legs < sleeves:
		grey = 0.62
	elif sleeves < cloth:
		grey = 0.86
	elif absf(p.y - 36.0) < 0.9:
		grey = 0.55  # the belt
	return Vector2(d, grey * lerpf(1.0, 0.88, t))


func _segment(p: Vector2, a: Vector2, b: Vector2) -> float:
	var ab := b - a
	var h := clampf((p - a).dot(ab) / ab.length_squared(), 0.0, 1.0)
	return p.distance_to(a + ab * h)


# A signed distance in pixels to the share of the pixel the shape covers.
func _cover(d: float) -> float:
	return clampf(0.5 - d, 0.0, 1.0)


func _over(under: Color, top: Color, alpha: float) -> Color:
	var a := alpha + under.a * (1.0 - alpha)
	if a <= 0.0:
		return under
	var rgb := (
		(
			Vector3(top.r, top.g, top.b) * alpha
			+ Vector3(under.r, under.g, under.b) * under.a * (1.0 - alpha)
		)
		/ a
	)
	return Color(rgb.x, rgb.y, rgb.z, a)


func _texture(image: Image) -> ImageTexture:
	image.generate_mipmaps()
	return ImageTexture.create_from_image(image)
