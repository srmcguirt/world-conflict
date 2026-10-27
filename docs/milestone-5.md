# Milestone 5 — Tongues

Seed 1, year 2000: ten peoples, ten tongues, one family tree. Every split
spawns a dialect that descends from her mother's; the map is
`docs/milestone-5-tongues.png`, painted by tongue rather than by people, so
the eldest branches read at a glance.

```
cultures: 10
  #0 parent None   tongue Khehuetia   (the first people's language)
  #1 parent Some(0) tongue Pegaerei   #5 parent Some(0) tongue Khoraulae
  #2 parent Some(0) tongue Chaepuepue #6 parent Some(5) tongue Pimukue
  #3 parent Some(0) tongue Khianuru   #7 parent Some(3) tongue Yadefau
  #4 parent Some(0) tongue Nototi     #8 parent Some(0) tongue Huloube
                                      #9 parent Some(3) tongue Lopyalu
```

## Reproduce

```bash
cargo run --release -p sim-core --bin worldsim -- \
  --seed 1 --years 2000 --overlay language --out /tmp/language.png
```

`M` in the game cycles biome → culture → traits → belief → **language**
(slot 5, already wired).

## How tongues work

Every language is a phoneme inventory plus a 100-word core vocabulary
(`sim-core/src/language.rs`). Words are generated, never stored as strings:
`word(key)` syllabifies from the tongue's phonemes with a sound-change counter
per word. Sixteen lexeme keys carry what scripture needs today (year, sky,
storm, green, spawn, land, gift, wrath, and the six doctrines, people, plus
spares for later systems).

- **Drift**: once every 25 years each tongue shifts words, at a rate scaled by
  isolation — the far side of an ocean moves faster than the trade road. A
  shift is one bump of a word's change counter, and the counter rewrites the
  word's sounds.
- **Contact**: peoples whose homes sit within 14 tiles borrow one word per
  check from their nearest different-tongue neighbor, and a borrowed word is
  fixed (it stops drifting there).
- **Scripture reads both ways**: a verse is fact plus slots, so the game
  renders every verse as its own current tongue's line above the fixed
  English translation. The same event reads differently as the tongue moves;
  what happened, stays.

A people's home (`Culture.place`) is the mean of their tiles' coordinates,
refreshed on the aggregate cadence; it drives both isolation and contact.

## The unreliable copy

At a split the daughter inherits her mother's scripture — then re-spells it:
her own people-name, her own place-name, her own spelling of every witness.
The verse keeps its year and its kind; only the words change hands. Later
generations read a history that has quietly drifted toward the reader, and a
schism's two books can disagree about names while agreeing about facts.

Witness re-spelling uses the witness id (`person_name(culture, id)` is a pure
function of the name seed), and verse variants (translation line pick) hash
the witness name — so a re-spelled copy may pick a different variant of the
same verse. Accepted: the narrator is unreliable by design.

## Eden build gate (itch.io)

The design (Milestones) makes M5 the first released build. Build commands for
the universal macOS zip (the itch.io upload itself is manual):

```bash
cargo build --release -p sim-gd          # aarch64 layer
# x86_64 layer (once): rustup target add x86_64-apple-darwin
# NOTE: if Homebrew's cargo shadows rustup's in PATH, the cross-target std
# is invisible and the build fails with "can't find crate for `core`".
# Use rustup's cargo explicitly:
PATH="$HOME/.cargo/bin:$PATH" cargo build --release -p sim-gd --target x86_64-apple-darwin
mkdir -p target/universal
lipo -create -output target/universal/libsim_gd.dylib \
  target/release/libsim_gd.dylib \
  target/x86_64-apple-darwin/release/libsim_gd.dylib
# Verified on 2026-10-27: the fat dylib loads in Godot 4.7.2 headless and
# smoke_cast.gd runs. Export the Godot project as a macOS .app with
# target/universal/libsim_gd.dylib in place of the gdextension's dylib,
# ad-hoc sign (codesign --force --deep -s -), zip, and butler push to itch.io.
```

Included in the build: the map, seven powers, witnesses and scripture in each
people's own words, drifting tongues and their family tree, adaptation.

## Oss notes

- The design gave no numbers; these are tuning choices: 100 core words,
  16 live lexeme keys, drift 1 + isolation/60 words per check
  (a lone tongue on the map: about 4 words a generation from the 60-tile
  midpoint), contact radius 14 tiles, one borrowed word per contact check.
- Language overlay reuses the culture hue palette shifted +32 steps, so a
  people and their tongue are recognizably related colors without colliding.
- The language check joins the 25-tick door (splits → aggregates →
  languages → doctrines → schisms), and tongue names differ per split because
  they draw on the daughter's own name seed.

## Tests

Nine contracts in `sim-core/tests/language.rs`: a split spawns a descending
tongue, isolated tongues drift faster, neighbors borrow words, scripture
reads as native line above translation, a drifting tongue reads its scripture
differently while the translation stays, a daughter's copy carries her own
name, the family tree traces to the first tongue, the same seed writes the
same tongues, and the language overlay paints every tile. Plus unit tests for
word stability under reads, drift changing a word, and a borrowed word
holding.
