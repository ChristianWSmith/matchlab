# Ticket 08: Create matchlab-utility Crate

## Context
Create the player utility/satisfaction crate. Models player retention probability based on observable experience proxies (§16).

## Scope
- Create `crates/matchlab-utility/Cargo.toml` (deps: `matchlab-core`, `serde`)
- Create `crates/matchlab-utility/src/lib.rs` — re-exports
- Create `crates/matchlab-utility/src/satisfaction.rs` — `SatisfactionModel`, `PlayerExperience`, `SatisfactionWeights`

## Types

### SatisfactionWeights
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SatisfactionWeights {
    pub match_quality: f64,
    pub queue_time_penalty: f64,
    pub win_bonus: f64,
    pub loss_streak_penalty: f64,
    pub rank_progression_bonus: f64,
    pub fairness_sensitivity: f64,
    pub rematch_bonus: f64,
}

impl Default for SatisfactionWeights {
    fn default() -> Self {
        Self {
            match_quality: 1.0,
            queue_time_penalty: -0.01,
            win_bonus: 0.5,
            loss_streak_penalty: -0.3,
            rank_progression_bonus: 0.2,
            fairness_sensitivity: -0.8,
            rematch_bonus: 0.1,
        }
    }
}
```

### PlayerExperience
```rust
pub struct PlayerExperience {
    pub recent_match_qualities: Vec<f64>,
    pub recent_queue_times: Vec<f64>,
    pub recent_outcomes: Vec<bool>,
    pub current_streak: i32,
    pub rank_change: f64,
    pub perceived_fairness: f64,
    pub rematch_rate: f64,
}
```

### SatisfactionModel
```rust
pub struct SatisfactionModel {
    pub weights: SatisfactionWeights,
}

impl SatisfactionModel {
    pub fn satisfaction(&self, exp: &PlayerExperience) -> f64;
    pub fn retention_probability(&self, satisfaction: f64) -> f64;
    pub fn rematch_probability(&self, satisfaction: f64) -> f64;
}
```

## Scoring Logic
```
score = w_quality × avg_quality
      + w_queue × avg_queue
      + w_win × win_rate
      + w_streak × max(0, |streak| - 3)  [only for loss streaks < -3]
      + w_rank × rank_change
      + w_fair × (1 - fairness)
      + w_rematch × rematch_rate
```

Retention: `1 / (1 + exp(-satisfaction))` (logistic)
Rematch: `1 / (1 + exp(-0.5 × (satisfaction - 2.0)))` (higher threshold)

## Acceptance Criteria
- [ ] `cargo build -p matchlab-utility` succeeds
- [ ] `cargo test -p matchlab-utility` passes
- [ ] `satisfaction()` computes correct weighted score
- [ ] `retention_probability()` returns value in [0, 1]
- [ ] `rematch_probability()` returns value in [0, 1] with higher threshold than retention
- [ ] Default weights produce sensible scores for typical inputs
- [ ] Loss streak penalty only activates for streaks < -3

## Testing
- Unit test: `satisfaction` with all-default experience → baseline score
- Unit test: `satisfaction` with high match_quality → higher score
- Unit test: `satisfaction` with long queue times → lower score
- Unit test: `satisfaction` with loss streak of -5 → penalty applied
- Unit test: `satisfaction` with loss streak of -2 → no penalty
- Unit test: `retention_probability` is monotonic in satisfaction
- Unit test: `rematch_probability` < `retention_probability` at same satisfaction
- Unit test: `Default` weights match spec values

## Dependencies
- `matchlab-core`
- `serde` (workspace)
