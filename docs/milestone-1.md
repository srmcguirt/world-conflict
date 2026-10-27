# Milestone 1 — Living Map

Seed 1, year 2000: 4,046 settled tiles, 30,938 people, 8 peoples.

![Biome map](milestone-1-biome.png)
![Culture map](milestone-1-culture.png)

The culture map is the Milestone 1 "done when" check in a picture: three or
more visibly distinct peoples, separated by terrain. Unsettled land is a
desaturated biome shade, so only places people actually live are lit.

## Reproduce

```bash
cargo run --release -p sim-core --bin worldsim -- \
  --seed 1 --years 2000 --overlay culture --out /tmp/culture.png
```

`--trace 250` prints settled tiles, people, largest people and culture count
as the run goes:

```
  year  settled     people   largest  cultures
   500      778       5799       614     2
  1000     3150      24507      2390     3
  1500     3949      30259      1879     7
  2000     4046      30938      1879     8
```

## What the systems do

- **Eden** plants one people on the best settleable tile the seed offers.
- **Population** grows logistically toward the tile's latitude-corrected
  carrying capacity and never overshoots it.
- **Migration** spills a crowded tile's excess one ring out, and up to five
  tiles when the near frontier is full. That wave is what lets a people cross
  an empty desert or mountain belt, land beyond it, and end up as a separate
  component.
- **Snowballing settlements**: a migration never depopulates its source and
  always lands a founding band of one person, so a new tile carries its
  culture from birth instead of growing back an orphan.
- **Drift** is isolation-scaled mutation plus cross-culture blending. The
  vector is a set of neutral cultural markers, so divergence comes from
  history and distance rather than a biome attractor; physical adaptation is
  Milestone 4's trait system.
- **Founder effect**: every new settlement is a mutated copy of its source, so
  serial settlement drifts geographically the way real populations diverged.
- **Splits** (every 25 years): a component of at least 30 tiles holding at
  least 5% of a people's population and sitting at least 0.25 from the parent
  body's mean splits off after two consecutive checks.

## Gates

| Gate | Target | Measured |
|---|---|---|
| Tick length | 1 year | 1.73 ms/year (~578 years/s) |
| Speed ceiling | 100 years/s | 578 years/s at 160x80 |
| Peoples at year 2000 | 3 or more | 8 |

`cargo bench -p sim-core --bench tick` reproduces the tick number on the
machine; CI compiles the bench on every push.
