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
| D10 | **Map-first UI, no permanent scripture rail.** The map is the game screen; one selection-driven panel floats on click (settlement card / polity panel); clicking nothing shows nothing | User direction 2026-10-27: "we want the game view as clear as possible for awareness" |
| D11 | **Scripture becomes an event log**: a collapsible bottom strip (key `L`) holding verses *and* kingdom events for the Chosen — tier-ups, protests, riots, government changes, finished buildings. Clicking an entry pans the camera to where it happened | User direction: the log answers "what is going through the minds of my chosen" in one feed |
| D12 | **Settlement glyphs are Rust pixels, zoom-aware**: full composite glyph (roof/wall by culture style seed, material by biome, towers by tier, faith mark by doctrine) at zoom; a tier-colored pixel at map scale | Keeps the design's one-Image map architecture; 449 settlements at year 2000 can't each hold a fixed glyph |
| D13 | **Overlay cycle**: `M` walks biome → culture → language → polity → belief | Fixes the M5 gap (language overlay shipped in Rust but was unreachable in-game) and slots polity next to culture |
| D14 | **Selection**: click with no power selected selects (settlement → card, polity land → panel); the selected glyph takes a gold ring; `Esc` clears; `Tab` cycles settlements | The map could only cast powers; nothing was inspectable |
| D15 | **Empty states are designed**: before the first settlement the log names the year and where the first town will raise; the polity overlay falls back to culture colors until a polity exists; the selection panel shows a warm empty state | An empty panel that says nothing reads as a broken game |

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

## UI design (plan-design-review, 2026-10-27)

Wireframes: `docs/designs/m6a-wireframes.html` (v2, map-first) and
`docs/designs/m6a-iso-prototype.html` — a working canvas prototype of the
isometric renderer (drag-pan, wheel-zoom, click-to-pick, cliff walls, the shader
terms, settlement glyphs with towers and faith marks). Superseded for the map by
the isometric direction; the panel and log designs still stand. Visual direction
stays Kinetic Sand Glyphs — umber/black base, antique gold for the Chosen, glyph
rings (project learning `world-conflict-visual-direction`).

**Screen model.** The map is the whole window. A 1-line HUD (year, speed,
overlay, peoples, power) sits on top; the event log is a collapsible bottom
strip; exactly one floating panel answers "what is this?" when something is
clicked. No permanent rails. Camera: drag to pan, wheel to zoom, arrows/WASD to
move — isomorphic with awareness, per the user's direction.

**What each surface shows.**

| Surface | Contents |
|---|---|
| Settlement card | tier, slot ring colored by line (damaged slots hatched — riot history stays visible), current build + progress, labor, the strata bar with inline legend |
| Polity panel | the 6 meters as bars with numbers, unrest thresholds printed under them ("protest at 0.5 · riot at 0.65"), government, capital, settlement count, legitimacy source named |
| Event log | one chronological feed: verses (the Chosen's inner life) + kingdom events; a gold pip on the collapsed strip when something new arrives; entries click to focus the camera |

**Design dimension ratings** (0–10, and what a 10 needs):

| Dimension | Now | A 10 needs |
|---|---|---|
| Information architecture | 7/10 | one selection panel, log collapsible — shipped in this plan |
| Empty states | 3/10 → 9 | D15's designed empties (log names where the first town raises; polity overlay falls back to culture colors) |
| Interaction states | 4/10 → 9 | D14 selection + ring + Esc + Tab; power casting unchanged |
| Visual craft | 5/10 → 8 | D12 zoom-aware glyphs; remaining gap is final pixel craft, settled during implementation with screenshots |
| Event feedback | 6/10 → 9 | protest lands as a verse (already), riot damage shows as a hatched slot, both appear in the log |
| Accessibility | 5/10 → 8 | polity tints carry a border difference, not color alone; meters print numbers; keyboard selection via Tab |
| AI-slop risk | 7/10 | a game HUD in the project's own palette, not a dashboard template |
| Zoom/responsive | 4/10 → 8 | D12's zoom-aware rendering; glyph legibility re-checked at 1x and 4x in the milestone screenshot |

**Accessibility floor (non-negotiable in M6a):** no information carried by color
alone (polity tints get a border treatment, meters print numbers), keyboard
selection, and the log readable at its collapsed one-line height.

## Isometric renderer (user direction 2026-10-27: "I wanted to go isometric")

The user directed an isometric game view. This amends `GAME_DESIGN.md`'s
architecture line ("the map is one Image") in its own commit; `sim-core`'s
contracts are untouched. `paint_overlay(mode) -> PackedByteArray` keeps
returning one RGBA pixel per tile — the CLI, the overlays and every existing
test keep working. What changes is who turns tiles into a picture.

> **Revised 2026-10-27 (second pass).** The terrain and settlement specs below
> drew square diamonds and flat glyphs, which the user rejected. `GAME_DESIGN.md`
> (Rendering) now specifies hex columns with camera-facing sides, terraced
> heights, a three-quarter camera with no rotation, and settlements as clusters
> of polygon houses. Where this section disagrees, the design wins; D12's "Rust
> pixel glyphs" is superseded the same way.

**Terrain: one mesh, one draw call.** Godot builds a single `ArrayMesh` laid out
in iso space: for each tile a top diamond at `elevation × HEIGHT_SCALE`, plus a
wall quad dropping to each higher neighbor's height wherever the ground falls
away (cliffs — this is what makes the world read as raised relief, the
design's own language). Vertex colors come straight from `paint_overlay`. One
new API, `Sim.tile_heights() -> PackedFloat32Array`, feeds the geometry;
`revision()` gates the rebuild, which is valid because powers apply only at
tick boundaries (D9) and every tick bumps it.

**The beauty pass is one shader, not textures.** A single `ShaderMaterial`
over the terrain mesh computes, from per-vertex attributes only:

| Term | Effect |
|---|---|
| fixed light | top faces lit, cliff faces shaded by which way they look |
| height AO | tiles sitting below their surroundings darken slightly |
| water ripple | ocean and beach tiles breathe on a slow sine (per-vertex water flag from biome) |
| wet rim | coastline tiles darken at the water edge |
| chosen rim | the Chosen's tiles take an antique-gold edge — the design's gold-for-Chosen, painted on the world itself |

No texture sampling, no PBR, one draw call for the whole map. At the design's
320×160 gate map that is 51.2k quads in a single call with a color-attribute
update only on tick.

**Entities: cached procedural glyphs in a `YSort`.** Settlements are real iso
objects, not tile tints (user direction): a procedural `Image` texture per
`(culture style seed, material biome, tier, doctrine)` combination — dozens of
unique textures, cached — drawn as a `Sprite2D` anchored to the settlement's
tile at its elevation height, with towers stacked by tier and the faith mark
floating above the roof. Notables are small figures in their dress colors,
camera-culled. The M8 beacon and gold-edged wonders join the same node system.
`YSort` gives iso depth ordering for free.

**Camera and picking.** `Camera2D` over the iso-laid-out mesh: the mesh carries
the projection, so the camera is ordinary 2D. Fixed 2:1 angle (D18), drag to
pan, wheel to zoom (0.5×–4×), WASD/arrows, smoothed. Screen→tile is a six-line
inverse 2:1 projection with no rotation term.

**What this costs.** The M5-era `TextureRect` map in `main.gd` is replaced by
the mesh; the overlay cycle, the HUD and the event log are unchanged. The
design's "no TileMapLayer" clause survives — this is not a TileMap.

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

---

## GSTACK DESIGN REVIEW REPORT

```
STATUS: DONE_WITH_CONCERNS
REASON: The M6a plan had no UI design at all; it now does (D10-D15), calibrated
  to the Kinetic Sand Glyphs direction and the user's map-first direction.
  Mockups: HTML wireframes v2 at docs/designs/m6a-wireframes.html (the gstack
  designer binary was unavailable - no OPENAI_API_KEY - so the documented HTML
  wireframe fallback was used).
ATTEMPTED: pre-review audit (git log, AGENTS.md, GAME_DESIGN.md UI contracts,
  main.gd surface), UI scope detection, wireframe generation, 8 dimension
  ratings, decisions D1-D5 (D1 and D5 redirected the UI model), plan update.
CONCERNS:
  1. The M5 language overlay was unreachable in-game (main.gd cycles 4 of
     Rust's 5 modes). Fixed in the plan by D13, but it should be verified in
     the M6a implementation - the M overlay index mapping must include both
     language and polity.
  2. The scripture panel's current verses[0] freshness check assumes a static
     verse list. As an event log it needs a revision/append model; the exact
     Godot-side data flow (Sim.events() drain vs scripture_text polling) is an
     implementation decision to make when the log is built.
  3. Settlement glyph pixel craft (roof/wall silhouettes, faith marks) is
     specified but not drawn. It needs screenshot-driven iteration during
     implementation, the way M4's portraits did.
RECOMMENDATION: implement M6a sim-first (per the eng report's TDD order), then
  the UI in this order: overlay cycle + selection (D13/D14), settlement glyphs
  (D12), the two panels (D10), the event log (D11). Screenshot each stage.
```
