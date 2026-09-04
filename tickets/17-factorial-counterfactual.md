# Ticket 17: Factorial Design + Counterfactual Evaluation

## Context
Add advanced experiment features: factorial design for multi-factor experiment generation and counterfactual evaluation for comparing rating systems on identical game history (§13.5-13.6).

## Scope
- Create `crates/matchlab-experiments/src/factorial.rs` — `FactorialDesign`, `Factor`, `generate_configs()`
- Create `crates/matchlab-experiments/src/counterfactual.rs` — `GameHistory`, `counterfactual_eval()`
- Update `crates/matchlab-experiments/src/lib.rs` — re-exports

## FactorialDesign
```rust
pub struct FactorialDesign {
    pub factors: Vec<Factor>,
}

pub struct Factor {
    pub name: String,
    pub values: Vec<serde_yaml::Value>,
}

impl FactorialDesign {
    pub fn generate_configs(&self, base: &ExperimentConfig) -> Vec<ExperimentConfig>;
}
```

Generates the Cartesian product of all factor values, producing N = Π|factor_i| configs. Each config is a deep-merge of the base config with the factor's value set at the specified path.

### Example
```rust
let design = FactorialDesign {
    factors: vec![
        Factor { name: "experiment.rating.systems.0.name", values: ["elo", "glicko2"] },
        Factor { name: "experiment.game.beta", values: [300.0, 400.0, 500.0] },
    ],
};
// Generates 2 × 3 = 6 configs
```

## Counterfactual Evaluation
```rust
pub struct GameHistory {
    pub matches: Vec<MatchResult>,
    pub player_snapshots: Vec<HashMap<PlayerId, PlayerObservation>>,
}

pub fn counterfactual_eval(
    history: &GameHistory,
    systems: &[(&str, Box<dyn RatingSystem>)],
) -> HashMap<String, Vec<(PlayerId, RatingState)>>;
```

Replays identical match history through multiple rating systems. Each system's full `RatingState` (rating, RD, volatility, games_played) is preserved so Bayesian systems update correctly.

## Acceptance Criteria
- [ ] `cargo build -p matchlab-experiments` succeeds
- [ ] `cargo test -p matchlab-experiments` passes
- [ ] `FactorialDesign::generate_configs` produces correct Cartesian product
- [ ] Factor values are correctly set at nested paths
- [ ] `counterfactual_eval` produces identical results for same system + history
- [ ] `counterfactual_eval` produces different results for different systems + same history
- [ ] Bayesian systems (Glicko-2, TrueSkill) update correctly across matches

## Testing
- Unit test: `FactorialDesign` with 2 factors of 2 values each → 4 configs
- Unit test: `FactorialDesign` with empty factors → 1 config (base)
- Unit test: factor value correctly set at nested path
- Unit test: `counterfactual_eval` with Elo vs FlatPoints on same history → different final ratings
- Unit test: `counterfactual_eval` with same system twice → identical results
- Unit test: `counterfactual_eval` with Glicko-2 → RD decreases over matches

## Dependencies
- `matchlab-core`
- `matchlab-experiments` (existing `config.rs`, `seed.rs`)
- `matchlab-rating` (for `RatingSystem` trait)
