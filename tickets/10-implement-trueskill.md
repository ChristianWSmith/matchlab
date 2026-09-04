# Ticket 10: Implement TrueSkill Rating System

## Context
Implement the TrueSkill algorithm in `matchlab-rating`. Currently a stub with `todo!()`. Bayesian Gaussian skill estimation with team support.

## Scope
- Implement `crates/matchlab-rating/src/trueskill.rs` — Bayesian Gaussian update
- Update `crates/matchlab-rating/src/plugins.rs` — register `"trueskill"` in registry
- Add Lua hooks support (`with_hooks()` constructor)

## TrueSkill Algorithm

### State
Each player has `(μ, σ²)` — mean skill and variance.

### Update (simplified for 2-team matches)
1. Compute team performance distributions:
   - `μ_team = Σ μ_i` (sum of player means)
   - `σ²_team = Σ σ²_i + n * β²` (sum of variances + noise)

2. Compute win probability:
   - `c = sqrt(n * β² + Σ σ²_i)`
   - `δ = (μ_team_a - μ_team_b) / c`
   - `P(A wins) = Φ(δ / c)` where Φ is standard normal CDF

3. Update via truncated Gaussian conditioning:
   - For winning team: `v = V_cut(δ/c)`, `w = W_cut(δ/c)`
   - For losing team: `v = V_cut(-δ/c)`, `w = W_cut(-δ/c)`
   - `μ_i' = μ_i + (σ²_i / c) * v`
   - `σ²_i' = σ²_i * (1 - (σ²_i / c²) * w)`

Where `V_cut(t) = φ(t) / Φ(t)` and `W_cut(t) = V_cut(t) * (V_cut(t) + t)`, with φ = normal PDF, Φ = normal CDF.

## Types
```rust
pub struct TrueSkillConfig {
    pub initial_mean: f64,
    pub initial_variance: f64,
    pub beta: f64,
    pub dynamics: f64,
    pub draw_probability: f64,
}

pub struct TrueSkillRatingSystem {
    pub config: TrueSkillConfig,
    pub hooks: Option<LuaHooks>,
}
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-rating` succeeds
- [ ] `cargo test -p matchlab-rating` passes (all existing + new)
- [ ] TrueSkill produces correct rating updates for known test cases
- [ ] μ increases for winners, decreases for losers
- [ ] σ² decreases with more games (confidence increases)
- [ ] Team size > 1 works correctly (sum of means/variances)
- [ ] `from_yaml` parses config correctly
- [ ] `lua:trueskill` registry entry works with hooks
- [ ] Information budget is `WinLoss` only

## Testing
- Unit test: equal skills → 50% win probability
- Unit test: winner's μ increases, loser's μ decreases
- Unit test: σ² decreases after each game
- Unit test: team of 3 vs team of 3 works correctly
- Unit test: `from_yaml` with partial config uses defaults
- Unit test: `update` returns correct `RatingState` for all players

## Dependencies
- `matchlab-core`
- `matchlab-rating` (existing `system.rs`, `hooks.rs`)
- `serde` (workspace)

## References
- Herbrich, R., Graepel, T., & Minka, T. (2006). "TrueSkill: A Bayesian Skill Rating System"
- https://www.microsoft.com/en-us/research/publication/trueskill-a-bayesian-skill-rating-system/
