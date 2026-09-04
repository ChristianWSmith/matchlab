# Ticket 13: Additional Outcome Models

## Context
Implement the 5 outcome model variants specified in §6.3. Currently only `LogisticOutcomeModel` exists.

## Scope
- Create `crates/matchlab-game/src/variance.rs` — `VarianceOutcomeModel`
- Create `crates/matchlab-game/src/composition.rs` — `CompositionOutcomeModel`
- Create `crates/matchlab-game/src/performance.rs` — `PerformanceOutcomeModel`
- Create `crates/matchlab-game/src/fatigue.rs` — `FatigueOutcomeModel`
- Create `crates/matchlab-game/src/momentum.rs` — `MomentumOutcomeModel`
- Update `crates/matchlab-game/src/lib.rs` — add module declarations
- Add Lua hook integration to each model

## VarianceOutcomeModel
- Same as logistic but with larger noise envelope
- `variance_multiplier` amplifies the noise term
- `effective_skill` uses same logic as logistic

## CompositionOutcomeModel
- Reads from `SkillVector` dimensions with configurable weights
- `Effective team skill = Σ weighted_dimensions + synergy_bonus`
- Tests whether 1D ratings can capture multidimensional skill

## PerformanceOutcomeModel
- Individual performance stats affect win probability
- `performance_weight` controls how much kills/deaths/impact matter
- Higher-impact players on a team slightly boost win probability

## FatigueOutcomeModel
- Wraps a base `OutcomeModel`
- Session length degrades effective skill: `effective = base * (1 - fatigue_decay_rate * session_secs)`
- Models performance drop-off in long sessions

## MomentumOutcomeModel
- Wraps a base `OutcomeModel`
- Win/loss streaks slightly affect subsequent outcomes
- `momentum_factor` controls streak influence

## Acceptance Criteria
- [ ] `cargo build -p matchlab-game` succeeds
- [ ] `cargo test -p matchlab-game` passes
- [ ] All 5 models implement `OutcomeModel` trait
- [ ] `VarianceOutcomeModel` produces higher variance outcomes than logistic
- [ ] `CompositionOutcomeModel` uses SkillVector dimensions
- [ ] `PerformanceOutcomeModel` weights individual stats
- [ ] `FatigueOutcomeModel` degrades skill with session length
- [ ] `MomentumOutcomeModel` adjusts for streaks
- [ ] Lua hooks integrate with each model

## Testing
- Unit test: `VarianceOutcomeModel` with high multiplier → more upset wins
- Unit test: `CompositionOutcomeModel` with weighted dimensions → correct effective skill
- Unit test: `PerformanceOutcomeModel` with high-impact player → boosted win prob
- Unit test: `FatigueOutcomeModel` with long session → lower effective skill
- Unit test: `MomentumOutcomeModel` with 5-game win streak → boosted win prob
- Unit test: Each model's `win_probability` returns value in [0, 1]
- Unit test: Each model's `simulate` returns valid `MatchResult`

## Dependencies
- `matchlab-core`
- `matchlab-game` (existing `outcome.rs`, `logistic.rs`)
