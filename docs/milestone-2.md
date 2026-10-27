# Milestone 2 — First Witness

Seed 1, year 300, a meteor on Eden. Two people saw it, and their people wrote
it down:

```
witness: Yohiav, of the Kouvriamo

In year 301, a stone fell at Votias and the ground remembered it.
Yohiav said the Watcher is wrathful, and the Kouvriamo do not go there.

In year 301, a stone fell at Vouchiak and the ground remembered it.
Wovrosos said the Watcher is wrathful, and the Kouvriamo do not go there.
```

That is the milestone gate: **you smite something and read what someone
believed you meant by it.**

## Reproduce

```bash
# The binding, end to end, without a window:
cd godot && godot --headless --script res://tools/smoke_cast.gd

# The belief map after the same strike:
cargo run --release -p sim-core --bin worldsim -- \
  --seed 1 --years 400 --cast 4 --after 12 --overlay belief --out /tmp/belief.png
```

In the game: `1`-`7` pick a power, left-click casts it on the tile under the
cursor, right-click or Esc cancels. `M` cycles the biome, culture and belief
overlays. The scripture column is the Chosen people's newest verses, first at
the top.

## The six powers

| Slot | Power | Footprint | Who watches | What it does |
|---|---|---|---|---|
| 1 | Raise | 3x3 | 2 tiles | Lifts land, reclassifies the biome |
| 2 | Lower | 3x3 | 2 tiles | Sinks land; below sea level it becomes ocean |
| 3 | Paint | 3x3 | 1 tile | Replaces the biome (grassland for now) |
| 4 | Bless | 3x3 | 1 tile | +25% carrying capacity for ten years |
| 5 | Meteor | 3x3 | 6 tiles | Kills 85% of the people, craters the land, -30% capacity |
| 6 | Tornado | 3x3 | 3 tiles | Kills 40%, leaves the land alone |
| 7 | Spawn | 3x3 | 2 tiles | Beasts hunt the local people for twenty years |

The design's six powers count Raise and Lower as one, so Spawn takes the
seventh slot.

## How the witness rule works

- A power use snapshots every populated tile in its footprint plus its
  visibility radius, before the effect lands.
- Five ticks later each window closes and scores the tile:
  `(actual Δpopulation% − expected Δpopulation% from its own 10-tick trend) +
  Δcarrying-capacity%`, clamped to [-1, 1]. `|score| < 0.05` is "no effect".
- A negative score raises `wrathful` and `capricious`, a positive one raises
  `benevolent` and `protective`, and silence lowers `absent`.
- Each power promotes one witness from the most populous affected tile,
  preferring the Chosen people (a meteor promotes two). A tile with no people
  can't supply one, and a power over open ocean writes nothing at all.
- A witness nobody elevates within twenty years is demoted to a legend record.

## Names

Every people draws its own consonant and vowel inventory from its founding
seed, so the Kouvriamo and their daughters sound like themselves. Names are a
pure function of `(name_seed, index)` — nothing to serialize, and the same seed
always produces the same people.
