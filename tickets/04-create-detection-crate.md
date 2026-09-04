# Ticket 04: Create matchlab-detection Crate

## Context
Create the detection crate from scratch. Implements the `DetectionSystem` trait, smurf detection logic, and intervention policies as specified in §9 of the spec.

## Scope
- Create `crates/matchlab-detection/Cargo.toml` (deps: `matchlab-core`, `serde`)
- Create `crates/matchlab-detection/src/lib.rs` — re-exports
- Create `crates/matchlab-detection/src/detector.rs` — `DetectionSystem` trait, `DetectionResult`
- Create `crates/matchlab-detection/src/intervention.rs` — `InterventionAction` enum, `InterventionPolicy`, `PlayerInterventionState`
- Create `crates/matchlab-detection/src/smurf.rs` — `SmurfDetector` implementation

## Types

### DetectionSystem Trait
```rust
pub trait DetectionSystem: Send + Sync {
    fn observe(&mut self, match_result: &MatchResult, world: &World);
    fn evaluate(&self, player_id: PlayerId, world: &World) -> DetectionResult;
    fn recommend_action(&self, result: &DetectionResult) -> InterventionAction;
}
```

### DetectionResult
```rust
pub struct DetectionResult {
    pub player_id: PlayerId,
    pub probability_of_anomaly: f64,
    pub confidence: f64,
    pub evidence: Vec<String>,
}
```

### InterventionAction
```rust
pub enum InterventionAction {
    None,
    AccelerateRating { multiplier: f64 },
    IncreaseKFactor { new_k: f64 },
    FlagForReview,
    RestrictQueue { duration_ticks: u64 },
    TempBan { duration_ticks: u64 },
    Probation { duration_ticks: u64 },
    Ban,
}
```

### SmurfDetector
- Tracks per-player recent performance vs expected (from visible rating)
- Uses `VecDeque` for rolling window of last 20 performances
- Counts consecutive anomalous games (exceeds sigma threshold)
- Flags when `consecutive_anomalous >= min_anomalous_games`
- Default: `sigma_threshold = 3.0`, `min_anomalous_games = 5`

## Acceptance Criteria
- [ ] `cargo build -p matchlab-detection` succeeds
- [ ] `cargo test -p matchlab-detection` passes
- [ ] `SmurfDetector` correctly identifies consecutive anomalous performances
- [ ] `InterventionPolicy` escalates thresholds based on prior interventions
- [ ] `DetectionResult` probability ramps with anomalous streak length
- [ ] Truth separation: detector only uses `world.observations`, never `world.players`

## Testing
- Unit test: `SmurfDetector::observe` tracks performance history correctly
- Unit test: `SmurfDetector::evaluate` returns low probability for normal player
- Unit test: `SmurfDetector::evaluate` returns high probability after 5+ consecutive anomalies
- Unit test: `InterventionPolicy::apply` returns None below threshold
- Unit test: `InterventionPolicy::apply` returns escalating actions above thresholds
- Unit test: prior interventions lower effective thresholds (escalation factor)

## Dependencies
- `matchlab-core`
- `serde` (workspace)
