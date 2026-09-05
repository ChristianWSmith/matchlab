# matchlab — Agent Orientation

## What This Project Is

matchlab is a **discrete-event simulation framework** written in Rust (edition 2024) for evaluating competitive matchmaking and rating systems. It generates synthetic player populations with known ground truth, runs them through a simulated matchmaking ecosystem, and measures algorithm performance with real metrics.

It answers questions like: *Under what conditions does Elo outperform Glicko-2?* and *How much match quality must be sacrificed to reduce queue time by 50%?*

**Spec:** `docs/spec.md` is the authoritative design document. Read it before making non-trivial changes.

---

## Keeping This File in Sync

This file is the source of truth for how agents understand the project. It must stay locked to the actual state of the repo. **If you discover or intentionally introduce a discrepancy between this file and the codebase, update this file immediately** — do not defer it or leave a stale note.

Common situations that require an update:
- A crate was added, removed, or renamed
- A new module or key type was introduced (or an existing one changed significantly)
- The build order progressed and the "Current State" section is outdated
- A dependency changed (added/removed/upgraded)
- A convention was adopted or broken in practice
- The spec diverged from what this file describes

When updating, keep the same terse style. Do not narrate what changed — just make the file correct. If a section's content is wrong, rewrite it; do not append corrections.

---

## Architecture

A Cargo workspace under `crates/` with one binary at `src/main.rs`:

```
match-lab/              # workspace root
├── Cargo.toml          # workspace root (NOT a library crate)
├── src/main.rs         # CLI binary: `matchlab run <manifest>`
└── crates/
    ├── matchlab-core/          # simulation engine, time, events, world, RNG, core types
    ├── matchlab-players/       # archetypes, population generation, skill process
    ├── matchlab-game/          # outcome models, match execution
    ├── matchlab-matchmaking/   # queue, matchmaker, constraints, search strategies
    ├── matchlab-rating/        # rating systems (Elo, Glicko-2, TrueSkill, Flat)
    ├── matchlab-detection/     # smurf detection, interventions
    ├── matchlab-ranking/       # rank mapping, leaderboard
    ├── matchlab-loop/          # simulation loop, event handlers, machine state
    ├── matchlab-metrics/       # metric collectors (accuracy, quality, queue time, etc.)
    ├── matchlab-objective/     # weighted utility, multi-objective scoring
    ├── matchlab-adversarial/   # adversarial player agents (boosters, derankers, etc.)
    ├── matchlab-utility/       # player satisfaction / retention model
    ├── matchlab-experiments/   # runner, YAML config, factorial design, counterfactual eval
    └── matchlab-analysis/      # statistics, Pareto frontier, cohorts, reports
```

**Dependency flow** (each layer only depends on layers below it):
```
matchlab-core          ← no internal deps
    ↑
    ├── matchlab-players
    ├── matchlab-game          (+ mlua)
    ├── matchlab-matchmaking   (+ mlua)
    ├── matchlab-rating        (+ mlua)
    ├── matchlab-detection     (+ mlua)
    ├── matchlab-ranking
    ├── matchlab-metrics       (depends on core only; + mlua for hooks)
    ├── matchlab-objective     (depends on core + metrics)
    ├── matchlab-adversarial   (depends on core)
    └── matchlab-utility       (depends on core)

matchlab-loop          (depends on core + players + game + rating + matchmaking + metrics)
matchlab-experiments   (depends on core + players + game + rating + matchmaking + loop + metrics)
matchlab-analysis      (depends on core + metrics + experiments; objective when it exists)
matchlab (binary)      (depends on experiments + analysis)
```

---

## Design Principles

### 1. Truth Separation (Critical)

The simulation maintains two parallel representations of every player:

- **`PlayerReality`** — ground truth the simulation knows but algorithms never see.
- **`PlayerObservation`** — what rating/matchmaking/detection systems see, derived only from permitted data.

**No algorithm, matchmaker, or detection system may call `world.players[pid]` directly.** They must use `world.observations[pid]`. Violating this corrupts the entire experiment.

Exception: the **outcome model** (the game, i.e. the simulator of reality) may — and must — decide match winners from ground truth. It reads ground truth through the observation binding: `PlayerObservation.skill_vector`/`hidden_mmr` are set from `PlayerReality.skill` at population generation, and `LogisticOutcomeModel::effective_skill` uses `skill_vector.overall()`. This is what makes "Elo converges; MAE decreases" a real property: without it, outcomes depend only on ratings, the loop closes, and ratings random-walk instead of learning. **Rating systems, matchmaking, and detection must never read `skill_vector`/`hidden_mmr`** — those fields exist solely for the outcome model's benefit.

A "smurf" is not a player type or boolean flag — it is the combination of high `true_skill` with low `initial_rating` and few `games_played`. Detection systems must infer smurf status from observable behavior.

### 2. Pluggability via Traits

Every algorithm is a trait implementation:
- `RatingSystem` (trait) — Elo, Glicko-2, TrueSkill, FlatPoints
- `OutcomeModel` (trait) — Logistic, Variance, Composition, Fatigue, etc.
- `Matchmaker` (trait) — ExpandingWindow, Strict, Batch, HubSpoke
- `MetricCollector` (trait) — each metric is its own collector
- `DetectionSystem` (trait) — smurf detector, etc.

Swapping implementations should require zero changes outside the relevant crate.

### 3. Reproducibility

Every experiment is deterministic given its config + seed. The `SeedManager` derives separate seeds for population, games, arrivals, and behavior from a single experiment seed. `ExperimentResult` records config hash + git commit for exact reproduction.

### 4. Multi-Scale Time

`SimTime` is nanosecond resolution internally (`u64`). The event engine skips idle periods — if the next event is in 3 days, the clock jumps directly there. No wasted computation.

---

## Key Types (matchlab-core)

| Type | Purpose |
|------|---------|
| `SimTime(u64)` | Monotonic simulation clock, nanosecond internal |
| `PlayerId(u64)` | Newtype player identifier |
| `MatchId(u64)` | Newtype match identifier |
| `SimRng` | Deterministic RNG wrapper (`SmallRng` seeded from `u64`) |
| `SkillVector` | Named dimensions map (`HashMap<String, f64>`); v0.1 uses 1D |
| `PlayerReality` | Full ground truth — never exposed to algorithms |
| `PlayerObservation` | What algorithms see — rating, RD, games_played, etc. |
| `MatchResult` | Winner, teams, scores, per-player performances |
| `World` | Holds `players` (reality), `observations`, `matches`, `rng`, `time` |
| `EventEngine` | Priority queue of timestamped events + handler dispatch |
| `Simulation` | Composed `World` + `EventEngine`; call `.run(until)` |

---

## Current State

The workspace is fully implemented: 14 crates under `crates/`, a binary at `src/main.rs`. The root `Cargo.toml` is a Cargo workspace:

```
crates/
├── matchlab-core/
├── matchlab-players/
├── matchlab-game/
├── matchlab-matchmaking/
├── matchlab-rating/
├── matchlab-loop/
├── matchlab-metrics/
├── matchlab-experiments/
├── matchlab-analysis/
├── matchlab-detection/
├── matchlab-ranking/
├── matchlab-objective/
├── matchlab-adversarial/
└── matchlab-utility/
```

- `[workspace.dependencies]` declares `serde` (derive), `serde_yaml 0.9`,
  `rand 0.8`, `rand_chacha 0.3`, `mlua 0.10` (lua54, vendored);
  `[workspace.package]` sets `edition = "2024"`.
- `src/main.rs` is the `match-lab` binary with a `matchlab run <manifest>`
  CLI; it depends on `matchlab-experiments` and `matchlab-analysis`.
- `experiments/base/` exists (empty, for inherited base configs).
- `.github/workflows/ci.yml` runs build + test + check + clippy + fmt.
- `/results` is gitignored.
- `cargo build --workspace`, `cargo test --workspace`, and
  `cargo check --workspace` all pass.

**`matchlab-core` is implemented with the core types:**
- `time.rs` — `SimTime` (nanosecond `u64`), `ZERO`, `from_secs`/`from_millis`,
  `as_secs_f64`, `duration_since` (saturating), `ticks`.
- `rng.rs` — `SimRng` deterministic wrapper (`SmallRng` seeded from `u64`) with
  `gen_range`, `gen_bool`, `sample_normal` (Box-Muller), `gen_u64`. Requires the
  `small_rng` feature of `rand`; note `rand::Rng::gen` must be written
  `r#gen` in edition 2024.
- `player.rs` — `PlayerId`, `Region`, `SkillVector`, `VisibleRank`,
  `DetectionFlag`, `PlayerReality` (ground truth), `PlayerObservation`.
- `match_.rs` — `MatchId`, `Team`, `MatchState`, `MatchResult`,
  `PlayerPerformance`, `MatchConfig`.
- `event.rs` — `Event` trait (`time()`/`kind()`/`as_any()`), 13-variant
  `EventKind`, `TimestampedEvent` (min-heap ordered on `SimTime`), `EventHandler`
  (`Fn(&mut World, &dyn Event) -> Vec<Box<dyn Event>> + Send + Sync`),
  10 concrete events (PlayerJoin/Leave/Queue/Quit/Return/Disconnect, MatchFormed,
  MatchEnd, SkillChange, MatchTimer), plus a checked `downcast::<T>()` helper.
  The `Any`-based `as_any()` lets handlers recover a concrete event's payload
  (`downcast_ref`) after matching on `kind()` — this is how event handlers
  read `player_id`/`match_id`/teams. `EventEngine` (register_handler/schedule/
  next_event/peek_time/is_empty/tick).
- `world.rs` — `World` holding `players`, `observations`, `matches`, `rng`,
  `time`, with private monotonic ID counters (`next_player_id()`/`next_match_id()`).
  Truth separation: `player.rs`/`world.rs` enforce the rule that algorithms
  access players via `observe()`/`observations`, never `players`/`reality()`.
- `simulation.rs` — `Simulation { world, engine }` with `new`, and
  `run(until)` / `run_to_completion()` (skips idle clock periods).

**`matchlab-players` is implemented with the population logic:**
- `archetype.rs` — `ArchetypeConfig` (serde `Deserialize`: `name`, `proportion`,
  `skill_distribution`, `skill_volatility`, `improvement_rate`, `play_frequency`,
  `session_length`, `quit_probability`, optional `initial_rating`) and
  `DistributionConfig` (tagged enum: `normal`, `uniform`, `log_normal`). The
  optional `initial_rating` overrides visible rating while true skill stays
  sampled — the seed of the smurf-like mismatch; no boolean smurf flag exists.
- `skill.rs` — `SkillProcess { improvement_rate, volatility }` with
  `advance(&SkillVector, &mut SimRng)`. v0.1 uses **static** skill: with
  `improvement_rate=0, volatility=0` `advance` is the identity (no-op), so the
  population is generated once at `t=0` and never changes.
- `population.rs` — `PopulationConfig { size, archetypes }` and
  `PopulationGenerator::generate(config, rng) -> (Vec<PlayerReality>,
  Vec<PlayerObservation>)`. Each player is drawn from its archetype's
  distribution; observation uses `initial_rating` if set else the sampled skill
  (`rating_deviation: 350.0`, `games_played: 0`, etc. per §5.8). The
  observation's `skill_vector`/`hidden_mmr` carry the **true skill** so the
  outcome model can decide matches from ground truth; only `rating` is the
  initial/visible ladder value. Proportions become integer counts via the
  **largest-remainder method** so they always sum exactly to `size`.

**`matchlab-game` is implemented with the outcome model:**
- `outcome.rs` — `OutcomeModel` trait (spec §6.1): `win_probability(team_a,
  team_b)` and `simulate(match_id, team_a, team_b, rng) -> MatchResult`. Takes
  `PlayerObservation` only — never `PlayerReality` (truth separation).
- `logistic.rs` — `LogisticOutcomeModel` (spec §6.2) with `beta`, `noise`, and
  inert `use_multidimensional`/`dimension_weights` fields (multidim research
  path is **out of scope** for v0.1). `effective_skill` uses
  `obs.skill_vector.overall()` — the **true skill** carried in the observation
  binding (falling back to `obs.rating` only for skill-vacuous observations) —
  so match outcomes are decided by ground truth and Elo genuinely learns from
  results. `win_probability` is the logistic of the average-team-skill
  difference; `simulate` adds noise, picks a winner, and builds a fully
  populated `MatchResult` (team ids, scores, per-player `PlayerPerformance`,
  duration, `variance`). Ticket 12 grounded outcomes this way; the previous
  flat-`rating` default closed the loop (outcomes driven by ratings, which
  update those ratings) and made "MAE decreases" structurally impossible.

**`matchlab-rating` is implemented with the rating systems:**
- `system.rs` — `RatingSystem` trait (spec §8.1): `information_budget()`,
  `initialize(player_id)`, `predict(team_a, team_b)`,
  `update(match_result, observations) -> HashMap<PlayerId, RatingState>`, plus
  `rating()`/`uncertainty()` conveniences. `RatingState { rating,
  rating_deviation, volatility, games_played }`, 11-variant `ObservationType`.
- `elo.rs` — `EloRatingSystem` (spec §8.4) with `EloConfig { k_factor,
  initial_rating, beta }` and `from_yaml`. `divisor = beta * ln(10)` keeps the
  log10 Elo scale consistent with the logistic game model (so both compute the
  same win probability for a given rating gap). `update` applies
  `k_factor * (actual − expected)` per team member. Elo/Flat only declare
  `WinLoss` in their information budget; a **matching-info budget** is enforced:
  loop handlers sanitize the `MatchResult` via `filter.rs` before `update`.
- `filter.rs` — `filter_match_result(&MatchResult, &[ObservationType]) ->
  FilteredMatchResult` (spec §8.2) with `into_match_result(&self, MatchId) ->
  MatchResult` producing a budget-sanitized `MatchResult` (scores,
  per-player performances, and durations zeroed/emptied for non-permitted
  data). `matchlab-loop` calls this in `handle_match_end`, so a WinLoss-only
  system never sees score/perf/duration leaks.
- `flat.rs` — `FlatPointsRatingSystem` (spec §8.3) with `FlatPointsConfig {
  win_points, loss_points, initial_rating }` and `from_yaml`; fixed ±points
  baseline.
- `plugins.rs` — `registry` module with `all_systems()` (`["elo",
  "flatpoints"]`) and `from_name(name, &serde_yaml::Value) ->
  Option<Box<dyn RatingSystem>>`. Glicko-2/TrueSkill are **not** registered in
  v0.1; unknown names return `None`.

**`matchlab-matchmaking` is implemented with the queue + batch matchmaker:**
- `queue.rs` — `QueueEntry` (player_id, joined_at, observation, region, party_id,
  game_mode, role, latency_ms) and `Queue` with `enqueue`, `remove`,
  `remove_batch`, `waiting_time` (saturating `now − joined_at` — the basis of the
  v0.1 queue-time metric), `entries`/`len`/`is_empty`, `from_entries`.
- `matchmaker.rs` — `Matchmaker` trait (spec §7.2)
  `find_matches(queue, world, team_size, now, rng) -> Vec<ProposedMatch>`;
  `ProposedMatch { team_a, team_b, quality_score }` with static `match_quality`
  = `1 − (|avg_a − avg_b| / 400).clamp(0,1)` computed from **observations** only.
- `constraint.rs` — `Constraint` trait (spec §7.3). No concrete constraints in
  v0.1; the batch matchmaker runs with an empty list.
- `batch.rs` — `BatchMatchmaker { interval_ticks, constraints }` (spec §7.8).
  **Rating-balanced** formation: sort candidates by `observation.rating`
  (ties by `joined_at`) and assign alternately to team A / team B in
  consecutive `2 × team_size` blocks. Adjacent-by-rating players land on
  opposite teams, so the two teams are balanced and `match_quality` stays
  ~0.96–0.98 (the naive FIFO pairing caps near 0.68 on the standard
  population, failing the quality exit criterion). The `interval_ticks`
  field is metadata the event handler uses to decide when to trigger
  matchmaking; the handler forms matches in consecutive blocks, emitting the
  final block when full (the spec's reference loop silently drops it).
  ExpandingWindow/Strict/HubSpoke are **out of scope**.

**`matchlab-loop` is implemented with the event-handler machine:**
- `machine.rs` — `LoopConfig { team_size, batch_interval_ticks, rejoin_delay,
  max_matches }` and `MachineState { population: HashMap<PlayerId,
  (PlayerReality, PlayerObservation)>, queue, active_matches: HashMap<MatchId,
  MatchResult>, matches_completed, matches_formed, pub metrics: MetricsEngine,
  rating_system, outcome_model, matchmaker }`. The `handle_*` functions are
  plain `(world, event, state) -> Vec<Box<dyn Event>>` pure-per-event
  transforms, so they are unit-testable without the engine.
  - `PlayerJoin` → add reality+observation to `World`, set `queue_joined_at`,
    schedule `PlayerQueue`.
  - `PlayerQueue` → enqueue the player (entry built from the live observation)
    and refresh `obs.queue_joined_at` to `world.time` (keeps the
    queue-time metric measuring the current join→formation wait, including
    re-queues after a match).
  - `MatchTimer` (new periodic event) → call `find_matches`, cap formation to
    the remaining `max_matches − matches_formed` budget (a formed match is an
    in-flight obligation, so over-capping on `matches_completed` would overshoot),
    emit one `MatchFormed` per proposal + re-schedule the next timer.
  - `MatchFormed` → simulate via the outcome model with `world.rng`,
    `metrics.record_match(&result, world)` (recorded at **formation** time —
    recording at MatchEnd made `queue_time` ≈ match duration, breaking the
    "queue time = actual wait" exit criterion), store the `MatchResult` in
    `active_matches` + `World.matches[InProgress]`, schedule `MatchEnd` at
    `now + duration`.
  - `MatchEnd` → `rating_system.update` on a **budget-sanitized** result
    (`filter_match_result` + `into_match_result`), apply returned
    `RatingState`s back to `World.observations` only (truth separation), mark
    the match `Completed`, increment `matches_completed`, and re-queue all
    participants after `rejoin_delay` while `matches_formed < max_matches`.
  Forming is capped by `matches_formed` (guarantees the loop terminates at
  exactly `max_matches` completed matches); `find_matches` is invoked with the
  world's rng temporarily swapped out because the matchmaker signature takes
  `&World` + `&mut SimRng`. `MachineState::new(rating, outcome, matchmaker,
  metrics, config)`; `matches_formed()` getter (field private).
- `lib.rs` — `MatchLoop { state: Arc<Mutex<MachineState>>, world, engine }`
  with `new(rating, outcome, matchmaker, metrics, config, seed)` registering
  the five handlers on the `EventEngine`, scheduling initial `PlayerJoin`s
  (sorted by `PlayerId.0` — `HashMap` iteration order is randomized via
  `RandomState`, so unsorted seeding would break determinism) plus an initial
  `MatchTimer`, `run()` that ticks to completion, `run_until(SimTime)`
  (uses `engine.peek_time()`), and `finalize_metrics()`. Initial `PlayerJoin`
  order + equal-time heap pops make the whole experiment deterministic for a
  given seed. Re-exports `LoopConfig`, `MachineState`, and the `handle_*` fns.

**`matchlab-metrics` is implemented with the collectors** (depends on
`matchlab-core` only; metrics are the sole legitimate reader of `PlayerReality`
besides the simulation):
- `collector.rs` — `MetricCollector` trait (spec §11.2): `name()`,
  `record_match(mr, world)`, `compute() -> MetricResult`, and an optional
  `time_buckets() -> Option<Vec<f64>>` (default `None`) that the engine folds
  into a `{name}_by_time` metric; `MetricResult` enum (`Scalar`,
  `Distribution`, `Summary { mean, median, p75, p90, p95, p99, stddev }`,
  `Histogram { buckets }`, `TimeSeries { bucket_means }`) with
  `serde::Serialize`.
- `engine.rs` — `MetricsEngine` (spec §11.1): `register`, `record_match`,
  `finalize()` (also inserts each collector's `{name}_by_time` `TimeSeries`
  when `time_buckets` is present), `results() -> &HashMap<String, MetricResult>`.
- `stats.rs` — `Summary { n, mean, median, p75, p90, p95, p99, stddev }`,
  `summary(&[f64])` (nearest-rank percentile by truncation per §14.1), and
  `summary_to_result(&[f64])` (empty sample → `Scalar(0.0)`). This is the
  canonical statistics implementation; `matchlab-analysis` re-exports it
  (`matchlab_analysis::stats`) and it lives here to keep collectors on the
  metrics-only-core boundary.
- `accuracy.rs` — `RatingAccuracyCollector` ("rating_accuracy"): MAE of
  `obs.rating` vs `reality.skill.overall()` over each match's **participants**,
  summarized (spec §11.3; participant-sampled rather than whole-population
  snapshots so memory/steps scale with matches, not matches × players). Each
  sample is time-stamped; `MetricCollector::time_buckets` (default
  `None`) yields a 20 equal-duration-bucket mean series that the engine folds
  into a `rating_accuracy_by_time` `TimeSeries` — the "MAE decreases over
  time" convergence evidence for the v0.1 acceptance ticket. Reads ground
  truth — allowed for metrics.
- `quality.rs` — `MatchQualityCollector` ("match_quality"):
  `1 − (|avg_a − avg_b|/400).clamp(0,1)` from observation ratings, summarized.
- `queue.rs` — `QueueTimeCollector` ("queue_time"): wait = `world.time
  .duration_since(obs.queue_joined_at)` per participant — real join→formation
  wait, **not** match duration (v0.1 exit condition).
No collectors besides these three are in scope for v0.1 (spec §11.3's
inequality/ndcg/correlation/convergence/etc. are out).

**`matchlab-experiments` is implemented with the config + runner:**
- `config.rs` — serde types for the full experiment manifest (spec §13.2):
  `ExperimentConfig`, `ExperimentSpec` (name, optional description, seed,
  population/game/matchmaking/rating/detection/ranking/metrics/objectives/
  cohorts/duration/output), `PopulationSpec`/`ArchetypeSpec`/`DistributionSpec`
  (normal/uniform/log_normal), `GameSpec`, `MatchmakingSpec`
  (algorithm + flattened params, max_queue_time), `RatingSpec { systems: Vec<RatingSystemSpec> }`
  with `RatingSystemSpec { name, params }` flatten, `DetectionSpec`,
  `SmurfDetectionSpec`, `RankingSpec`, `RankBracketSpec`, `ObjectiveWeightsSpec`,
  `CohortSpec`/`CohortFilterSpec` (tagged enum), `DurationSpec { until_secs | max_matches | max_time }`,
  `OutputSpec { directory, format }`. `cohorts` is a required `Vec`
  (manifests use `cohorts: []` when unused).
- `inherit.rs` — YAML-level config inheritance: `load(path)` /
  `resolve_str(text, base_dir)` / `load_value`, with `base: <path>` keys
  resolved recursively and `deep_merge` (mappings merge recursively; scalars
  and sequences are replaced by the child). Enables spec §13.1's controlled
  one-variable-differs experiments; the `experiments/base/` directory holds
  inherited base configs.
- `seed.rs` — `SeedManager` (spec §13.7): separate seeds for
  population/game/arrival/behavior derived from the one experiment seed via
  `derive(name, parent_seed)`, plus `hash_config(&ExperimentConfig)` (a
  `#[derive(Default)]` `DefaultHasher` over length-prefixed serialized
  fields) and `git_commit_hash()` for `ExperimentResult`.
- `runner.rs` — `ExperimentRunner::run(&ExperimentConfig) ->
  Result<ExperimentResult, String>`: generates the population, builds the
  rating system via `matchlab_rating::registry::from_name` (params flattened
  to a `serde_yaml::Value::Mapping`), builds the logistic outcome model +
  batch matchmaker (`batch_interval` in YAML is seconds →
  `LoopConfig.batch_interval_ticks`), registers the named metric collectors
  (errors on unknown names), and runs `MatchLoop` to the `DurationSpec`
  bound. v0.1 uses only the **first** `rating.systems` entry. Returns
  `ExperimentResult { experiment_id = "{name}-{config_hash}", name,
  config_hash, git_commit, timestamp, matches_completed, matches_formed,
  simulated_time_secs, metrics }`; the timestamp is a hand-rolled ISO-8601 UTC
  string (no chrono dep). `metrics` is a `BTreeMap` (not `HashMap`) so JSON
  serialization key order is deterministic across processes. Each registered
  collector's `time_buckets()` (if present) is folded into `{name}_by_time`
  `TimeSeries` by the engine. 6 unit tests include a same-seed determinism
  check (identical metrics) and a sim-time bound check. `lib.rs` re-exports
  the public API.
- Binary `src/main.rs` — `matchlab run <manifest.yaml>` (exit 0/1/2): loads via
  `inherit::load`, runs, and delegates output to `matchlab-analysis` — writes
  the result as pretty JSON to `<output.directory>/<experiment.name>.json`
  (`export::write_result_json`) and, when `output.report: true`, a Markdown
  report to `<name>.md` (`report::generate_report`) with config hash, git
  commit, and the metrics table.
- `experiments/v0_1_basic.yaml` — the spec §17 minimal v0.1 manifest (10,000
  players, team size 5, cold ladder start with `initial_rating: 1000`, flat
  skill, no detection/ranking/objective/cohorts, capped by `max_time: 604800`).
  The `initial_rating` deviates from the literal §17 snippet to provide a
  meaningful convergence scenario: visible ratings start at 1000 while true
  skill is sampled from N(1000, 250), so Elo has something to learn.

**`matchlab-analysis` is implemented with the reporting/export layer:**
- `stats.rs` — re-exports `matchlab_metrics::stats` as the `summary`/
  `Summary`/`summary_to_result` API (spec §14.1). The canonical implementation
  stays in `matchlab-metrics` to keep the metrics-only-core boundary.
- `report.rs` — spec §14.4: `ReportConfig { include_plots, include_raw_data,
  format }` and `ReportFormat { Json, Markdown }` (HTML out of scope, despite
  the spec's enum). `generate_report(&ExperimentResult) -> String` (single
  Markdown report) and `generate_comparison_report(&[ExperimentResult],
  &ReportConfig) -> String` (Markdown or JSON). Markdown includes name, config
  hash, git commit, matches completed, simulated time, and a metric table.
- `export.rs` — spec §14.5: `ExportedMatch`/`ExportedObservation`
  (serde `Serialize` + `Deserialize`), `ExportFormat` (implements
  `FromStr`; parquet parses but `write()` returns `ErrorKind::Unsupported`
  for v0.1), and `RawDataExporter` accumulating per-match + per-observation
  traces (`record_observations` sorts by `PlayerId` for deterministic JSON)
  writing `matches.json`/`observations.json`. `write_result_json(&result,
  &dir)` writes the metrics JSON under `OutputSpec.directory`.
  `RawDataExporter` is a standalone utility in v0.1 — per-match loop wiring is
  left to a later ticket.
- Determined output: `ExperimentResult.metrics` is a `BTreeMap` and the
  exporter sorts observations, so two runs with identical seed produce
  byte-identical files (the wall-clock `timestamp` field is the only thing
  that legitimately differs).

**`matchlab-ranking` is implemented with the rank mapper + leaderboard:**
- `ranker.rs` — `RankMapper` trait (spec §10.1): `rating_to_rank(rating) ->
  Rank` and `rank_to_rating_range(rank) -> (f64, f64)`, `Rank { tier,
  division }` (serde `Deserialize`), `BracketRankMapper { brackets: Vec<RankBracket> }`
  and `RankBracket { rank, min, max }`. `rating_to_rank` finds the first
  bracket where `min <= rating < max`; ratings outside all brackets clamp to
  the **last** bracket (the spec's reference behavior, not the first).
  `rank_to_rating_range` returns `(0.0, 0.0)` for unknown ranks.
- `leaderboard.rs` — `Leaderboard` (spec §10.2) with `update(player_id,
  rating, rank, games_played)` (insert-or-replace, then re-sort by rating
  descending), `rank_of(player_id) -> Option<usize>`, `top_n(n) -> &[LeaderboardEntry]`
  (clamps when `n > len`), `entries()`/`len()`/`is_empty()`. `LeaderboardEntry {
  player_id, rating, rank, games_played }`.
- `lib.rs` — re-exports `Leaderboard`, `LeaderboardEntry`, `BracketRankMapper`,
  `Rank`, `RankBracket`, `RankMapper`.

**`matchlab-objective` is implemented with the utility scoring:**
- `utility.rs` — `ObjectiveWeights` (serde `Deserialize`: `match_quality`,
  `queue_time`, `rating_accuracy`, `convergence_speed`, `smurf_damage`,
  `false_positive_rate`, `streak_frustration`; `Default` matches §12.1) and
  `ObjectiveFunction::evaluate(&HashMap<String, MetricResult>) -> (f64, &HashMap)`
  (spec §12.1). Higher-is-better metrics (match quality) add weighted mean;
  lower-is-better metrics (queue time, rating error, convergence games) subtract;
  `smurf`/`streaks` `Distribution` values are read by index. The raw metrics map
  is returned by reference and never discarded (§12.2 — the "never discard raw
  metrics" rule). `lib.rs` re-exports `ObjectiveFunction`, `ObjectiveWeights`.

The twelve-step build order is complete and v0.1 is accepted.

**Build the project following the v0.1 build order in `docs/spec.md` (section 17).** Steps 1–12 are complete.

---

## v0.1 Build Order (Summary)

1. **Workspace + Core Types** — `matchlab-core`: SimTime, PlayerId, MatchId, SimRng, SkillVector, PlayerReality, PlayerObservation, MatchResult
2. **Event Engine** — EventEngine, Event trait, EventKind, World, Simulation
3. **Player Population** — `matchlab-players`: PopulationGenerator, SkillProcess (static)
4. **Game Outcome** — `matchlab-game`: OutcomeModel trait, LogisticOutcomeModel
5. **Elo Rating** — `matchlab-rating`: RatingSystem trait, Elo, FlatPoints
6. **Queue + Matchmaker** — `matchlab-matchmaking`: Queue, BatchMatchmaker
7. **Event Handlers** — Wire everything: PlayerJoin → Queue → Match → RatingUpdate
8. **Metrics** — `matchlab-metrics`: RatingAccuracy, MatchQuality, QueueTime collectors
9. **Config + Runner** — `matchlab-experiments`: YAML parsing, config inheritance, ExperimentRunner, CLI
10. **Analysis + Output** — `matchlab-analysis`: summary stats, JSON export
11. **Acceptance** — `cargo run -- run experiments/v0_1_basic.yaml` produces metrics JSON, Elo MAE decreases (197.5 → 159.5), match quality mean 0.98, queue time 5.02s, all tests pass, deterministic.

---

## Running

```bash
cargo build
cargo test
cargo run -- run experiments/v0_1_basic.yaml
```

---

## Config Format

Experiment manifests are YAML, parsed via serde. Configs support **inheritance** — an experiment can declare `base: experiments/base/standard.yaml` and deep-merge overrides, enabling controlled comparisons where only one variable changes.

The minimal v0.1 manifest is at `docs/spec.md` section 17 ("v0.1 Minimal Experiment Manifest").

---

## Conventions

- **Rust edition:** 2024
- **Shared deps** (workspace): `serde` (with derive), `serde_yaml 0.9`, `rand 0.8`, `rand_chacha 0.3`, `mlua 0.10` (lua54, vendored)
- **No comments in code** unless explicitly requested
- **Unit tests** live in `#[cfg(test)] mod tests` blocks within each source file
- **Crate naming:** `matchlab-{domain}` (e.g., `matchlab-core`, `matchlab-rating`)
- **File naming:** `match_.rs` (not `match.rs`, which is a Rust keyword)
- **Plugin model:** Lua scripts under `plugins/` override specific decision points ("hooks") at runtime via `mlua`. Native Rust implementations remain the default. Scripts use `lua:<trait>` prefix in YAML (e.g., `lua:elo`). Missing hooks fall back to Rust defaults. See `docs/spec.md` §3.3 for hook signatures and design rules.
- **Lua scripts are pure.** No `math.random` — all randomness comes from `SimRng`. Scripts receive only observable data, never `PlayerReality`.

---

## Where to Find Things

| Need | Look at |
|------|---------|
| Core types (PlayerReality, World, etc.) | `crates/matchlab-core/src/` |
| How events flow | `crates/matchlab-core/src/event.rs`, `src/lib.rs` (Simulation) |
| Rating algorithm implementations | `crates/matchlab-rating/src/` |
| Matchmaker implementations | `crates/matchlab-matchmaking/src/` |
| Metric collector implementations | `crates/matchlab-metrics/src/` |
| Lua hook scripts | `plugins/` (organized by trait type) |
| Hook definitions + loader | `crates/matchlab-{trait}/src/hooks.rs`, `loader.rs` |
| How to add a Lua hook | Write a `.lua` file in `plugins/`, define hook functions, reference via `lua:<name>` in YAML |
| Experiment YAML schema | `docs/spec.md` section 13.1 |
| Build plan | `tickets/` directory |
| Adversarial agents (booster, deranker, etc.) | `crates/matchlab-adversarial/src/` |
| Player satisfaction model | `crates/matchlab-utility/src/satisfaction.rs` |
