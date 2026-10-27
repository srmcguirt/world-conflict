# Milestone 4 — Adaptation

Seed 1, year 2000: ten peoples, and they no longer look the same. Mean skin
runs from 0.53 to 0.91, tracking where each people's home sits between the
poles and the equator. The map is `docs/milestone-4-traits.png`: dark warm
bands along the equator, pale slates toward the poles and the highlands.

```
cultures: 10
  #0 Chosen skin 0.70     #5  skin 0.84
  #1 Other   skin 0.91    #6  skin 0.91
  #2 Other   skin 0.82    #7  skin 0.75
  #3 Other   skin 0.82    #8  skin 0.53
  #4 Other   skin 0.77    #9  skin 0.59
```

## Reproduce

```bash
cargo run --release -p sim-core --bin worldsim -- \
  --seed 1 --years 2000 --overlay traits --out /tmp/traits.png
```

`M` in the game cycles biome → culture → **traits** → belief.

## How bodies adapt

Every tile holds four traits in [0, 1] — skin, stature, cold tolerance, lung
capacity — starting at 0.5.

| Trait | driven by | perk |
|---|---|---|
| skin | UV (latitude) | none — looks only |
| stature | temperature | none — looks only |
| cold | temperature | growth +10% at full adaptation below temp 0.35 |
| altitude | elevation | growth +10% at full adaptation above elev 0.62 |

Each decade a tile drifts 5% toward the environment's optimum for each trait,
with a little hash noise so lineages differ under the same sun. Settlers carry
their traits with them, the way they carry their language and their god. The
design rule stands: cosmetic differences and small regional perks, never a
ranking — nothing any people has is better in the abstract, only better
suited to where they live, and skin and stature change nothing at all.

## Portraits

Every witness and prophet is a person now: `promote_witness` samples their
skin and build from the tile that raised them. The sim draws a 12×16 RGBA
pixel sprite per person (skin tone face, build-wide shoulders, hair rolled by
id, a cloak in their people's dress color), and Godot scales it ×3 with
nearest filtering. The dress color is the culture's banner warmed by the
people's mean skin, so two peoples at the same latitude still dress apart.

The newest witness's portrait sits above the scripture column, next to the
verses they spoke.

## Oss notes

- The design gave no numbers; these are tuning choices, documented above.
- The traits overlay mixes a cooling slate into the skin ramp by altitude so
  high cold country reads differently from pale lowlands.
- `docs/milestone-3.md`'s wrathful sacrifice now has a rule above it, added
  with the player on 2026-10-27: **doctrine proposes, covenants bind** —
  practices are the people's own reading of the god's conduct, and the
  Milestone 8 covenant system will bound what they may enact.

## Tests

Eight contracts in `sim-core/tests/adaptation.rs`: the sun writes skin across
peoples, highlanders grow lungs, adaptation takes generations, a cold-adapted
body lets the land carry more people (same seed, one tile apart), looks cost
nothing (skin and stature change no population anywhere), a witness takes the
traits of their tile, portraits are stable per person, and the traits overlay
paints skin. Plus unit tests for the optima table, the perk gates and the skin
ramp.
