# 012 — Final verification & acceptance

## Summary

End-to-end verification of the Lua-native refactor: full CI-equivalent checks,
reproduction of the v0.1 acceptance numbers through the all-Lua pipeline, and a
determinism gate. This is the last ticket; nothing ships before it passes.

## Context

Each ticket kept the tree green, but only this ticket runs the full suite
against the finished state and checks that the refactor did not regress the
project's own acceptance results (the strongest form of dogfooding). It also
validates the ported golden tests (Glicko-2, etc.) one final time.

## Scope

**In:**
- Full build/test/check/lint/fmt pass across the workspace.
- Acceptance run reproducing v0.1 numbers with an all-Lua pipeline.
- Determinism gate (two identical runs byte-identical).
- Ported-golden test audit.

**Out:**
- No new feature work; only fixes discovered by verification (with their own
  tests).

## Design

### Verification matrix

| Check | Command / detail |
|-------|------------------|
| Build | `cargo build --workspace` |
| Tests | `cargo test --workspace` |
| Check | `cargo check --workspace` |
| Lint | `cargo clippy --workspace -- -D warnings` |
| Format | `cargo fmt --check` |
| CI parity | `.github/workflows/ci.yml` steps run locally |

### Acceptance run (v0.1)

`cargo run -- run experiments/v0_1_basic.yaml` must reproduce the accepted v0.1
results through the all-Lua pipeline (rating elo, game logistic, matchmaking
batch, metrics from scripts):

| Metric | Expected (v0.1 acceptance) | Tolerance |
|--------|----------------------------|-----------|
| `rating_accuracy` mean start → end | Elo MAE decreases, ~197.5 → ~159.5 trend | trend down; within ~10% of recorded means |
| `rating_accuracy_by_time` | mean decreases over buckets | monotone-ish decrease |
| `match_quality` mean | ≈ 0.98 | ±0.02 |
| `queue_time` mean | ≈ 5.02 s | ±0.5 s |
| matches completed | capped by `max_time` as before | same count as prior run |

Record the new numbers in the ticket notes / AGENTS.md if they shift within
tolerance.

### Determinism gate

- `cargo run -- run experiments/v0_1_basic.yaml` twice into two output dirs;
  diff the JSON files. They must be byte-identical except the `timestamp`
  field (which legitimately differs).
- Same-seed runner unit test (exists) must still pass.
- Determinism through the Lua RNG path: covered by the same-seed test +
  `matchlab-lua` rng tests.

### Ported-golden audit

Confirm each of these still passes in `cargo test --workspace`:

- Glicko-2 worked example (r′=1464.06, RD′=151.52, σ′=0.05999) via
  `glicko2.lua`.
- TrueSkill update sanity via `trueskill.lua`.
- Elo predict/update via `elo.lua`; flat ±points via `flat.lua`.
- Detection ladder + escalation via `smurf.lua`.
- Metric correctness spot-checks (match_quality ≈ 1.0 for equal teams, etc.).
- Novel-system examples (`decay_elo`, `avg_rating_gap`) run.

## Steps

1. Run the verification matrix; fix any failures (with tests) and re-run.
2. Run `v0_1_basic` acceptance; compare against the recorded v0.1 numbers.
3. Run the determinism gate (two runs, byte-diff).
4. Audit the ported-golden tests; confirm all listed items pass.
5. Run the dogfood/comparison manifests from ticket 010 once each.
6. Final `AGENTS.md` touch-up if any recorded number changed.
7. Summarize results (numbers + any fixes) in the ticket as a completion note.

## Acceptance Criteria

- [ ] All five verification-matrix commands pass clean.
- [ ] `v0_1_basic` reproduces the acceptance numbers within tolerance through an
      entirely Lua-native pipeline.
- [ ] Determinism gate: two identical runs byte-identical (modulo `timestamp`).
- [ ] Ported-golden audit list all green.
- [ ] Every manifest from ticket 010 runs with exit 0 and non-empty results.
- [ ] No stale references to deleted Rust systems anywhere in `src/`, `crates/`,
      `experiments/`, `plugins/`, or docs (from ticket 009/011 sweeps).

## Testing

The acceptance criteria *are* the testing here: the full unit suite (all layer
tickets' tests), the CLI acceptance run, the determinism byte-diff, and the
golden audit. Any failure found is fixed with a regression test before
re-running.

## Risks / Notes

- If the Lua ports shift numbers slightly (e.g. Glicko floating-point), document
  the delta and update the recorded acceptance values — but only within a
  defensible tolerance (≤1% on means).
- The `rating_accuracy_by_time` trend is the key convergence evidence; if it
  flattens or rises, the logistic/elo port has a bug — do not accept without
  investigating.