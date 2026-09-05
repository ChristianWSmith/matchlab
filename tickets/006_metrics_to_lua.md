# 006 — Port metric collectors to Lua

## Summary

Make `MetricCollector` fully Lua-implementable and **delete the Rust metric
collectors**. `LuaMetricCollector` becomes the only way to build a metric; all
13 built-in metrics are ported to Lua scripts under `plugins/metrics/`. The
metrics engine, the `MetricResult` enum, and the canonical stats (`summary`)
stay in Rust.

## Context

`matchlab-metrics` currently has 13 Rust collectors plus hook-style `LuaHooks`.
Metrics are the sole legitimate reader of `PlayerReality` besides the
simulation, so the metric snapshot passed to Lua scripts **may include
`true_skill`** per participant (nothing else may see it).

The `MetricCollector` trait is unchanged: `name()`, `record_match(&mut self, mr,
world)`, `compute() -> MetricResult`, optional `time_buckets()`. The engine
folds `time_buckets` into `{name}_by_time` series — that machinery stays Rust.

## Scope

**In:**
- `LuaMetricCollector` adapter in `matchlab-metrics::lua`.
- Lua ports of the 12 named collectors (see below).
- Metric `name` + optional `time_buckets` declared by the script.
- Metric resolution: manifest names → `plugins/metrics/<name>.lua`.
- Runner wiring; loop test updates; manifest metrics sections; tests.

**Out:**
- No changes to `collector.rs` (`MetricResult`), `engine.rs`, or `stats.rs`.

## Design

### Lua contract (metric script)

```lua
-- required globals read at load:
name = "match_quality"                 -- metric key in results / objectives
-- optional global read at load:
time_buckets = { 0.0, 100.0, 200.0 }   -- or omit / return nil

function on_record(match_result, snapshot, config, context)
    -- match_result: table from matchlab-lua convert
    -- snapshot: metric snapshot — participant observations AND true_skill
    --           per participant (metrics may read reality)
    -- returns context  (accumulate samples in context, e.g. context.samples)
end

function compute(config, context)
    -- returns result table:
    --   { kind = "scalar", value = number }
    --   { kind = "distribution", values = { ... } }
    --   { kind = "summary", values = { ... } }      -- Rust computes the Summary
    --   { kind = "summary", mean, median, p75, p90, p95, p99, stddev }  -- explicit
end
```

Scripts accumulate samples in `context` (e.g. `context.samples = {}` appended in
`on_record`) and return a result table from `compute`. Rust owns the percentile
math via `summary_to_result` when the script returns `{ kind = "summary",
values = ... }`.

### `matchlab-metrics::lua` — `LuaMetricCollector`

```rust
pub struct LuaMetricCollector {
    vm: LuaVm,
    context: Mutex<Context>,
    metric_name: String,
    buckets: Option<Vec<f64>>,
}

impl LuaMetricCollector {
    /// `script` is resolved from a manifest metric name to plugins/metrics/<name>.lua.
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String>;
    // validate_script(path, &["on_record", "compute"])
    // read `name` global (required); read `time_buckets` global (optional,
    // a Lua array -> Vec<f64>)
}
```

`MetricCollector` impl:
- `name()` → `metric_name`.
- `record_match(mr, world)` → build `mr_tbl` + metric snapshot (participant
  observations with `true_skill`); `vm.call_required("on_record", ...)` → `ctx`;
  store.
- `compute()` → `vm.call_required("compute", (config, ctx))` → result table →
  `MetricResult` (map the four kinds above); store `ctx`.
- `time_buckets()` → `buckets.clone()`.

### Metric scripts (`plugins/metrics/`)

Port each Rust collector, preserving metric key + semantics:

- `match_quality.lua` — `1 - (|avg_a - avg_b|/400).clamp(0,1)` from observation
  ratings; summary.
- `queue_time.lua` — wait = `now - obs.queue_joined_at` per participant; summary.
- `rating_accuracy.lua` — |rating − true_skill| per participant; summary;
  declares `time_buckets` so the engine emits `rating_accuracy_by_time` (the
  convergence evidence).
- `match_inequality.lua` — expected win-probability distribution; summary.
- `ndcg.lua` — NDCG over the quality series; scalar.
- `dimensionality_fidelity.lua` — correlations of 1D rating and skill-vector
  prediction vs true overall skill; fidelity = multiD improvement; return the
  packed summary (or distribution) exactly as the Rust collector does.
- `convergence.lua` — games until `|rating - true_skill| < threshold`; summary
  (`Inf` when empty, as today).
- `responsiveness.lua` — fraction of updates moving in the outcome's direction;
  scalar.
- `stability.lua` — rating stddev for stable players (`improvement_rate` < 0.1,
  read from reality in the snapshot); scalar.
- `streaks.lua` — probabilities of 3/5/8/10-game streaks; distribution.
- `population_health.lua` — `[inflation, compression, initial_mean, final_mean]`;
  distribution.
- `smurf.lua` — per-smurf damage + detection events; identify smurfs by
  properties (high skill + low games), never a flag; the packed summary.

Note: `stability` and `smurf` need reality fields (`improvement_rate`,
`games_played`, `skill`) — the metric snapshot must include them. Extend the
`matchlab_lua::convert::metric_snapshot` to carry the needed reality fields per
participant (documented as metrics-only).

### Deletions

- `crates/matchlab-metrics/src/accuracy.rs`, `quality.rs`, `queue.rs`,
  `inequality.rs`, `ndcg.rs`, `dimensionality.rs`, `convergence.rs`,
  `responsiveness.rs`, `stability.rs`, `streaks.rs`, `population.rs`,
  `smurf.rs`, `hooks.rs`.
- Keep: `collector.rs`, `engine.rs`, `stats.rs`, `cohort.rs` (tier helper).
  Update `lib.rs`.

### Config + runner

- `config.rs` — metrics list stays `Vec<String>` (names). No structural change.
- `runner.rs::register_metrics` → for each name, resolve
  `plugins/metrics/<name>.lua` and `LuaMetricCollector::load`; unknown name →
  error (missing script). `runner.rs::register_metrics` keeps the error
  contract.
- Runner `MINI` manifest metrics unchanged (`match_quality`, `queue_time`,
  `rating_accuracy`).
- Runner `all_metric_collectors_register` test: all 12 names now resolve to
  scripts.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` tests: `MetricsEngine::new()` +
  `register(MatchQualityCollector::new())` → register the Lua
  `match_quality.lua` collector via the resolution helper.
- Manifests: metrics sections unchanged in shape (names resolve to scripts), so
  only the presence of the scripts matters.

## Steps

1. Implement `lua.rs` (`LuaMetricCollector`) + result-kind mapping.
2. Extend `matchlab_lua::convert::metric_snapshot` with metrics-only reality
   fields (`true_skill`, `improvement_rate`, `games_played`, `skill`).
3. Write the 12 scripts under `plugins/metrics/`.
4. Delete the Rust collector files and `hooks.rs`; update `lib.rs`.
5. Update `runner.rs::register_metrics`; update runner tests.
6. Update `machine.rs` tests (Lua match_quality helper).
7. Write ported tests (below).
8. Update `AGENTS.md` (metrics crate section).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] No reference to the 12 Rust collector types or metrics `LuaHooks` remains
      (grep-clean).
- [ ] Every manifest metric name resolves to an existing script; unknown name →
      `Err("unknown metric collector: <name>")` as today.
- [ ] `rating_accuracy.lua` produces a `rating_accuracy_by_time` series (via
      `time_buckets`), and the series mean decreases over the experiment on the
      v0.1 population (the acceptance convergence evidence).
- [ ] All 13 metric keys appear in `full_featured.yaml` results with
      type-appropriate `MetricResult` shapes (scalar / distribution / summary).
- [ ] `smurf.lua` and `stability.lua` read reality fields from the snapshot
      (metrics-only path) and produce the same values the Rust collectors did on
      the standard population (spot-check against recorded results).
- [ ] Determinism: metrics identical across two same-seed runs.
- [ ] `time_buckets` absent → no `_by_time` series for that metric (engine
      behavior unchanged).

## Testing

- Per-script unit tests: construct a `World` + `MatchResult`, call
  `record_match` + `compute`, assert the `MetricResult` shape and rough value
  (e.g. equal ratings → match_quality summary mean ≈ 1.0; queue_time measured
  from `queue_joined_at`).
- `rating_accuracy_by_time`: verify the engine folds the buckets and the series
  exists.
- Resolution: unknown metric name errors; script with `name` global is keyed by
  that name (not the filename).
- Adapter: context accumulation across `record_match` calls; missing `compute`
  → load error.
- Runner: `all_metric_collectors_register` (12 Lua metrics) passes;
  determinism test still byte-identical.

## Risks / Notes

- The `dimensionality_fidelity`, `smurf`, and `stability` collectors pack
  multiple numbers into `Summary` fields or read reality — port them field-for-
  field and add a spot-check against recorded baseline values from the current
  Rust runs.
- Keep `MetricResult::Summary` math in Rust (canonical `stats`), so Lua scripts
  returning `{ kind = "summary", values = ... }` delegate to
  `summary_to_result`. This keeps percentiles consistent across scripts.