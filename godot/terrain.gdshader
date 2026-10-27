// Terrain beauty pass. Every number here comes from docs/art-bible.md.
// One draw call for the whole world: the mesh carries per-vertex COLOR (the
// sim's per-tile overlay color), NORMAL (analytic surface facing, so lights
// can land on the world) and CUSTOM0 (hash, flags, AO, rim), where flags is
// water=1 plus chosen=2.
shader_type canvas_item;

uniform float light_x = -0.62; // art bible: light from the upper left
uniform float light_y = -0.55;
uniform float ambient = 0.38; // art bible: the floor no surface goes below
uniform float diffuse = 0.62;
uniform float ripple_speed = 1.4;
uniform float night = 0.0; // 0 day, 1 night
uniform vec2 beacon_pos = vec2(0.0); // local mesh space
uniform float beacon_radius = 476.0; // 14 tiles * 34px
uniform vec3 beacon_color = vec3(0.851, 0.659, 0.271); // Chosen gold
uniform sampler2D color_tex; // per-tile overlay color, updated on revision

varying float v_water;
varying float v_chosen;
varying float v_ao;
varying float v_rim;
varying float v_hash;
varying vec2 v_pos;

void vertex() {
	float flags = COLOR.r; // water=1, chosen=2, water+chosen=3
	v_water = mod(flags, 2.0);
	v_chosen = step(2.0, flags);
	v_ao = COLOR.g; // height AO
	v_rim = COLOR.b; // territory rim
	v_hash = COLOR.a; // procedural hash
	v_pos = VERTEX.xy;
}

void fragment() {
	vec3 col = texture(color_tex, UV).rgb;

	// Light: one direction, everywhere (art bible). Flat tiles face the
	// camera, so they take a base term and the height gradient's slope
	// modulates it; wall faces shade by which way they look.
	vec2 n = normalize(NORMAL.xy + vec2(0.0001, 0.0001));
	float slope = dot(n, normalize(vec2(light_x, light_y)));
	float is_wall = step(3.5, UV.x);
	float lit = ambient + diffuse * mix(0.55 + 0.45 * slope, 0.25 + 0.75 * slope, is_wall);

	// Water breathes; land does not.
	if (v_water > 0.5) {
		lit += sin(TIME * ripple_speed + v_hash * 40.0) * 0.08;
		col *= 0.94; // water sits darker than land at the same value
	}

	// Height AO and the coast/territory rim.
	lit *= 1.0 - v_ao;
	col *= lit;
	if (v_rim > 0.01) {
		col = mix(col, beacon_color, v_rim * 0.55);
	}

	// Night: everything cools toward umber; the beacon is a real light.
	if (night > 0.01) {
		col = mix(col, vec3(0.165, 0.122, 0.090), 0.25 * night);
	}
	float beacon = pow(max(0.0, 1.0 - distance(v_pos, beacon_pos) / beacon_radius), 1.5);
	col += beacon_color * beacon * 0.85 * (0.85 + 0.15 * sin(TIME * 2.0)) * night;

	COLOR = vec4(col, 1.0);
}
