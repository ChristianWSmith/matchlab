# Ticket 14: Additional Metric Collectors

## Context
Implement the 10 remaining metric collectors specified in §11.3. Currently only `RatingAccuracy`, `MatchQuality`, and `QueueTime` exist.

## Scope
- Create `crates/matchlab-metrics/src/inequality.rs` — `MatchInequalityCollector`
- Create `crates/matchlab-metrics/src/ndcg.rs` — `NDCGCollector`
- Create `crates/matchlab-metrics/src/dimensionality.rs` — `DimensionalityFidelityCollector`
- Create `crates/matchlab-metrics/src/convergence.rs` — `ConvergenceCollector`
- Create `crates/matchlab-metrics/src/responsiveness.rs` — `ResponsivenessCollector`
- Create `crates/matchlab-metrics/src/stability.rs` — `StabilityCollector`
- Create `crates/matchlab-metrics/src/streaks.rs` — `StreakCollector`
- Create `crates/matchlab-metrics/src/population.rs` — `PopulationHealthCollector`
- Create `crates/matchlab-metrics/src/smurf.rs` — `SmurfMetricsCollector`
- Create `crates/matchlab-metrics/src/cohort.rs` — `CohortFilter` enum + `tier_for_skill()`
- Update `crates/matchlab-metrics/src/lib.rs` — add all module declarations + re-exports
- Add Lua hook integration to each collector

## Collectors

### MatchInequalityCollector
- Records win probability distribution across matches
- Spread metric: `(2*p - 1)²` — 0 at fair 0.5, 1 at lopsided
- Returns `Summary` of win probabilities

### NDCGCollector
- Normalized Discounted Cumulative Gain over match qualities
- Measures whether good matches cluster early vs late
- Returns `Scalar` (NDCG value)

### DimensionalityFidelityCollector
- Pearson correlation: 1D rating vs true skill
- Pearson correlation: SkillVector prediction vs true skill
- Fidelity = improvement from multiD over 1D
- Returns `Summary` with correlations

### ConvergenceCollector
- Tracks games until `|rating - true_skill| < threshold`
- Returns `Summary` of games-to-convergence

### ResponsivenessCollector
- Does rating move in direction consistent with outcome?
- Winners should gain, losers should lose
- Returns `Scalar` (fraction of correct-direction updates)

### StabilityCollector
- Rating variance for stable players (low improvement_rate)
- Returns `Scalar` (mean stddev of stable players' ratings)

### StreakCollector
- Tracks streak lengths (3, 5, 8, 10 games)
- Returns `Distribution` [p3, p5, p8, p10]

### PopulationHealthCollector
- Rating inflation/deflation over time
- Compression (stddev change)
- Returns `Distribution` [inflation, compression, initial_mean, final_mean]

### SmurfMetricsCollector
- Detection rate, false positive rate, damage, games-to-detection
- Per-archetype breakdown
- Returns `Summary`

### CohortFilter
```rust
pub enum CohortFilter {
    All,
    SkillRange(f64, f64),
    Archetype(String),
    GamesPlayedRange(u64, u64),
    Region(Region),
    PartySize(usize),
    SessionLength(f64, f64),
    RankTier(String),
    IsSmurfByProperties,
}
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-metrics` succeeds
- [ ] `cargo test -p matchlab-metrics` passes
- [ ] All 10 collectors implement `MetricCollector` trait
- [ ] Each collector returns correct `MetricResult` variant
- [ ] `CohortFilter::matches` correctly filters players
- [ ] `tier_for_skill` maps skill values to tier strings
- [ ] Lua hooks integrate with each collector

## Testing
- Unit test for each collector's `compute()` with known inputs
- Unit test: `CohortFilter::SkillRange` filters correctly
- Unit test: `CohortFilter::Archetype` filters correctly
- Unit test: `tier_for_skill` returns correct tier for boundary values
- Integration test: collectors record data from simulated matches

## Dependencies
- `matchlab-core`
- `matchlab-metrics` (existing `collector.rs`, `engine.rs`, `stats.rs`)
