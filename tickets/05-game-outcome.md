# Ticket 05 — Game Outcome (v0.1 Build Order Step 4)

## Goal

Create `crates/matchlab-game` with the `OutcomeModel` trait and the
`LogisticOutcomeModel` that simulates a match from the two teams' observations
and produces a `MatchResult`.

## Scope / Deliverables

- `crates/matchlab-game/` depending on `matchlab-core`.
  - `outcome.rs` — `OutcomeModel` trait (spec §6.1):
    - `win_probability(team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64`
    - `simulate(match_id, team_a, team_b, rng) -> MatchResult`
  - `logistic.rs` — `LogisticOutcomeModel` (spec §6.2):
    - Fields `beta`, `noise`, and (present but unused in v0.1)
      `use_multidimensional`, `dimension_weights`.
    - `effective_skill(obs)` returns `obs.rating` in v0.1 (or the flat rating
      scalar). Multidimensional skill path is **out of scope** for v0.1.
    - `win_probability`: logistic of average-team-skill difference.
    - `simulate`: add noise to probability, pick winner, build a `MatchResult`
      with both team player-id lists, scores, per-player `PlayerPerformance`,
      duration, and `variance`.
- `lib.rs` re-exports.
- (Other outcome models — Variance, Composition, Fatigue, Momentum — are
  **out of scope** for v0.1. Do not create them.)

## Acceptance criteria

- [ ] Equal-rating teams produce `win_probability ≈ 0.5`.
- [ ] Over 10,000 simulated games between equal teams, the win rate is ≈ 50%
      (spec §17 Step 4 exit criterion).
- [ ] Favored team wins more often than the underdog over many games, with win
      rate tracking `win_probability`.
- [ ] `simulate` returns a well-formed `MatchResult` populated for every
      participating player.
- [ ] Simulation is deterministic for a given seed/team setup.

## Testing

- Equal teams → `win_probability` ≈ 0.5; imbalance shifts probability correctly.
- 10,000-game equal-match simulation → empirical win rate within tolerance.
- `simulate` produces distinct outcomes across seeds / variance (non-zero
  `variance` field).
- All players present in the returned `MatchResult`'s performances.

## Dependencies

Tickets 02 (types), 03 (SimRng; not strictly required but needed for
determinism).

## Notes

- Spec references: §6.1, §6.2, §17 Step 4.
- Keep `use_multidimensional`/`dimension_weights` fields present but inert in
  v0.1 so the multidimensional research path (spec §6.3) can be enabled later
  without a breaking change.
- Truth separation: `simulate`/`win_probability` consume `PlayerObservation`
  only — they must not look up `PlayerReality`.
