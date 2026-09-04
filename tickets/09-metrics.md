# Ticket 09 — Metrics (v0.1 Build Order Step 8)

## Goal

Create `crates/matchlab-metrics` with the `MetricCollector` trait, a
`MetricsEngine` aggregator, an internal stats helper, and the three v0.1
collectors: `RatingAccuracyCollector`, `MatchQualityCollector`,
`QueueTimeCollector`.

## Scope / Deliverables

- `crates/matchlab-metrics/` depending on `matchlab-core` **only** (per the
  dependency flow in AGENTS.md).
  - `collector.rs` — `MetricCollector` trait: `name()`, `record_match(mr,
    world)`, `compute() -> MetricResult`; and `MetricResult` enum
    (`Scalar`, `Distribution`, `Summary { mean, median, p75, p90, p95, p99,
    stddev }`, `Histogram`) (spec §11.2).
  - `engine.rs` — `MetricsEngine`: register collectors, `record_match(mr,
    world)`, `finalize()`, `results()` (spec §11.1).
  - `stats.rs` — `summary(&[f64]) -> Summary` and `summary_to_result(...)`
    (share the same logic as spec §14.1; implement here so collectors can use
    it, keeping the crate self-contained as AGENTS.md requires metrics only
    depend on core).
  - `accuracy.rs` — `RatingAccuracyCollector`: MAE of `obs.rating` vs
    `reality.skill.overall()` (spec §11.3). This reads ground truth — allowed
    for metrics (simulation logic), confirming exit criterion "MAE decreases
    over time".
  - `quality.rs` — `MatchQualityCollector`: `1 - (|avg_a - avg_b|/400).clamp` on
    observation ratings, summarized (spec §11.3).
  - `queue.rs` — `QueueTimeCollector`: actual wait = `now.duration_since(
    obs.queue_joined_at)` for each participant (spec §11.3). **Must measure
    wait, not match duration** (v0.1 exit criterion).
- `lib.rs` re-exports.

> **Truth separation note:** metrics are the *only* consumer of `PlayerReality`
> besides the simulation itself. Do not let collectors leak reality into
> algorithms — collectors are read-only aggregators at the end/timeline of the
> run.

## Acceptance criteria

- [ ] Each collector computes correct values from a hand-built `World` +
      `MatchResult`.
- [ ] `QueueTimeCollector.times_secs` reflects join→formation wait and **not**
      match duration (assert with a contrived long-duration match: wait stays
      small).
- [ ] `MatchQualityCollector` mean satisfies the v0.1 threshold (> 0.85) once
      the batch matchmaker balances teams (checked in Ticket 12).
- [ ] `MetricsEngine` aggregates all collectors and `finalize()` produces a
      `HashMap<String, MetricResult>`.

## Testing

- Unit: 5v5 match with equal average ratings → quality ≈ 1.0.
- Unit: queue_time equals `world.time - queue_joined_at` for a synthetic entry;
      verify a long match duration does not inflate it.
- Unit: rating_accuracy reported as mean absolute error from known reality.
- Engine: register 3 collectors, record several matches, `finalize`, assert all
  three keys present.

## Dependencies

Tickets 02 (types), 07 (used in real flow), 04 (population for accuracy data).

## Notes

- Spec references: §11.1–§11.3, §17 Step 8.
- Only the three v0.1 collectors are in scope. Other collectors in spec §11.3
  (inequality, ndcg, dimensionality, correlation, convergence, responsiveness,
  stability, streaks, population, smurf, cohort, rank accuracy) are **out of
  scope** for v0.1.
- `summary`/`summary_to_result` duplicated here vs matchlab-analysis is
  intentional to respect the dependency boundary (metrics depends only on core;
  analysis depends on metrics). Keep them in sync; a small shared helper crate
  is an acceptable alternative if it doesn't violate the dependency flow.
