# Art bible — page 1 (the world)

Every number the terrain renderer reads lives here. When a screenshot looks
wrong, the fix is a number on this page, not a patch in the shader. (Layer 1 of
the graphics chain, `docs/graphics.md`.)

## Palette families

Six biomes, each a ramp with shadow / mid / light values. The mid is the sim's
existing `biome_color`; shadow and light are derived by the grading rule below,
so the ramps stay in one family. Hex for authoring, sRGB 0-255 for the shader.

| Biome | shadow | mid (sim) | light |
|---|---|---|---|
| Ocean | `#14304E` | `#1A3E6E` | `#2C5484` |
| Beach | `#A8915C` | `#D6C594` | `#E8DBB4` |
| Grassland | `#5E6E40` | `#7C9454` | `#9CB06C` |
| Forest | `#2E4E32` | `#3A663C` | `#4E7C4E` |
| Desert | `#9A7C48` | `#CEAE6E` | `#E2C48E` |
| Mountain | `#6A675F` | `#8A8682` | `#A8A49C` |
| Ice | `#B4C2CE` | `#E8F0F6` | `#F8FBFD` |

Accents, fixed by the project's visual direction (learning
`world-conflict-visual-direction`, 2026-10-27):

| Role | Color | Where |
|---|---|---|
| Chosen gold | `#D9A845` | Chosen territory rim, wonders, the beacon |
| Rival ochre | `#B9853F` | Rival territories, secondary structure |
| Ink | `#121110` | Deepest shadow, UI base |
| Umber | `#2A1F17` | UI panels, haze |
| Text | `#EDE6DA` | UI type |
| Muted | `#A89A86` | UI secondary type |

## Light

- **Direction:** from the upper left, 35° above the horizon. In 2D screen
  space the light vector is `(-0.62, -0.55)` normalized (x right, y down), so
  cliffs facing upper-left are lit and cliffs facing lower-right are in shadow.
- **Ambient:** 0.38 — the floor no surface goes below.
- **Diffuse:** 0.62 on top faces; wall faces take 0.55 × their facing term so
  cliffs read as volume without going muddy.
- **Height AO:** a tile sitting `d` below the mean of its four neighbors
  darkens by `min(0.25, d × 0.35)`.

## Water

- Ripple: `sin(time × 1.4 + tile_hash) × 0.08` on lightness. Slow, never
  flashing.
- Wet rim: land tiles adjacent to water darken 12% at the shared edge.

## Grading

One pass over everything: lift shadows toward umber (never pure black), roll
highlights toward gold, cap saturation at 0.85. The palette *is* the grade —
screenshots should histogram into these families, not into noise.

## Fog and haze

- Atmospheric perspective: desaturate 18% and cool toward umber over the outer
  40% of the view radius.
- Depth haze on the map's far edge only; the play area stays crisp.

## Readability rules (the gates)

1. At 0.5× zoom: Chosen border, town vs camp, water vs land each read in under
   a second.
2. At 4× zoom: material variation per tile, no flat diamonds.
3. One light direction in every screenshot.
4. 60fps at 4× on the 320×160 map with ~500 entities (R14, measured at M6).
5. Same seed → identical render (R10).

## Night pass

The beacon is a real light: a warm gold point light at the beacon tile, radius
~14 tiles, falloff to zero, pulsing slowly. Settlements take a faint hearth
glow proportional to tier. Everything else cools 25% toward umber. Night is a
mode, not a separate art style.
