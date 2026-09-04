# Ticket 02 — Core Types (v0.1 Build Order Step 1)

## Goal

Create the `crates/matchlab-core` crate and implement the fundamental types the
entire simulation is built on: time, identity, RNG, skill, and the player/match
data structures.

## Scope / Deliverables

Core crate at `crates/matchlab-core/` with modules per spec §4–§6:

- `time.rs` — `SimTime(u64)` newtype (nanosecond internal, `u64`):
  - `ZERO`, `from_secs(f64)`, `from_millis(u64)`, `as_secs_f64()`,
    `duration_since(SimTime)`, `ticks()` (spec §4.1)
- `player.rs` — `PlayerId(u64)`; `Region` enum (`NA`, `EU`, `Asia`, `Other`);
  `SkillVector` (HashMap dimension → value, `one_dimensional`, `overall`,
  `weighted_overall`) (spec §5.3); `VisibleRank`, `DetectionFlag`
  (spec §5.5); `PlayerReality` (ground truth, spec §5.4); `PlayerObservation`
  (what algorithms see, spec §5.5)
- `match_.rs` — `MatchId(u64)`, `Team{A,B}`, `MatchState`, `MatchResult`,
  `PlayerPerformance`, `MatchConfig` (spec §6.4)
- `rng.rs` — `SimRng` deterministic wrapper. Decide base: `SmallRng` from
  `rand` with `seed_from_u64`, providing `gen_range`, `gen_bool`,
  `sample_normal`, `gen_u64` (spec §4.6 uses Box-Muller; keep it deterministic)
- `lib.rs` — re-exports the public types.

Follow the exact `PlayerReality` / `PlayerObservation` field sets in the spec.
Field order and types must match so later tickets (population generation,
metrics, Elo) can rely on them.

## Acceptance criteria

- [ ] `crates/matchlab-core` compiles (`cargo build -p matchlab-core`).
- [ ] `SimTime` supports nanosecond ticks, secs/millis conversion, and
      `duration_since` with saturating arithmetic.
- [ ] `PlayerReality` and `PlayerObservation` are distinct types with the
      spec'd fields; `PlayerReality` is never accessible to algorithms by
      design (enforced later via World API, Ticket 03).
- [ ] `SkillVector` supports 1D and weighted multidimensional overall.
- [ ] `SimRng` is fully deterministic: same seed ⇒ same sequence.

## Testing

- `time.rs`: `from_secs`/`as_secs_f64` round-trip; `from_millis`;
  `duration_since` ordering and saturating behavior.
- `rng.rs`: same seed ⇒ identical `gen_range` sequence; two different seeds
  diverge; `sample_normal` mean ~ requested mean over many draws.
- `player.rs`: `SkillVector::one_dimensional(...).overall()` returns the value;
  `weighted_overall` respects weights.
- ID generation is tested in Ticket 03 (World owns the counters).

## Dependencies

Ticket 01 (workspace foundation).

## Notes

- Spec references: §4.1 (time), §5.3–§5.5 (player model + skill), §6.4
  (match types), §4.6 (RNG).
- v0.1 is **1D static skill**; `SkillVector` only ever carries the `overall`
  dimension — but keep the type general (it already is).
- Do not yet implement `World`/`EventEngine` (Ticket 03), but the types here
  must be shaped so Ticket 03 slots them in without churn.
- `match_.rs` (file name) not `match.rs` (Rust keyword).
