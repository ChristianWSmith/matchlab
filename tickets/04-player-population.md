# Ticket 04 — Player Population (v0.1 Build Order Step 3)

## Goal

Create `crates/matchlab-players` and implement population generation with
archetype configs plus the static skill process for v0.1.

## Scope / Deliverables

- `crates/matchlab-players/` depending on `matchlab-core`.
  - `archetype.rs` — `ArchetypeConfig` (serde `Deserialize`): `name`,
    `proportion`, `skill_distribution`, `skill_volatility`, `improvement_rate`,
    `play_frequency`, `session_length`, `quit_probability`, optional
    `initial_rating` (spec §5.7). `DistributionConfig` enum with `normal`,
    `uniform`, `log_normal` variants (spec §5.7).
  - `skill.rs` — `SkillProcess` with `improvement_rate`, `volatility`; for
    **v0.1 this is static** (no evolution), but implement `advance()` per spec
    §5.6 so later versions can enable it. For v0.1 the population is generated
    once at `t=0` and skill does not change.
  - `population.rs` — `PopulationConfig { size, archetypes }` and
    `PopulationGenerator::generate(config, rng) -> (Vec<PlayerReality>,
    Vec<PlayerObservation>)`:
    - Sample skill per player from the archetype distribution.
    - Rating = `archetype.initial_rating` if set, else the sampled skill
      (spec §5.8; this is the seed of the smurf-like mismatch, though v0.1 uses
      a single `stable` archetype).
    - Build reality + observation pairs with the spec'd field defaults
      (e.g. `rating_deviation: 350.0`, `games_played: 0`).
- `lib.rs` re-exports.

## Acceptance criteria

- [ ] Generating a 1000-player `stable` archetype (normal mean=1000, stddev=250)
      yields observed mean ≈ 1000 and stddev ≈ 250 (spec §3 exit criterion).
- [ ] `initial_rating`, when set, overrides the observation rating while true
      skill stays sampled — ground truth and observation diverge correctly.
- [ ] Archetype proportions convert to integer counts that sum to `size`.
- [ ] Generation is deterministic given the RNG seed.

## Testing

- Generate N players; assert mean/stddev within tolerance (e.g. ±5% for 1000).
- `initial_rating` set ⇒ `observation.rating == initial_rating` while
  `reality.skill.overall()` is near the distribution mean.
- `SkillProcess::advance` with `improvement_rate=0, volatility=0` returns the
  same skill (static v0.1 baseline).
- Determinism: same seed ⇒ identical populations.

## Dependencies

Tickets 02, 03 (core types; World used in later wiring).

## Notes

- Spec references: §5.7 (archetypes), §5.8 (population generator), §17 Step 3.
- The spec includes rich example archetypes (improving, declining, returning,
  volatile, new_player, smurf). For v0.1 only a single `stable` archetype is
  required, but keep the config schema general so those archetypes can be added
  via YAML without code changes.
- Do **not** expose a boolean smurf flag anywhere, per AGENTS.md principle 1.
