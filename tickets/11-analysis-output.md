# Ticket 11 — Analysis + Output (v0.1 Build Order Step 10)

## Goal

Create `crates/matchlab-analysis` with statistical summarization and JSON
output so experiment results land in `results/` as metrics JSON, and wire it
into the runner/CLI.

## Scope / Deliverables

- `crates/matchlab-analysis/` depending on `matchlab-core`, `matchlab-metrics`,
  `matchlab-experiments` (per AGENTS.md dependency flow; analysis depends on
  core + metrics + objective — objective is out of scope, so depend on core +
  metrics + experiments as needed).
  - `stats.rs` — `Summary` struct + `summary(&[f64]) -> Summary` with mean,
    median, p75/p90/p95/p99, stddev; and `summary_to_result(...)` producing a
    `MetricResult::Summary`. (Keep in sync with the helper in Ticket 09.)
  - `report.rs` — JSON / summary serialization for `ExperimentResult`,
    producing a readable report (spec §14.4): experiment name, config hash,
    git commit, utility score (if any), and the metrics table.
  - `export.rs` — JSON raw/output export (spec §14.5, **JSON only** for v0.1;
    parquet is out of scope). Write `results/` per `OutputSpec.directory` with
    the metrics JSON and optional raw matches/observations JSON.
- Wire `matchlab-analysis` into the runner (Ticket 10): after `sim.run` +
  `metrics.finalize()`, if `output.formats` includes `json`, write the metrics
  to `output.directory`. Ensure `results/` is gitignored (Ticket 01).
- This closes the v0.1 "Produces results/ with metrics JSON" exit criterion.

## Acceptance criteria

- [ ] `summary` produces all required percentiles and stddev for a known input.
- [ ] Running the v0.1 experiment writes a metrics JSON file under
      `output.directory` (e.g. `results/`).
- [ ] The JSON contains one entry per registered collector name with its
      `MetricResult`.
- [ ] The report includes config hash + git commit for reproducibility.
- [ ] Re-running with the same seed writes identical output content.

## Testing

- Unit: `summary` on a known `&[f64]` returns correct mean/median/percentiles.
- Unit: serializing an `ExperimentResult` to JSON round-trips.
- Integration: run the v0.1 experiment and assert the output file exists and
  parses as JSON with the three metric keys.

## Dependencies

Tickets 09 (metrics types), 10 (runner producing `ExperimentResult`).

## Notes

- Spec references: §14.1 (statistics), §14.4 (report), §14.5 (export), §17
  Step 10.
- Parquet, plots, HTML reports are **out of scope** for v0.1 (JSON only;
  `OutputSpec.formats: [json]`).
- The statistic helper appears in both `matchlab-metrics` (for collectors) and
  here — keep them consistent. If a shared helper crate is created, it must not
  violate the dependency flow in AGENTS.md.
