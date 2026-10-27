# Speed plan

How World Conflict keeps 100 sim-years per second and a smooth frame as the
world grows (design, Frame and tick scheduling; R14). Measured 2026-10-27 on an
Apple M5, seed 1, the Godot debug path (`target/debug/libsim_gd.dylib`).

## Where it stands

| What | Before | Now | How |
|---|---|---|---|
| 100 ticks at year 2000, in Godot | 5,247 ms | 733 ms | `sim-core` at opt-level 3 in the dev profile |
| One tick at year 2000, in Godot | 52 ms | ~7 ms | same |
| Terrain rebuild after Raise or Meteor | 484 ms (whole map) | ~50 ms | full-width bands, packed arrays, only bands near `Sim.changed_tiles` |
| Terrain build at load | 484 ms in one frame | ~320 ms over 20 frames, worst 16 ms | one band per frame behind the loading bar |
| Settlement refresh per tick | every view freed and rebuilt | only changed sites redrawn | per-tile diff in `game.gd` |
| Frame spikes from ticking | unbounded | capped near 8 ms | the design's tick budget, backlog capped at one second |

**What that means for play:** a tick costs about 7 ms at year 2000, and the
budget is 8 ms, so the 100× setting runs roughly one tick per frame, about 60
years per second at 60 fps. That is arithmetic from the numbers above, not a
measured in-game rate. The M5 release bench measured 3.06 ms per tick before
M6a added settlements, buildings and polities, so M6a roughly doubled the tick.

## Next, ranked by gain for the effort

1. **Profile the M6a tick.** 7 ms against 3 ms before M6a points at the
   25-tick settlement, polity and meter checks. Add per-system benches to
   `sim-core/benches/tick.rs` and fix the hot one. *Gain: back toward 100
   years/s at 160×80. Effort: half a day.*
2. **Build terrain bands in Rust.** Port the terrace, corner, peak and column
   math from `godot/hex_terrain.gd` into `sim-gd`, returning packed arrays per
   band; GDScript keeps only picking and placement. *Gain: a power's rebuild
   from ~50 ms to a few ms, load from ~320 ms to ~20 ms; makes 320×160
   practical. Effort: about a day, with tests for the height rules.*
3. **Run the sim on its own thread, Mac only.** The design allows this if the
   100 years/s gate misses. `Sim` keeps owning the API but a `std::thread` owns
   `World`; commands go in over a channel and each tick publishes a snapshot
   (overlay bytes, tile codes, settlement sites, events). Determinism holds
   because the sim stays single-threaded and powers still apply at tick
   boundaries (D9). The web build keeps the in-thread path. *Gain: the tick
   leaves the frame entirely; the only route to 100 years/s on the 320×160
   target if a tick costs ~12 ms there. Effort: 2–3 days, needs a design
   sign-off.*
4. **gdext `safeguards-dev-balanced`.** Lighter runtime checks in dev builds
   (the log shows "safeguards strict"). *Gain: small, cheap. Effort: minutes,
   but confirm the feature exists in gdext 0.5.5 first.*
5. **Stop sending heights every repaint.** Already done for heights (checked
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
