#!/usr/bin/env bash
# Diff-level checks that lints can't see: they look at what a change removes,
# not just at what the code looks like afterward. Runs on the staged diff in
# the pre-commit hook, or on `BASE...HEAD` when given a base (CI).
#
# Override one check for one commit with its variable, and say why in the
# commit message:
#   SLOP_ALLOW_FEWER_ASSERTS=1   a refactor really did merge assertions
#   SLOP_ALLOW_IGNORE=1          a test is parked with a tracked reason
#   SLOP_ALLOW_GATE_CHANGE=1     a reviewed change to the gate itself
#   SLOP_ALLOW_EXPECT=1          a reviewed batch of #[expect] suppressions
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

if [[ $# -gt 0 ]]; then
  diff() { git diff --unified=0 "$1...HEAD" -- "${@:2}"; }
  base="$1"
else
  diff() { git diff --cached --unified=0 -- "${@:2}"; }
  base=""
fi

fail=0
rust=('*.rs')

# 1. Tests may grow or change, never quietly shrink. An agent that can't make
#    a test pass deletes the assertion; this catches it.
asserts='^\s*(debug_)?assert(_eq|_ne)?!'
removed=$(diff "$base" "${rust[@]}" | grep -E '^-' | grep -v '^---' | sed 's/^-//' | grep -cE "$asserts" || true)
added=$(diff "$base" "${rust[@]}" | grep -E '^\+' | grep -v '^+++' | sed 's/^+//' | grep -cE "$asserts" || true)
if (( removed > added )) && [[ "${SLOP_ALLOW_FEWER_ASSERTS:-}" != 1 ]]; then
  echo "slop-guard: this change removes $removed assertions and adds $added." >&2
  echo "  Fix the code, not the test. If assertions really were merged, set SLOP_ALLOW_FEWER_ASSERTS=1." >&2
  fail=1
fi

# 2. No parking failing tests.
if diff "$base" "${rust[@]}" | grep -E '^\+' | grep -qE '#\[ignore'; then
  if [[ "${SLOP_ALLOW_IGNORE:-}" != 1 ]]; then
    echo "slop-guard: adds #[ignore] to a test. Make it pass, or set SLOP_ALLOW_IGNORE=1 with a reason." >&2
    fail=1
  fi
fi

# 3. No loosening the gate itself in the same change as code.
gate_files=(Cargo.toml clippy.toml deny.toml rustfmt.toml scripts/check.sh scripts/slop-guard.sh .githooks/pre-commit .github/workflows/ci.yml)
code_changed=$(diff "$base" 'sim-core/src/*' 'sim-gd/src/*' 'sim-core/tests/*' | grep -c '^+++' || true)
if (( code_changed > 0 )); then
  if [[ "${SLOP_ALLOW_GATE_CHANGE:-}" != 1 ]] &&
    diff "$base" "${gate_files[@]}" | grep -E '^\+' | grep -v '^+++' | grep -qE '= *"allow"|level *= *"allow"|-A |--cap-lints|SLOP_ALLOW'; then
    echo "slop-guard: this change relaxes a lint or gate alongside code changes." >&2
    echo "  Loosen the gate in its own commit, reviewed on its own." >&2
    fail=1
  fi
fi

# 4. Suppressions must say why (the lint enforces reason=; this keeps them rare).
new_expects=$(diff "$base" "${rust[@]}" | grep -E '^\+' | grep -cE '#!?\[expect\(' || true)
if (( new_expects > 3 )) && [[ "${SLOP_ALLOW_EXPECT:-}" != 1 ]]; then
  echo "slop-guard: adds $new_expects #[expect] suppressions. Fix the code instead of silencing the lint." >&2
  fail=1
fi

exit "$fail"
