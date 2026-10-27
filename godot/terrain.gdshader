// Terrain beauty pass (design, Rendering). Every number comes from
// docs/art-bible.md. hex_terrain.gd bakes light into COLOR.r and writes the
// face kind to COLOR.g (0 land top, 0.25 water top, 0.5 side) and the
// fragment's offset inside its hex to COLOR.ba. Per-tile data comes from two
// textures sampled at the tile's texel: the overlay color, and the tile code
// (culture id + 1 in RG, ocean in B), which is compared across each edge to
// draw borders, the Chosen's gold edge and the wet coastline.
shader_type canvas_item;

uniform sampler2D color_tex : filter_nearest;
uniform sampler2D code_tex : filter_nearest;
uniform vec2 grid = vec2(160.0, 80.0);
uniform float chosen_code = 0.0; // chosen culture id + 1; 0 = none
uniform float ripple_speed = 1.4;
uniform float night = 0.0; // 0 day, 1 night
uniform vec2 beacon_pos = vec2(0.0);
uniform float beacon_radius = 388.0;
uniform vec3 gold = vec3(0.851, 0.659, 0.271); // Chosen gold #D9A845
uniform vec3 ink = vec3(0.071, 0.067, 0.063); // #121110
uniform vec3 umber = vec3(0.165, 0.122, 0.090); // #2A1F17

varying float v_light;
varying float v_kind;
varying vec2 v_local;
varying vec2 v_pos;

void vertex() {
	v_light = COLOR.r;
	v_kind = COLOR.g;
	v_local = COLOR.ba * 2.0 - 1.0;
	v_pos = VERTEX;
}

vec4 code_at(ivec2 tile) {
	tile.x = (tile.x + int(grid.x)) % int(grid.x);
	tile.y = clamp(tile.y, 0, int(grid.y) - 1);
	return texture(code_tex, (vec2(tile) + 0.5) / grid);
}

float culture_of(vec4 code) {
	return floor(code.r * 255.0 + 0.5) + floor(code.g * 255.0 + 0.5) * 256.0;
}

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

void fragment() {
	ivec2 tile = ivec2(floor(UV * grid));
	vec3 col = texture(color_tex, UV).rgb;
	bool top = v_kind < 0.4;
	bool water = v_kind > 0.1 && v_kind < 0.4;
	float light = v_light;

	if (top) {
		// Which edge this fragment faces, and how close it is (1 at the edge).
		// Pointy-top sides from the east, clockwise: E, SE, SW, W, NW, NE.
		float angle = atan(v_local.y, v_local.x);
		int side = int(mod(round(angle / 1.0471976), 6.0));
		float facing = float(side) * 1.0471976;
		float edge = dot(v_local, vec2(cos(facing), sin(facing))) / 0.8660254;

		int east = tile.y % 2;
		ivec2 offsets[6] = ivec2[6](
			ivec2(1, 0), ivec2(east, 1), ivec2(east - 1, 1),
			ivec2(-1, 0), ivec2(east - 1, -1), ivec2(east, -1));
		vec4 here = code_at(tile);
		vec4 there = code_at(tile + offsets[side]);
		float mine = culture_of(here);
		float theirs = culture_of(there);

		// Every hex reads as a hex: a faint seam along its edge.
		light *= 1.0 - 0.07 * smoothstep(0.9, 0.99, edge);

		// Material grain so a tile is never one flat color up close.
		light *= 0.96 + 0.08 * hash(vec2(tile) + floor(v_local * 3.0) * 0.37);

		if (water) {
			light += sin(TIME * ripple_speed + hash(vec2(tile)) * 40.0) * 0.05;
			// Shallow water near the shore runs lighter.
			if (there.b < 0.5) {
				col = mix(col, col * 1.35, smoothstep(0.55, 1.0, edge) * 0.6);
			}
		} else if (there.b > 0.5) {
			// Wet rim: land darkens where it meets the sea.
			col *= 1.0 - 0.12 * smoothstep(0.7, 1.0, edge);
		}

		// Borders are lines, not just tints (accessibility floor): a dark seam
		// between peoples, and gold on the Chosen's side of their frontier.
		if (mine != theirs && !water && there.b < 0.5) {
			float seam = smoothstep(0.86, 0.97, edge);
			if (chosen_code > 0.5 && mine == chosen_code) {
				col = mix(col, gold, smoothstep(0.7, 0.97, edge) * 0.85);
			} else if (mine > 0.5 && theirs > 0.5) {
				col = mix(col, ink, seam * 0.55);
			}
		}
	}

	col *= light;
	// Grade: shadows lift toward umber, never black (art bible).
	col = mix(umber, col, smoothstep(0.0, 0.55, light) * 0.25 + 0.75);

	if (night > 0.01) {
		col = mix(col, umber, 0.25 * night);
	}
	float beacon = pow(max(0.0, 1.0 - distance(v_pos, beacon_pos) / beacon_radius), 1.5);
	col += gold * beacon * 0.85 * (0.85 + 0.15 * sin(TIME * 2.0)) * night;

	COLOR = vec4(col, 1.0);
}
