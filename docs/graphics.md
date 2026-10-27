# Graphics chain — how World Conflict gets its look

Researched 2026-10-27. This document is the production plan for the game's
visuals: what bar we hold ourselves to, what prior art we imitate, what assets
we actually acquire, and the four layers everything is built from.

## The bar

Not Diablo IV. That is a 3D PBR isometric game with a 300-person art team; the
honest bar for a 2D isometric god game is **Against the Storm / Manor Lords art
direction with AAA-grade coherence and polish**. Art direction beats polygon
count — every reference below wins on decisions, not budget.

What we steal from AAA specifically:

| Technique | How it translates here |
|---|---|
| One consistent light direction | The terrain shader computes analytic normals from the elevation attribute; light comes from one fixed direction, cliff faces shade by facing |
| Atmospheric perspective | Desaturate and cool with distance from the camera focal point; the map's edges fall into umber haze |
| Rim light on subjects | The Chosen's tiles take an antique-gold edge; wonders are gold-edged per the design |
| Cinematic color grading | One grading pass at the end: lifted umber shadows, gold highlights, restrained saturation — the Kinetic Sand Glyphs palette *is* the grade |
| Normal-mapped light response | Analytic normals let real 2D lights light the world: the beacon burns by night, a meteor glows on impact, a blessed settlement warms |

That last row is the big one: because our terrain is procedural, we can compute
normals from height instead of authoring normal maps, and Godot's 2D light
system then works on the whole world for free.

## Prior art teardown

| Reference | Steal this | Our twist |
|---|---|---|
| **Against the Storm** | Value structure that stays readable at 32px; dark-fantasy palette families per biome; atmosphere (rain, fog) as a mood layer | Our biomes are 6 and generated; the palette ramps live in the shader as the bible's numbers |
| **Manor Lords** | Organic settlement forms — Streusiedlung (scattered), Haufendorf (clustered), Rundling (circular) are real historical village shapes; earth-tone material discipline; buildings that look lived-in | Settlement claiming picks the catchment; the glyph generator composes in the culture's own style seed, so forms vary per people, not per map |
| **Cult of the Lamb** | Silhouette-first design; exactly one accent color per subject | Our accent language is already fixed: antique gold = the Chosen, ochre = rivals |
| **RimWorld / Dwarf Fortress** | Information density that survives a glance; the overlay system as first-class UI | Our six overlays (biome/culture/traits/belief/language/polity) are the same idea, painted by the sim |
| **Dorfromantik** | Procedural placement that still composes — small rules (water adjacency, clustering, spacing) that make generated worlds look designed | Claimed catchments already enforce spacing; composition rules (settlements near water, on high ground) go in the claiming score |
| **Old World / CK3 maps** | Painted-relief map legibility; borders that read at a glance | Polity borders are the same problem; the gold edge + hatched patterns solve it without relying on color alone |

## Asset inventory — what we actually acquire

Honest answer: almost nothing, and that is the design. Our world is infinite
and generated; a hand-painted tileset cannot cover it. What is worth having:

| Asset | Source | Use |
|---|---|---|
| Kenney isometric packs (CC0) | kenney.nl / itch.io | Prototyping, UI placeholder glyphs, style reference for the generator's output bar |
| Screaming Brain iso packs (CC0) | itch.io | Same — their wall/floor/object/town sets are the closest free prior art to our look |
| game-icons.net (CC-BY) | game-icons.net | UI icon grid for the toolbar and panel motifs |
| One font (OFL) | Google Fonts | The UI's single typeface; pick a face with good small-size rendering for the log |
| Aseprite (~$20, optional) | aseprite.org | Pixel touch-ups on generated sprites when the generator's output needs a hand-placed highlight |

What we do **not** acquire: terrain tilesets, building sprite sheets, character
art. Those are generated (below). Buying them would fight the procedural world.

## The four layers

```
┌─ Layer 1: the art bible ─────────── one page, every number lives here
│    palette ramps per biome · value structure · light direction ·
│    silhouette rules · readability rules · the grading pass
│
├─ Layer 2: terrain ──────────────── one mesh, one shader, zero textures
│    per-vertex colors from paint_overlay · analytic normals from elevation
│    · light + AO + water ripple + wet rim + Chosen rim · 2D lights land on it
│
├─ Layer 3: entities ─────────────── procedural pixel sprites, cached by key
│    parts library (wall/roof/material × biome, towers × tier, faith marks ×
│    doctrine) · composed from the culture's style seed · M4's portrait
│    pipeline is the proof this works · notables use the same path
│
└─ Layer 4: UI ───────────────────── one Godot theme resource + one font
     panels · glyph-ring motifs · the event log · meters · toolbar
```

**Layer 1 is the product.** Every visual decision downstream reads its numbers
from the bible. When a screenshot looks wrong, the fix is a bible number, not a
one-off patch — that is what keeps a generated world coherent at AAA polish.

**Layer 3's parts library is the real art work.** A settlement glyph is
composed, not drawn: wall style × roof style × material (by biome) × tower
count (by tier) × faith mark (by doctrine), all seeded by the culture's style
seed. We author maybe 6 walls, 6 roofs, 4 materials, 6 faith marks as small
pixel pieces, and the generator composes hundreds of distinct settlements from
them. Manor Lords proves the forms; Cult of the Lamb proves the silhouettes.

## Tools

- **Godot** — shader iteration (the terrain look) and, per Kenney's own
  tutorial, an iso sprite renderer if we ever want 3D-modeled building pieces
  rendered to sprite sheets.
- **Aseprite** — pixel touch-ups and the parts library authoring.
- **Material Maker** (free, OSS) — only if we later want texture-level detail;
  the shader's procedural detail may make it unnecessary.
- **AI image generation** — concept art and palette exploration only. Final
  tiles must come from the generator: AI output cannot hold consistency across
  an infinite procedural world, and the determinism rule (R9/R10) forbids a
  non-reproducible asset source anyway.

## Iterating higher than the prior art

Prior art ships a fixed tileset; we ship a *generator with an art direction*.
That gives us four things they cannot have:

1. **No two kingdoms look alike.** Every culture's architecture composes from
   its own style seed. A rival's towns are recognizable as *theirs* across the
   map — the design's per-culture palette index made visible in 3D form.
2. **The world carries its history.** Riot damage persists as a scarred glyph.
   A temple's faith mark follows the doctrine. The Chosen's border glows gold.
   Prior art's tiles look the same on day 1 and year 500.
3. **The map is lit by the game's events.** The beacon is a real light source
   at night; a meteor lights the impact; a blessed settlement warms. The god's
   actions are visible as light, not just as overlays.
4. **Infinite scale with one art direction.** 320×160 at M6's gate, larger
   later, same pipeline, same polish — because nothing is hand-placed.

## Quality gates (each is checkable in a screenshot)

1. **Readability at map scale (0.5×):** the Chosen's border, a town vs a camp,
   and water vs land are each distinguishable in under a second.
2. **Detail at 4× zoom:** tiles carry material variation; no flat diamonds.
3. **Palette discipline:** every visible color traces to a bible ramp. A
   screenshot's histogram should show the bible's families, not noise.
4. **60fps** at 4× zoom on the 320×160 map with ~500 entities (the design's
   R14 frame gate, measured at M6).
5. **Determinism:** same seed → byte-identical world render (R10).
6. **One light direction, everywhere.** Any screenshot where light appears to
   come from two directions is a bug.

## Look-dev spike (the first step, in Godot)

Before M6a's UI work, in the real medium:

1. Build the iso terrain mesh + shader + camera against the real worldgen.
2. Author the bible's first page: 6 biome ramps, the light direction, the AO
   and grading numbers.
3. Screenshot at 0.5× / 1× / 4× on seed 1 year 2000, plus a night pass with
   the beacon lit.
4. Iterate against the six gates above.

That is where the look gets decided. Everything after it is composition.
