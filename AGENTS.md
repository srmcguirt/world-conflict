# World Conflict: rules for coding agents

These rules apply to every agent working in this repo, whatever editor or model
runs it. The gate enforces most of them mechanically. The rest are for you to keep.

## The design is the spec

- `GAME_DESIGN.md` is the specification. Before writing code, find the section it
  implements and cite it in the doc comment, the way existing code does:
  `(design, Witness rule)`, `(D24)`, `(R9)`.
- If the design is silent or ambiguous, stop and ask. Do not invent a rule and
  ship it as if the design said so.
- If the code and the design disagree, say so in your report. Do not quietly
  pick one.

## Done means the gate is green

```bash
scripts/check.sh        # fmt, clippy with the workspace lints, tests, D24 guard
```

- Run it before saying a task is done, and paste the last line.
- The pre-commit hook runs `scripts/slop-guard.sh` and `scripts/check.sh --fast`.
  Install it once per clone with `git config core.hooksPath .githooks`.
- Never commit with `--no-verify`. If the gate is wrong, fix the gate in its own
  commit and say why.

## Tests

- Write the test first, from the design's numbers, then make it pass.
- A test must be able to fail. Assert the behavior, not that a constant equals
  itself. `cargo mutants -p sim-core --file <file>` shows whether your tests
  catch a broken implementation; a "missed" mutant is a test that isn't checking
  anything.
- Never delete or weaken an assertion to get green, and never add `#[ignore]`.
  The slop guard blocks both; the overrides (`SLOP_ALLOW_*`) are for a human to
  set, and the commit message has to say why.
- Every test you write must use every value it computes. An unused `let` in a
  test means the check you meant to write is missing.

## Lints

- The workspace lints in `Cargo.toml` are `clippy::pedantic` plus a set of
  restriction lints. CI denies every warning.
- `#[allow]` is denied. If a lint is truly wrong for one item, use
  `#[expect(lint, reason = "...")]` on that item only. The reason says why
  this code is right, not "to silence clippy". Fixing the code is almost
  always better.
- Do not change `Cargo.toml` lint levels, `clippy.toml`, `deny.toml`,
  `scripts/` or `.github/` in the same change as code.

## Determinism (sim-core)

- No `HashMap` or `HashSet`. Use `BTreeMap`, `BTreeSet` or `Vec` (R9).
- Transcendental float math goes through `libm` (`libm::sinf`, not `f32::sin`).
  `clippy.toml` bans the std versions.
- No wall clock, no thread RNG. Every random draw comes from the system's seeded
  stream, derived from the world seed.
- One tick is one sim-year (D24). No tick-length constants.
- No `.unwrap()` in library code. Return `Option` or `Result`, or use `expect`
  with a message that states the invariant.

## Shape of the code

- Match the code around you: its naming, comment density and idioms.
- Prefer an array indexed by a constant (`[f32; TRAITS]`) over four parallel
  fields that copy one another.
- Don't add speculative abstraction, unused parameters or "for later" code.
  Later milestones add what they need.
- Keep functions under 80 lines (`too_many_lines`). Split by meaning, not just
  by length.
- Comments explain why, or point to the design. Don't narrate what the next
  line does.

## Commits

- One milestone task per commit, with a message in the existing style
  (`M4: adaptation — trait drift, perks, portraits, dress tints`).
- Commit only files you changed for that task. Check `git status` first: another
  agent may be working in the same checkout.
