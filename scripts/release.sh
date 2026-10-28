#!/usr/bin/env bash
# Builds the Eden release for itch.io (design, Distribution Plan; D3: Apple
# Silicon only): the release sim library, copied into the Godot project so the
# export bundles it, then an ad-hoc signed macOS .app zipped in exports/.
# Then checks what it built: the signature, the library inside the app, and a
# smoke run of the exported app that loads the sim and renders the map.
#
# Needs Godot's 4.7.2 export templates installed (Editor → Manage Export
# Templates, or the .tpz from the Godot release page).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

app_zip="exports/World Conflict.zip"
check="exports/check"

echo "== release library"
cargo build --release -p sim-gd --locked
# The headless editor that runs the export loads the debug library.
cargo build -p sim-gd --locked
mkdir -p godot/bin exports
cp target/release/libsim_gd.dylib godot/bin/

echo "== export (macOS, ad-hoc signed; the sim library is arm64, D3)"
godot --headless --path godot --import >/dev/null 2>&1 || true
godot --headless --path godot --export-release "macOS" "../$app_zip"

echo "== verify"
rm -rf "$check"
mkdir -p "$check"
ditto -x -k "$app_zip" "$check"
app="$check/World Conflict.app"
codesign --verify --deep --strict "$app"
lib=$(find "$app" -name libsim_gd.dylib | head -1)
if [[ -z "$lib" ]]; then
  echo "release: the simulation library is missing from the app" >&2
  exit 1
fi
echo "sim library: $(lipo -archs "$lib")"

# The shot tour loads seed 1, builds the terrain and renders before quitting:
# if the bundled library didn't load, no shot is ever taken.
shots=$("$app/Contents/MacOS/World Conflict" -- --shots --years 50 2>&1 | grep -c '^\[shot\]' || true)
if [[ "$shots" -lt 1 ]]; then
  echo "release: the exported app did not reach the map" >&2
  exit 1
fi
echo "smoke run: reached the map ($shots shots)"
echo "release: $app_zip"
