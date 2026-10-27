# Milestone 3 — Belief

Seed 1, year 300, six meteors on the largest settlement, then forty years
between each: a daughter people comes out of it believing in a wrathful god,
and founds its own faith.

```
cultures: 4
  #0 parent None Chosen tiles  470 people 3717 faith 0 None          piety 0.00 belief [0.34, 0.30, 0.32, 0.30, 0.20, 0.33]
  #1 parent Some(0) Other tiles 142 people 1325 faith 1 Some(Wrathful) piety 0.13 belief [0.91, 0.30, 0.35, 0.30, 0.20, 0.28]
  #2 parent Some(0) Other tiles 178 people 1173 faith 0 None          piety 0.00 belief [0.33, 0.30, 0.31, 0.30, 0.20, 0.33]
  #3 parent Some(0) Other tiles  62 people  194 faith 0 None          piety 0.00 belief [0.33, 0.30, 0.31, 0.30, 0.20, 0.33]
```

Two peoples who lived through the same world now have different religions about
the same god. The map is `docs/milestone-3-belief.png`.

## Reproduce

```bash
cargo run --release -p sim-core --bin worldsim -- \
  --seed 1 --years 300 --cast 4 --casts 6 --after 40 \
  --overlay belief --out /tmp/belief.png
```

`--cast 4` is a meteor (the toolbar index), `--casts 6` repeats it at the most
populous tile, and `--after 40` gives the people forty years to make sense of
it. `--overlay belief` paints each tile by what its people believe.

## How belief becomes doctrine

- **Belief lives per tile**, six axes (wrathful, benevolent, capricious,
  protective, jealous, absent), all starting at 0.3 except jealous 0.2 and
  absent 0.35.
- **Seeing counts.** Every populated tile in the footprint plus its visibility
  radius takes 0.15 of the act's intent immediately — a meteor reads as wrath
  before anyone measures what it did.
- **The window sharpens it.** Five ticks later each witness scores
  `(Δpopulation% − expected) + Δcapacity%`; a strong score moves another 0.18
  on the matching axes, and a dead zone of 0.05 is "no effect".
- **Testimony spreads.** Every decade a tile drifts 26% toward its people's
  mean, and where a people already lean one way (`conviction`: top axis ≥0.35
  and ahead by 0.05) that belief grows 0.04 a decade. Doubt never grows: nobody
  preaches that the god is absent.
- **Doctrine crystallizes** at 0.5 on the dominant axis, after holding for two
  consecutive 25-tick checks.
- **A prophet rises** when a doctrine crystallizes: the people's most salient
  witness is elevated (or a new person is raised), and their verse becomes the
  doctrine's founding line. Prophets keep their office; witnesses are demoted
  after twenty years.
- **Piety** is the mean of the five conviction axes minus `absent`.

## Schism

Checked every 25 ticks. A people at least 0.4 (Euclidean, six axes) from its
faith's founding belief for two consecutive checks gets a new `faith_id`.
Peoples who diverge in the same check and are within 0.2 of each other share
one new faith — they saw the same god.

## Re-anointing

`world.queue_anoint(culture_id)` moves favor at the next tick: the old people
become `Other`, their `jealous` rises 0.15, and both scriptures gain a verse.
The ceremonial confirm (D11: a one-second hold and a card naming who is
displaced) waits on the inspector from Milestone 6; the mechanic and its
determinism are covered by tests.

## Doctrine effects today

- **Wrathful** takes 2% of the people every decade (sacrifice).
- Piety is already exposed to the HUD and the belief overlay.
- Benevolent, protective, capricious and absent effects belong to buildings,
  research and war (Milestones 6-9); the doctrine is recorded now so those
  systems read it rather than inventing it.

## Tests

Eleven contracts in `sim-core/tests/faith.rs`: culture belief is the
population-weighted mean, doctrine needs two checks, a new doctrine elevates
exactly one prophet who is named in scripture and not demoted, a schism founds
a faith, peoples who diverge together share it, re-anointing moves favor and
angers the old people, piety falls with doubt, wrathful doctrine costs lives,
and the same seed and actions produce the same faiths.
