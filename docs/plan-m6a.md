# Milestone 6a plan — Kingdoms

Target of this review: the Milestone 6 plan in `GAME_DESIGN.md` (Deep systems §1
Buildings and settlement advancement, §2 Social balance, the Milestone mapping
line "Milestone 6, Kingdoms", and the M3-law block under D18).

Status: reviewed, decisions D2–D9 recorded, ready to implement TDD.

## Decisions taken in review

| # | Decision | Why |
|---|---|---|
| D2 | Rescale + catchment, combination with experiments | The design's tier numbers are unreachable in this world (evidence below) |
| D3 | **Claimed catchments + percentile tiers.** One settlement per claimed region (greedy, radius 2, non-overlapping). Tier = percentile of the claimed-catchment distribution, recomputed each 25-tick check: City top 1%, Town top 5%, Village top 25%, Camp the rest. The design's 80%-for-2-checks demotion rule applies on top | Measured distribution has only a 2.5× spread median→max, so no absolute ladder works at any world scale; percentiles self-stabilize as M7–M10 grow the world |
| D4 | **Split M6a/M6b.** M6a = settlements, 5 building lines, polities, meters, 4 governments, protest/riot. M6b = the covenant machinery (episodes, hold, judgment), the land law, and the two stranger laws | Covenant machinery is entirely unbuilt (P1 finding); mixing it with Kingdoms makes both harder to test |
| D5 | **A polity is a territory.** Founding: a tier-1 settlement with no polity within reach founds one. Joining: later settlements within reach join it. Territory: settled tiles belong to the polity of the nearest settlement within its radius. Cohesion = spread of per-tile `culture_vec` and `belief` across the polity's tiles, plus a penalty per extra culture or faith. Federation waits for M8 | The design never says how a polity is founded or what land it holds; cohesion-as-culture-count stays 1.0 until M8 and the rebellion rule never fires |
| D6 | **serde + ron** for `sim-core/data/buildings.ron`, hashed into `SIM_BUILD` | Design mandates data files; a const table breaks "content tuning never needs a code change" |
| D7 | **Extend `Role`** (Witness, Prophet → + Ruler, Priest, Merchant); ambition drives coups/heresies/secession | Uses the existing budgets and salience eviction; no parallel notable system |
| D8 | **Defer the family-line panel** to M6b/M7; the sim-side tracking ships in M6a | The design's own mapping omits it from M6 |
| D9 | **Two new modules**: `settlements.rs` (tiers, claiming, slots, labor, construction) and `society.rs` (polities, strata, meters, governments, protest/riot) | Auto-decided (question unanswered); smaller diff, matches module granularity |

## Evidence

All numbers below are measured, seed 1, release build.

```
world at year 2000:   30,938 people / 33,335 capacity (93%)
                     4,046 settled tiles, densest tile = 12.0 people
design tier table:    Camp 50+ | Village 200+ | Town 2,000+ | City 10,000+
                     footprints 1 / 1 / 1 / 3 tiles
```

**The tier table as written never fires.** A City's whole 3-tile footprint holds
~36 people against a 10,000 threshold; even ×10 world capacity (probe) yields 0
cities while flooding 4,046 camps. The distribution shape, not the scale, is the
problem: catchments are uniformly saturated (top 274 vs median ~160).

Claimed catchments fix the count through scarcity (one settlement per region),
and percentiles fix the ladder:

```
radius 2: 449 settlements | max 274 | p1 255 | p5 227 | p10 198 | p25 157 | p50 109
radius 3: 295 settlements | max 470 | p1 439 | p5 368 | p10 336 | p25 261 | p50 185
```

At year 2000 that is ~45 settlements per culture and ~4–5 cities world-wide,
which is the shape the design's tier ladder was reaching for.

## REALITY rows

- `REALITY: premise-table pass` — every factual claim above carries a file:line
  or a measured number; unverified=0.
- `REALITY: already-done pass` — `git log --since=2026-10-20` shows M1–M5 only;
  no settlements/polity/meter code exists to reuse or collide with.
- `REALITY: surface-check pass` — `worldsim --overlay biome|culture|traits|belief|language`
  verified live; `--overlay polity` is the new surface M6a adds.
- `REALITY: binding-decisions pass` — D2–D9 above are this review's ledger; no
  reintroduced overruled default.
- `REALITY: numbers pass` — all thresholds tagged `[measured: seed 1, year 2000,
  release]`; the design's own tier numbers are quoted as `[design, unreachable]`.
- `REALITY: deferred-asks pass` — no linked issue; deferrals are design-scoped
  (rebellion/revolution/exodus → M8, Knowledge+Craft lines → M7/M8, family-line
  panel → M6b/M7, federation+conquest → M8).
- `REALITY: pr-count pass` — one PR per wave: M6a lands as one milestone commit
  series on main, M6b after it.

## Findings

```
[P0] (confidence: 9/10) GAME_DESIGN.md:295-303 — tier thresholds unreachable:
     densest tile holds 12 people against Camp's 50+; no settlement ever forms.
     Resolved by D3.
[P1] (confidence: 9/10) sim-core/src/faith.rs:31 is the only "covenant" hit in
     the tree — the M3 land law, D19 counting, D20 hold and D17/D21 colors are
     specced with tests but have no code. Resolved by D4 (M6b).
[P1] (confidence: 8/10) GAME_DESIGN.md:258 — "polities form by conquest or
     federation" with no founding rule and no definition of federation; the
     design never says what land a polity holds. Resolved by D5.
[P2] (confidence: 7/10) GAME_DESIGN.md:68 vs :121 — the tile model carries
     `polity_id` while the text says "polities group cultures"; cohesion read as
     culture-count stays 1.0 until M8, so the 0.75 rebellion rule never fires
     in M6. Resolved by D5's cohesion-as-spread.
[P2] (confidence: 7/10) GAME_DESIGN.md:855-866 vs :817-823 — the Milestones
     list names the family-line UI in M6, the mapping omits it. Resolved by D8.
[P2] (confidence: 7/10) GAME_DESIGN.md:404 — Republic needs "knows Philosophy",
     an M7 invention; M6 governments stop at Band/Chiefdom/Monarchy/Theocracy,
     matching the mapping. No action.
[P3] (confidence: 6/10) GAME_DESIGN.md:289 — SAVE_VERSION bump on schema change
     has no save system to bump yet; SIM_BUILD lands now, SAVE_VERSION is a
     forward note. No action in M6a.
```

## Architecture

```
per tick (existing 10-tick block unchanged)
per 25-tick check, after language drift:

  settlements::tick_settlements
    ├── claim catchments (greedy, radius 2, one per region)
    ├── tier ladder from claimed percentiles (+ 80%/2-check demotion)
    ├── labor: pop × (1 − subsistence) → fills one build slot
    └── construction: score = need × culture weight × doctrine modifier
                        tie → lowest catalog index

  society::tick_society
    ├── polity founding/joining by settlement reach
    ├── territory: settled tile → nearest settlement's polity
    ├── strata shares → roll up per polity
    ├── 6 meters with hysteresis
    ├── government thresholds (2 checks): theocracy, monarchy, chiefdom
    └── unrest: protest (verse) at 0.5+, riot (−2% pop, one building) at 0.65+

data flow:  tiles ──claim──▶ settlements ──reach──▶ polities ──meters──▶ events
            belief/trait tiles ──spread──▶ cohesion ──▶ unrest ──▶ protest verse
```

New state: `settlements: Vec<Settlement>`, `polities: Vec<Polity>`,
`tile_polity: Vec<u16>` (the design's per-tile `polity_id` slot),
`settlement_of: Vec<u32>`. No HashMap; all Vec-indexed (R9).

## Test review — coverage

New code paths and their contracts (each is a test written first):

- claiming: non-overlapping, one settlement per region, greedy order is
  deterministic (same seed → same claims)
- tier ladder: percentiles of the claimed distribution; City = top 1% at 449
  settlements ⇒ ~4 cities `[measured]`
- demotion: below 80% of the tier's percentile value for 2 checks drops one tier
- granary absorbs one famine and refills over 10 years
- temple slows verse drift 30% per level (compare drift counters with/without)
- construction: score = need × culture weight × doctrine modifier; a tie breaks
  to the lowest catalog index
- upkeep: an unpaid settlement decays one building level per 50 sim-years
- polity founding by a tier-1 settlement; later settlements within reach join;
  territory follows the nearest settlement
- cohesion falls as belief spreads across a polity's tiles
- theocracy at piety > 0.7 with priesthood wealth ≥ 30%, held 2 checks
- protest at unrest ≥ 0.5 writes a verse; riot at ≥ 0.65 removes 2% of the
  largest settlement's people and damages one building
- `SIM_BUILD` hash is stable across runs and changes when the catalog changes
- determinism: same seed + same action log → same settlements, tiers,
  governments (R10)
- performance: tick with ~449 settlements stays inside the frame budget
  (current tick 3.06 ms `[measured: M5 bench]`)

Suppressed (confidence < 7): none.

## Performance

The design's 4,000-settlement × 20-slot estimate describes an M9-scale world.
At M6 the measured world yields 449 claimed settlements `[measured]`, so slot
updates are ~1.3k per check against a 3.06 ms tick — the language check already
costs more. The frame gate (R14) is measured again at M6 close, as the design
requires, on the 320×160 map.

## Not in scope (M6a)

Covenant machinery and the land/stranger laws (M6b), Knowledge and Craft
building lines (M7/M8), rebellion/revolution/exodus outcomes (M8), conquest and
federation (M8), the family-line UI panel (M6b/M7), SAVE_VERSION (no save
system yet).

## GSTACK REVIEW REPORT

```
STATUS: DONE_WITH_CONCERNS
REASON: M6 plan reviewed; the design's tier table was unreachable as written
  (P0) and the covenant machinery it depends on is unbuilt (P1). Both are
  resolved by the D2-D9 decisions: claimed catchments + percentile tiers, and
  the M6a/M6b split. Two concerns remain open for implementation.
ATTEMPTED: scope gate, Step 0 (code + measurement probes for tier thresholds,
  claimed catchments, covenant/polity gaps), complexity gate (D9), decisions
  D2-D9, reality rows, findings with confidence scores, test coverage map.
CONCERNS:
  1. Percentile tiers demote a settlement when a rival grows. The design's
     80%-for-2-checks hysteresis softens it, but "your town shrank because a
     neighbor's grew" may still read as unfair. First playtest should watch it.
  2. Cohesion-as-spread has no design numbers for the penalty weights. M6a
     ships tuning values documented like M3-M5's, and the 0.75 rebellion
     threshold is M8's to fire anyway.
RECOMMENDATION: implement M6a TDD in this order: (1) claimed catchments + tier
  ladder, (2) buildings catalog + labor + construction, (3) polities +
  territory, (4) strata + meters + governments, (5) protest/riot. Then M6b.
```
