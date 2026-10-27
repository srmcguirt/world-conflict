# Speed plan

How World Conflict keeps 100 sim-years per second and a smooth frame as the
world grows (design, Frame and tick scheduling; R14). Measured 2026-10-27 on an
Apple M5, seed 1, the Godot debug path (`target/debug/libsim_gd.dylib`).

## Where it stands

| What | Before | Now | How |
|---|---|---|---|
| 100 ticks at year 2000, in Godot | 5,247 ms | 733 ms | `sim-core` at opt-level 3 in the dev profile |
| One tick at year 2000, in Godot | 52 ms | ~3.4 ms | same, plus migration scanning hex rings instead of whole discs |
| One tick at year 2000, release | 4.48 ms | 2.14 ms | migration's ring scan (bit-identical results on four seeds) |
| Terrain rebuild after Raise or Meteor | 484 ms (whole map) | ~50 ms | full-width bands, packed arrays, only bands near `Sim.changed_tiles` |
| Terrain build at load | 484 ms in one frame | ~320 ms over 20 frames, worst 16 ms | one band per frame behind the loading bar |
| Settlement refresh per tick | every view freed and rebuilt | only changed sites redrawn | per-tile diff in `game.gd` |
| Frame spikes from ticking | unbounded | capped near 8 ms | the design's tick budget, backlog capped at one second |

**What that means for play:** a tick costs about 3.4 ms at year 2000 in
Godot, and the budget is 8 ms, so the 100× setting fits about two ticks per
frame, enough for 100 years per second at 60 fps. That is arithmetic from the
numbers above, not a measured in-game rate.

**Where the tick goes** (release, seed 1, years 2000–2500, per tick):
migration 1.73 ms, polities 0.23 ms, drift 0.12 ms, everything else under
0.02 ms each. M6a's systems were never the problem: before the fix, migration
was 4.07 of 4.48 ms, because each crowded tile built a full hex disc per ring
(out to radius 5) and then filtered it back down to the ring.

## Next, ranked by gain for the effort

1. **Done 2026-10-27: profile the tick.** Migration was 91% of it; scanning
   rings directly halved the tick with identical results.
2. **Make migration's full-world search cheaper.** It still costs 1.7 ms
   because a saturated world sends almost every crowded tile out to radius 5
   finding no room. Keeping a per-region count of tiles with room would let a
   search skip full regions, but the skip must not change which tile wins a
   tie. *Gain: maybe another 1 ms per tick. Effort: half a day plus a
   fingerprint check like the one used here.*
3. **Build terrain bands in Rust.** Port the terrace, corner, peak and column
   math from `godot/hex_terrain.gd` into `sim-gd`, returning packed arrays per
   band; GDScript keeps only picking and placement. *Gain: a power's rebuild
   from ~50 ms to a few ms, load from ~320 ms to ~20 ms; makes 320×160
   practical. Effort: about a day, with tests for the height rules.*
4. **Run the sim on its own thread, Mac only.** The design allows this if the
   100 years/s gate misses. `Sim` keeps owning the API but a `std::thread` owns
   `World`; commands go in over a channel and each tick publishes a snapshot
   (overlay bytes, tile codes, settlement sites, events). Determinism holds
   because the sim stays single-threaded and powers still apply at tick
   boundaries (D9). The web build keeps the in-thread path. *Gain: the tick
   leaves the frame entirely; the only route to 100 years/s on the 320×160
   target if a tick costs ~12 ms there. Effort: 2–3 days, needs a design
   sign-off.*
5. **gdext `safeguards-dev-balanced`.** Lighter runtime checks in dev builds
   (the log shows "safeguards strict"). *Gain: small, cheap. Effort: minutes,
   but confirm the feature exists in gdext 0.5.5 first.*
6. **Stop sending heights every repaint.** Already done for heights (checked
   only after a power); the overlay and code textures are ~100 KB per repaint
   at 160×80, which the research measured as trivial. No action.

## Not recommended now

- **Staggering systems** (each tile updated every k ticks): deterministic, but
  it changes the rules, so it is a design decision, not a speed tweak.
- **Rayon / data parallelism in the tick:** deterministic only with per-tile
  RNG streams and fixed-order reductions, which conflicts with the current
  per-system streams (Determinism) and complicates the web build.
- **MultiMesh or baked textures for buildings:** `_draw` output is cached and
  batched since Godot 4.4/4.6; with the diffed refresh, 450–2,000 settlement
  nodes are fine.

## Sources

The research pass behind items 2–5 cites the Godot docs on procedural
geometry, custom drawing and `ImageTexture.update`, the canvas culling code in
`renderer_canvas_cull.cpp`, the 4.4 / 4.6 batching notes, Cargo's profile
overrides, and the gdext docs on `experimental-threads`.
