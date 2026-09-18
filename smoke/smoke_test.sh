#!/usr/bin/env bash
# smoke/smoke_test.sh — quick sanity check that every CLI subcommand starts
# and produces the expected artifacts.  Run from the workspace root:
#
#   bash smoke/smoke_test.sh
#
# Exit code is nonzero on the first failure.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SMOKE_DIR="$ROOT/smoke"
OUTPUT_DIR="$SMOKE_DIR/output"

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

pass() { printf "  ✓ %s\n" "$1"; }
fail() { printf "  ✗ %s\n" "$1" >&2; exit 1; }
run_matchlab() { cargo run --manifest-path "$ROOT/Cargo.toml" -- "$@"; }

json_field() {
  python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
for k in sys.argv[2].split('.'):
    d = d[k]
print(d)
" "$1" "$2"
}

echo "=== smoke: --help ==="
output=$(run_matchlab --help 2>&1)
echo "$output" | grep -q "matchlab" || fail "help output missing 'matchlab'"
pass "prints usage"

echo "=== smoke: --version ==="
output=$(run_matchlab --version 2>&1)
echo "$output" | grep -q "matchlab [0-9]" || fail "version output malformed: $output"
pass "prints version"

echo "=== smoke: run ==="
run_matchlab run "$SMOKE_DIR/experiment.yaml"
run_json="$OUTPUT_DIR/run/smoke.json"
[ -f "$run_json" ] || fail "missing run result"
[ "$(json_field "$run_json" name)" = "smoke" ] || fail "wrong experiment name"
[ "$(json_field "$run_json" config_hash)" = "c0fad22fef1ab636" ] || fail "config hash mismatch"
[ "$(json_field "$run_json" matches_completed)" = "0" ] || fail "matches_completed != 0"
pass "produces smoke.json (fields ok)"

echo "=== smoke: compare ==="
run_matchlab compare "$run_json"
pass "compare exits cleanly"

echo "=== smoke: package ==="
cd "$ROOT"
run_matchlab package "$SMOKE_DIR/experiment.yaml"
pkg=$(ls smoke_*.json 2>/dev/null | head -1)
[ -n "$pkg" ] || fail "missing package file"
pkg_hash=$(json_field "$pkg" metadata.config_hash 2>/dev/null || json_field "$pkg" config_hash)
[ "$pkg_hash" = "c0fad22fef1ab636" ] || fail "package config hash mismatch"
rm -f "$pkg"
pass "produces reproduction package (hash ok)"

echo "=== smoke: power ==="
output=$(run_matchlab power --sd 10 --effect 5 2>&1)
echo "$output" | grep -q "Required replications" || fail "power output missing expected text"
pass "prints power analysis"

echo "=== smoke: study ==="
run_matchlab study "$SMOKE_DIR/study.yaml" --replicates 1
study_json=$(find "$OUTPUT_DIR/study" -name 'smoke_study-*.json' ! -name 'study_stats.json' -print -quit 2>/dev/null)
[ -n "$study_json" ] || fail "missing study result"
[ "$(json_field "$study_json" name)" = "smoke_study" ] || fail "wrong study name"
stats_json="$OUTPUT_DIR/study/study_stats.json"
[ -f "$stats_json" ] || fail "missing study_stats.json"
pass "produces study result (fields ok)"

echo "=== smoke: analyze ==="
run_matchlab analyze "$study_json"
pass "analyze exits cleanly"

echo "=== smoke: compare-stats ==="
run_matchlab compare-stats "$study_json" "$study_json"
pass "compare-stats exits cleanly"

echo "=== smoke: optimize ==="
cd "$SMOKE_DIR"
run_matchlab optimize optimize.yaml
opt_json="output/optimize/smoke_optimize_optimization.json"
[ -f "$opt_json" ] || fail "missing optimization result"
[ "$(json_field "$opt_json" name)" = "smoke_optimize" ] || fail "wrong optimize name"
[ "$(json_field "$opt_json" budget)" = "2" ] || fail "budget != 2"
opt_trials=$(json_field "$opt_json" "trials | length" 2>/dev/null || python3 -c "import json; print(len(json.load(open('$opt_json'))['trials']))")
[ "$opt_trials" = "2" ] || fail "expected 2 trials, got $opt_trials"
pass "produces optimization result (fields ok)"

echo ""
echo "All smoke tests passed."
