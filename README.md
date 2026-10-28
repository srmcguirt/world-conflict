# World Conflict

A god-game of a chosen people. Civilizations rise on a generated hex world;
you guide one people from Eden onward, and they write scripture about what
they think you meant.

Godot 4 shell over a deterministic Rust simulation (gdext). The design is the
spec: [GAME_DESIGN.md](GAME_DESIGN.md). Rules for coding agents:
[AGENTS.md](AGENTS.md).

## Run it

Needs Rust 1.98 and Godot 4.7.

```bash
cargo build -p sim-gd
godot --path godot
```

`scripts/release.sh` builds the signed macOS app into `exports/` (needs Godot's
4.7.2 export templates).

`scripts/check.sh` is the quality gate: fmt, clippy with the workspace lints,
the sim's tests, and gdlint/gdformat.

## License

Source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md).
You may read, run, modify and share it for noncommercial purposes. Commercial
use, including selling the game or anything built from it, is reserved to the
author. Outside contributions need a written agreement first, so the project
can still be relicensed.
