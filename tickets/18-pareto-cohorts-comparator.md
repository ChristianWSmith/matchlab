# Ticket 18: Pareto Frontier, Cohort Analysis, Comparator

## Context
Add advanced analysis tools: Pareto frontier computation, cohort analysis engine, and multi-experiment comparator (§14.2-14.3, 14.6).

## Scope
- Create `crates/matchlab-analysis/src/pareto.rs` — `ParetoPoint`, `pareto_front()`, `dominates()`
- Create `crates/matchlab-analysis/src/cohorts.rs` — `CohortResult`, `analyze_cohort()`, `cohort_rating_accuracy()`
- Create `crates/matchlab-analysis/src/comparator.rs` — `Comparator`, `MetricComparison`, `metric_comparison()`, `ranking()`
- Update `crates/matchlab-analysis/src/lib.rs` — re-exports

## Pareto Frontier
```rust
pub struct ParetoPoint {
    pub label: String,
    pub values: Vec<f64>,
}

pub fn pareto_front<'a>(
    points: &'a [ParetoPoint],
    higher_is_better: &[bool],
) -> Vec<&'a ParetoPoint>;

fn dominates(a: &ParetoPoint, b: &ParetoPoint, higher_is_better: &[bool]) -> bool;
```

A point dominates another if it is at least as good on all dimensions and strictly better on at least one. The Pareto frontier is the set of non-dominated points.

## Cohort Analysis
```rust
pub struct CohortResult {
    pub name: String,
    pub player_count: usize,
    pub metrics: HashMap<String, MetricResult>,
}

pub fn analyze_cohort(
    name: &str,
    filter: &CohortFilter,
    world: &World,
    full_metrics: &MetricsEngine,
) -> CohortResult;
```

Filters players by cohort (skill range, archetype, games played, etc.) and computes per-cohort metrics.

## Comparator
```rust
pub struct Comparator {
    pub results: Vec<ExperimentResult>,
    pub baseline: Option<usize>,
}

impl Comparator {
    pub fn metric_comparison(&self) -> HashMap<String, Vec<MetricComparison>>;
    pub fn ranking(&self) -> Vec<(&ExperimentResult, f64)>;
}

pub struct MetricComparison {
    pub experiment: String,
    pub value: MetricResult,
}
```

Compares multiple experiment results side-by-side, with optional baseline for delta computation.

## Acceptance Criteria
- [ ] `cargo build -p matchlab-analysis` succeeds
- [ ] `cargo test -p matchlab-analysis` passes
- [ ] `pareto_front` correctly identifies non-dominated points
- [ ] `pareto_front` with `higher_is_better` flags works correctly
- [ ] `analyze_cohort` filters players correctly and computes metrics
- [ ] `Comparator::metric_comparison` returns all metrics for all experiments
- [ ] `Comparator::ranking` sorts by utility score descending
- [ ] `Comparator::set_baseline` enables delta computation

## Testing
- Unit test: `pareto_front` with 3 points where 1 dominates 2 → returns 2 points
- Unit test: `pareto_front` with all non-dominated → returns all
- Unit test: `pareto_front` with mixed higher/lower is better
- Unit test: `analyze_cohort` with `SkillRange` filter → correct player subset
- Unit test: `analyze_cohort` with `Archetype` filter → correct player subset
- Unit test: `Comparator::ranking` sorts by utility score
- Unit test: `Comparator::metric_comparison` includes all experiments

## Dependencies
- `matchlab-core`
- `matchlab-metrics` (for `MetricResult`, `MetricsEngine`, `CohortFilter`)
- `matchlab-experiments` (for `ExperimentResult`)
- `matchlab-analysis` (existing `stats.rs`, `report.rs`)
