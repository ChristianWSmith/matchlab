# Ticket 10 — Config + Runner + CLI (v0.1 Build Order Step 9)

## Goal

Create `crates/matchlab-experiments` (YAML config parsing + inheritance,
`SeedManager`, `ExperimentRunner`) and wire the `matchlab run <manifest>`
CLI into the top-level binary so a v0.1 experiment can actually be executed.

## Scope / Deliverables

- `crates/matchlab-experiments/` depending on the v0.1 crates (core, players,
  game, matchmaking, rating, metrics, analysis).
  - `config.rs` — serde config types mirroring spec §13.2, adapted to v0.1
    scope: `ExperimentConfig`, `ExperimentSpec`, `PopulationSpec`,
    `ArchetypeSpec`, `DistributionSpec`, `GameSpec`, `MatchmakingSpec`,
    `RatingSpec { systems }`, `MetricSpec` (list of names), `DurationSpec`,
    `OutputSpec`. Omit fields for detection/ranking/objectives/cohorts that are
    out of scope, but keep the schema forward-compatible (accept and ignore, or
    include optional fields).
    - Support the minimal v0.1 manifest exactly as in spec §17 ("v0.1 Minimal
      Experiment Manifest") — `population.size`, `game.team_size`,
      `matchmaking.algorithm: batch` with `batch_interval`,
      `rating.systems` (elo), `metrics` list, `duration`, `output`.
  - `inherit.rs` (or in `config.rs`) — **config inheritance**: a `base:` field
    loads a base YAML and deep-merges overrides (spec §13.3). Implement a typed
    or generic deep-merge for the config tree.
  - `seed.rs` — `SeedManager` deriving `population_seed`, `game_seed`,
    `arrival_seed`, `behavior_seed` from one `experiment_seed` via `derive()`
    (DefaultHasher), plus `hash_config()` and `git_commit_hash()` (spec §13.7).
  - `runner.rs` — `ExperimentRunner::run(&ExperimentConfig) -> ExperimentResult`:
    - construct `World` with the population seed, generate the population
      (single `stable` archetype), build the `EventEngine` with the handlers
      from Ticket 08, register the v0.1 collectors (Ticket 09),
    - select the rating system via `from_name`,
    - enforce the info budget before `update` (spec §8.2 filtering for the
      rating system),
    - `sim.run(until: SimTime::from_secs(duration.max_time))` and stop at
      `duration.matches`,
    - `metrics.finalize()`,
    - produce `ExperimentResult { experiment_id, name, config_hash, git_commit,
      timestamp, metrics, utility_score }`.
- **Binary/CLI** — update `src/main.rs`: `matchlab run <manifest.yaml>`:
  load (with inheritance) + parse config, invoke the runner, and delegate
  output/reporting to matchlab-analysis (Ticket 11). Print a summary to stdout.
- Add `experiments/v0_1_basic.yaml` (the minimal v0.1 manifest from spec §17).

## Acceptance criteria

- [ ] `matchlab run experiments/v0_1_basic.yaml` parses the config, runs the
      simulation, and exits 0 (spec §17 Step 9 exit criterion).
- [ ] Inheritance works: a child config with `base:` deep-merges overrides
      without clobbering sibling fields (spec §13.3).
- [ ] Seeds are derived deterministically from the experiment seed.
- [ ] `ExperimentResult` records config hash and git commit.
- [ ] Same seed + config ⇒ byte-identical `ExperimentResult` (spec exit
      criterion "Same seed → identical results").

## Testing

- Test config parsing from the YAML manifest (or an inline string) and that all
  fields deserialize.
- Test inheritance: base + override of `rating.systems` keeps the base's
  population/game unchanged.
- Test `SeedManager` determinism and distinctness of derived seeds.
- Integration: run the v0.1 basic experiment in-process and assert
  `metrics.finalize()` produced the three collectors' results and matches
  completed.

## Dependencies

Tickets 02–09; Ticket 11 (analysis) needed for JSON output — implement the
runner's in-memory result first, then complete output wiring with Ticket 11.

## Notes

- Spec references: §13 (experiments), §3.3 (plugin registry — call `from_name`),
  §8.2 (info-budget filter), §17 Step 9, and the minimal manifest in §17.
- The manifest uses `matchmaking.algorithm: batch` and `batch_interval: 10`;
  the handler in Ticket 08 consumes these.
- `utility_score` is optional in v0.1 (objective function is out of scope); leave
  it `None` unless cheap to include.
- CLI crate args: keep simple (`run <manifest>` subcommand). No framework dep
  needed.
