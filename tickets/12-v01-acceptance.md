# Ticket 12 — v0.1 Acceptance (Exit Criteria Verification)

## Goal

Systematically verify every v0.1 exit criterion from `docs/spec.md` §17 /
AGENTS.md, run the full test suite, and reconcile `AGENTS.md` "Current State"
with the now-implemented codebase. This ticket is done only when all criteria
are demonstrably green.

## Scope / Deliverables

- Ensure the v0.1 experiment manifest produces a valid run:
  `cargo run -- run experiments/v0_1_basic.yaml`.
- Verify against each exit criterion (spec §17):
  1. `cargo run -- run experiments/v0_1_basic.yaml` builds and completes with
     exit 0.
  2. Produces `results/` containing metrics JSON.
  3. **Elo ratings converge** — MAE decreases over time. Add whatever is needed
     (e.g. a time-bucketed / per-snapshot MAE check, or an assertion in the
     runner/analysis) to demonstrate this. If convergence does not happen, this
     is a defect to fix (likely Elo scale, K-factor, or matchmaking imbalance),
     not something to paper over.
  4. **Match quality mean > 0.85** — assert from the `MatchQualityCollector`
     output.
  5. **Queue time measures actual wait** not match duration — confirmed by the
     `QueueTimeCollector` implementation and an assertion on real output.
  6. **All `cargo test` pass** — full workspace.
  7. **Same seed → identical results** — run twice, diff output hashes.
- Update `AGENTS.md` "Current State":
  - Change pre-implementation → v0.1 complete.
  - Record which crates exist and the dependency/commit reality of the repo.
  - Update AGENTS.md immediately on any discovered discrepancy per its
    "Keeping This File in Sync" rule.

## Acceptance criteria

- [ ] `cargo run -- run experiments/v0_1_basic.yaml` exits 0 and prints a
      summary.
- [ ] `results/` contains the three collector outputs (match_quality,
      queue_time, rating_accuracy) as JSON.
- [ ] Elo MAE demonstrably decreases across the run (grabbed from output /
      analysis, with recorded initial vs final MAE).
- [ ] `match_quality.mean` > 0.85.
- [ ] `queue_time` values correspond to wait, not match duration.
- [ ] `cargo test` green across the whole workspace.
- [ ] Two runs with `seed: 42` produce identical outputs (hash comparison).
- [ ] AGENTS.md "Current State" updated to reflect v0.1 as implemented.

## Testing

- The full `cargo test` suite (all prior tickets' tests must pass).
- A dedicated printout or report line per exit criterion in the run output so
  an auditor can confirm each one.
- A second `cargo run` invocation with the same seed; assert output equality
  (could be a script or an in-process test).

## Dependencies

Tickets 01–11.

## Notes

- Spec references: §17 "v0.1 Exit Criteria".
- This ticket is primarily **verification and reconciliation**. If a criterion
  fails, fix the underlying defect in the relevant component (do not weaken the
  criterion).
- The convergence check (criterion 3) may require the analysis layer (Ticket 11)
  to emit time-bucketed MAE; if Ticket 11 did not include that, add a minimal
  bucketed-MAE output here.
- After closing, optionally open follow-up tickets for post-v0.1 work
  (Glicko-2, TrueSkill, more metrics, detection, objective, adversarial,
  multidimensional skill) — do not implement them here.
