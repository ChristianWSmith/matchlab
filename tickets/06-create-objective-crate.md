# Ticket 06: Create matchlab-objective Crate

## Context
Create the objective function crate. Combines multiple metrics into a single utility score for experiment comparison while preserving raw values (§12).

## Scope
- Create `crates/matchlab-objective/Cargo.toml` (deps: `matchlab-core`, `matchlab-metrics`, `serde`)
- Create `crates/matchlab-objective/src/lib.rs` — re-exports
- Create `crates/matchlab-objective/src/utility.rs` — `ObjectiveWeights`, `ObjectiveFunction`

## Types

### ObjectiveWeights
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObjectiveWeights {
    pub match_quality: f64,
    pub queue_time: f64,
    pub rating_accuracy: f64,
    pub convergence_speed: f64,
    pub smurf_damage: f64,
    pub false_positive_rate: f64,
    pub streak_frustration: f64,
}

impl Default for ObjectiveWeights {
    fn default() -> Self {
        Self {
            match_quality: 1.0,
            queue_time: 0.5,
            rating_accuracy: 1.0,
            convergence_speed: 0.8,
            smurf_damage: 2.0,
            false_positive_rate: 1.5,
            streak_frustration: 0.3,
        }
    }
}
```

### ObjectiveFunction
```rust
pub struct ObjectiveFunction {
    pub weights: ObjectiveWeights,
}

impl ObjectiveFunction {
    pub fn evaluate(&self, metrics: &HashMap<String, MetricResult>) -> (f64, &HashMap<String, MetricResult>);
}
```

## Scoring Logic
- `match_quality`: positive weight × mean value
- `queue_time`: negative weight × mean (lower is better)
- `rating_accuracy`: negative weight × mean (lower error is better)
- `convergence`: negative weight × mean (fewer games is better)
- `smurf`: negative weight × damage value (index 3 of distribution)
- `streaks`: negative weight × p5 value (index 0 of distribution)

## Acceptance Criteria
- [ ] `cargo build -p matchlab-objective` succeeds
- [ ] `cargo test -p matchlab-objective` passes
- [ ] `evaluate()` returns correct score for known metric inputs
- [ ] `evaluate()` preserves raw metrics (never discards them)
- [ ] Default weights produce sensible scores

## Testing
- Unit test: `evaluate` with all-zero metrics → score = 0
- Unit test: `evaluate` with high match_quality → positive score contribution
- Unit test: `evaluate` with high queue_time → negative score contribution
- Unit test: `evaluate` returns reference to original metrics map
- Unit test: `Default` weights match spec values

## Dependencies
- `matchlab-core`
- `matchlab-metrics`
- `serde` (workspace)
