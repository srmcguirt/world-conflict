#!/usr/bin/env bash
# The quality gate. The pre-commit hook and CI both run this, so "green" means
# the same thing on a laptop and on GitHub. See AGENTS.md.
#
#   scripts/check.sh          full gate
#   scripts/check.sh --fast   skip the release-mode test run (hook default)
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

step() { printf '\n== %s\n' "$1"; }

step "rustfmt"
cargo fmt --all --check

step "clippy (workspace lints, -D warnings)"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "tests (sim-core, headless)"
if [[ "${1:-}" == "--fast" ]]; then
  cargo test -p sim-core --locked --quiet
else
  cargo test -p sim-core --locked --release --quiet
fi

step "tick-length guard (D24: 1 tick = 1 sim-year)"
if grep -rniE "ticks_per_year|TICKS_PER_YEAR|tick_length|YEARS_PER_TICK" sim-core/src sim-gd/src; then
  echo "tick-length constants are forbidden (D24)" >&2
  exit 1
fi

if command -v gdlint >/dev/null 2>&1; then
  step "gdlint / gdformat"
  gdlint godot/
  gdformat --check godot/
fi

printf '\nquality gate: green\n'
