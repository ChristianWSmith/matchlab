# Ticket 06 — Rating Systems: Elo + FlatPoints (v0.1 Build Order Step 5)

## Goal

Create `crates/matchlab-rating` with the `RatingSystem` trait, and implement
two systems for v0.1: `EloRatingSystem` and `FlatPointsRatingSystem`.

## Scope / Deliverables

- `crates/matchlab-rating/` depending on `matchlab-core`.
  - `system.rs` — `RatingSystem` trait (spec §8.1):
    - `information_budget()` → `Vec<ObservationType>`
    - `initialize(player_id) -> RatingState`
    - `predict(team_a, team_b) -> f64`
    - `update(match_result, observations) -> HashMap<PlayerId, RatingState>`
    - convenience `rating()` / `uncertainty()` defaults
    - `RatingState { rating, rating_deviation, volatility, games_played }`
    - `ObservationType` enum (spec §8.1)
  - `elo.rs` — `EloRatingSystem` (spec §8.4):
    - Config `EloConfig { k_factor, initial_rating, beta }`, `from_yaml`.
    - `beta`-consistent scale: win probability uses `10^(d/div)` with
      `divisor = beta * ln(10)` so rating and game model agree (spec §8.4 note).
    - `update` applies K-factor * (actual − expected) to each team member.
  - `flat.rs` — `FlatPointsRatingSystem` (spec §8.3):
    - Config `FlatPointsConfig { win_points, loss_points, initial_rating }`,
      `from_yaml`.
    - Fixed ±points baseline.
  - `plugins/mod.rs` (or `src/plugins.rs`) — a `registry` with `all_systems()`
    and `from_name(name, &serde_yaml::Value) -> Option<Box<dyn RatingSystem>>`
    with arms for `elo` and `flatpoints` (spec §3.3). **Do not** register
    glicko2/trueskill yet (out of scope for v0.1).
- `lib.rs` re-exports.

> **Note on information-budget enforcement:** the spec's `filter.rs` (spec §8.2)
> is a separate mechanism added alongside the runner in Ticket 10. Wire it in
> Ticket 10; for this ticket, declare budgets correctly and keep `update` only
> reading `WinLoss` data (which Elo/Flat legitimately use).

## Acceptance criteria

- [ ] Elo: known ratings + an upset produce expected rating shifts
      (e.g. lower-rated winner gains more than a favorite winning).
- [ ] Equal-rating `predict` returns ≈ 0.5.
- [ ] FlatPoints: winner gains `win_points`, loser loses `loss_points`.
- [ ] `initialize` returns the configured initial rating.
- [ ] `from_name("elo", ...)` / `from_name("flatpoints", ...)` return the
      correct system; `from_name("glicko2", ...)` returns `None` (not yet
      implemented in v0.1).
- [ ] Both systems only consume `WinLoss` observation data.

## Testing

- Elo `equal_ratings_produce_50_percent` (spec §8.4).
- Elo: a strong player beating a weak one changes rating less than a weak
  player beating a strong one.
- FlatPoints: exact ±points arithmetic.
- `update` increments `games_played` for each participant.
- Registry `from_name` round-trips YAML config (k_factor, initial_rating, beta).

## Dependencies

Tickets 02 (types), 05 (MatchResult — from core, already available).

## Notes

- Spec references: §8.1, §8.3, §8.4, §17 Step 5.
- Accurate Elo behavior matters for v0.1 exit criterion "Elo ratings converge
  (MAE decreases over time)" (Ticket 12).
- Keep `beta`/log scale conversion exactly as in spec §8.4 so match-quality and
  counterfactual metrics are not corrupted by inconsistent scales.
