# Ticket 09: Implement Glicko-2 Rating System

## Context
Implement the full Glicko-2 algorithm in `matchlab-rating`. Currently a stub with `todo!()`. This is the most complex rating system in the spec.

## Scope
- Implement `crates/matchlab-rating/src/glicko.rs` — full 6-step Glicko-2 algorithm
- Update `crates/matchlab-rating/src/plugins.rs` — register `"glicko2"` in registry
- Add Lua hooks support (`with_hooks()` constructor)

## Glicko-2 Algorithm Steps

1. **Scale conversion**: Convert rating/RD/volatility to Glicko-2 scale (μ, φ, σ)
   - `μ = (rating - 1500) / 173.7178`
   - `φ = RD / 173.7178`
   - `σ = volatility`

2. **Compute v** (estimated variance of team rating from outcomes):
   - For each opponent: `g(φ_j) = 1 / sqrt(1 + 3*φ_j²/π²)`
   - `E_i = 1 / (1 + exp(-g(φ_j) * (μ_i - μ_j)))`
   - `v = 1 / Σ(g(φ_j)² * E_i * (1 - E_i))`

3. **Compute Δ** (estimated improvement):
   - `Δ = v * Σ(g(φ_j) * (actual_outcome - E_i))`

4. **Iterate to find σ'** (new volatility):
   - Use Newton-Raphson on `f(σ) = exp(Δ²/(φ²+v+σ²)) - (σ²+τ²)/(φ²+v+σ²) - Δ²/(φ²+v+σ²)`
   - Iterate until convergence (epsilon threshold)

5. **Update φ* and compute new φ, μ**:
   - `φ* = 1 / sqrt(1/φ² + 1/(σ'² + v))` ... wait, actually:
   - `φ* = sqrt(φ² + σ'²)`
   - `μ' = μ + φ*² * Σ(g(φ_j) * (actual - E))`

6. **Convert back to rating scale**:
   - `rating' = 173.7178 * μ' + 1500`
   - `RD' = 173.7178 * φ*`
   - `volatility' = σ'`

## Types
```rust
pub struct GlickoConfig {
    pub initial_rating: f64,
    pub initial_rd: f64,
    pub initial_volatility: f64,
    pub tau: f64,
    pub epsilon: f64,
}

pub struct Glicko2RatingSystem {
    pub config: GlickoConfig,
    pub hooks: Option<LuaHooks>,
}
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-rating` succeeds
- [ ] `cargo test -p matchlab-rating` passes (all existing + new)
- [ ] Glicko-2 produces correct rating updates for known test cases
- [ ] Volatility increases after unexpected outcomes
- [ ] Volatility decreases after consistent outcomes
- [ ] RD decreases with more games played
- [ ] `from_yaml` parses config correctly
- [ ] `lua:glicko2` registry entry works with hooks
- [ ] Information budget is `WinLoss` only

## Testing
- Unit test: equal ratings → 50% win probability
- Unit test: known Glicko-2 test case (from Glickman's paper) matches expected output
- Unit test: volatility increases after upset win
- Unit test: RD decreases after multiple games
- Unit test: `from_yaml` with partial config uses defaults
- Unit test: `update` returns correct `RatingState` for all players

## Dependencies
- `matchlab-core`
- `matchlab-rating` (existing `system.rs`, `hooks.rs`)
- `serde` (workspace)

## References
- Glickman, M. E. (1999). "Parameter estimation in large dynamic paired comparison experiments"
- http://www.glicko.net/glicko/glicko2.pdf
