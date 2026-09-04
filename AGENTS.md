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
    ├── matchlab-game
    ├── matchlab-matchmaking
    ├── matchlab-rating
    ├── matchlab-detection
    ├── matchlab-ranking
    ├── matchlab-metrics       (depends on core only)
    ├── matchlab-objective     (depends on core + metrics)
    ├── matchlab-adversarial   (depends on core)
    └── matchlab-utility       (depends on core)

matchlab-loop          (depends on core + players + game + rating + matchmaking)
matchlab-experiments   (depends on all above)
matchlab-analysis      (depends on core + metrics + objective)
matchlab (binary)      (depends on experiments + analysis)
```

---

## Design Principles

### 1. Truth Separation (Critical)

The simulation maintains two parallel representations of every player:

- **`PlayerReality`** — ground truth the simulation knows but algorithms never see.
- **`PlayerObservation`** — what rating/matchmaking/detection systems see, derived only from permitted data.

**No algorithm, matchmaker, or detection system may call `world.players[pid]` directly.** They must use `world.observations[pid]`. Violating this corrupts the entire experiment.

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

The workspace foundation (v0.1 **Ticket 01**), the core types (v0.1
**Ticket 02**), the event engine + World (v0.1 **Ticket 03**), the player
population (v0.1 **Ticket 04**), the game outcome model (v0.1 **Ticket 05**),
the rating systems Elo + FlatPoints (v0.1 **Ticket 06**), the queue +
batch matchmaker (v0.1 **Ticket 07**), the event-handler loop (v0.1
**Ticket 08**), and the metric collectors (v0.1 **Ticket 09**) are complete.
The root `Cargo.toml` is a Cargo workspace with the nine v0.1 crates declared as members:

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
└── matchlab-analysis/
```

- `[workspace.dependencies]` declares `serde` (derive), `serde_yaml 0.9`,
  `rand 0.8`, `rand_chacha 0.3`; `[workspace.package]` sets `edition = "2024"`.
- `src/main.rs` is the `match-lab` binary with a `matchlab run <manifest>`
  CLI skeleton (prints "not yet implemented" until Ticket 10); it depends on
  `matchlab-experiments` and `matchlab-analysis`.
- `experiments/base/` exists (empty, for inherited base configs).
- `.github/workflows/ci.yml` runs build + test + check + clippy + fmt.
- `/results` is gitignored.
- `cargo build --workspace`, `cargo test --workspace`, and
  `cargo check --workspace` all pass.

**`matchlab-core` is implemented with the v0.1 core types:**
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
  (`downcast_ref`) after matching on `kind()` — this is how Ticket 08 handlers
  read `player_id`/`match_id`/teams. `EventEngine` (register_handler/schedule/
  next_event/peek_time/is_empty/tick).
- `world.rs` — `World` holding `players`, `observations`, `matches`, `rng`,
  `time`, with private monotonic ID counters (`next_player_id()`/`next_match_id()`).
  Truth separation: `player.rs`/`world.rs` enforce the rule that algorithms
  access players via `observe()`/`observations`, never `players`/`reality()`.
- `simulation.rs` — `Simulation { world, engine }` with `new`, and
  `run(until)` / `run_to_completion()` (skips idle clock periods).

**`matchlab-players` is implemented with the v0.1 population logic:**
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
  (`rating_deviation: 350.0`, `games_played: 0`, etc. per §5.8, with the
  observation's `skill_vector`/`hidden_mmr` derived from the visible rating).
  Proportions become integer counts via the **largest-remainder method** so
  they always sum exactly to `size`.

**`matchlab-game` is implemented with the v0.1 outcome model:**
- `outcome.rs` — `OutcomeModel` trait (spec §6.1): `win_probability(team_a,
  team_b)` and `simulate(match_id, team_a, team_b, rng) -> MatchResult`. Takes
  `PlayerObservation` only — never `PlayerReality` (truth separation).
- `logistic.rs` — `LogisticOutcomeModel` (spec §6.2) with `beta`, `noise`, and
  inert `use_multidimensional`/`dimension_weights` fields (multidim research
  path is **out of scope** for v0.1; `effective_skill` defaults to the flat
  `obs.rating`). `win_probability` is the logistic of the average-team-skill
  difference; `simulate` adds noise, picks a winner, and builds a fully
  populated `MatchResult` (team ids, scores, per-player `PlayerPerformance`,
  duration, `variance`).

**`matchlab-rating` is implemented with the v0.1 rating systems:**
- `system.rs` — `RatingSystem` trait (spec §8.1): `information_budget()`,
  `initialize(player_id)`, `predict(team_a, team_b)`,
  `update(match_result, observations) -> HashMap<PlayerId, RatingState>`, plus
  `rating()`/`uncertainty()` conveniences. `RatingState { rating,
  rating_deviation, volatility, games_played }`, 11-variant `ObservationType`.
- `elo.rs` — `EloRatingSystem` (spec §8.4) with `EloConfig { k_factor,
  initial_rating, beta }` and `from_yaml`. `divisor = beta * ln(10)` keeps the
  log10 Elo scale consistent with the logistic game model (so both compute the
  same win probability for a given rating gap). `update` applies
  `k_factor * (actual − expected)` per team member. **Information-budget
  enforcement (`filter.rs`) is deferred to Ticket 10** — Elo/Flat only read
  `WinLoss` data, declared correctly.
- `flat.rs` — `FlatPointsRatingSystem` (spec §8.3) with `FlatPointsConfig {
  win_points, loss_points, initial_rating }` and `from_yaml`; fixed ±points
  baseline.
- `plugins.rs` — `registry` module with `all_systems()` (`["elo",
  "flatpoints"]`) and `from_name(name, &serde_yaml::Value) ->
  Option<Box<dyn RatingSystem>>`. Glicko-2/TrueSkill are **not** registered in
  v0.1; unknown names return `None`.

**`matchlab-matchmaking` is implemented with the v0.1 queue + batch matchmaker:**
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
  FIFO-candidate greedy formation: sort by `joined_at`, fill team A then team B,
  emit matches in consecutive `2 × team_size` blocks (the spec's reference loop
  silently drops a full final block — this implementation emits it). The
  `interval_ticks` field is metadata the Ticket 08 handler uses to decide when
  to trigger matchmaking. ExpandingWindow/Strict/HubSpoke are **out of scope**.

**`matchlab-loop` is implemented with the v0.1 event-handler machine:**
- `machine.rs` — `LoopConfig { team_size, batch_interval_ticks, rejoin_delay,
  max_matches }` and `MachineState { population: HashMap<PlayerId,
  (PlayerReality, PlayerObservation)>, queue, active_matches: HashMap<MatchId,
  MatchResult>, matches_completed, matches_formed }` plus the boxed rating
  system, outcome model, and matchmaker. The `handle_*` functions are plain
  `(world, event, state) -> Vec<Box<dyn Event>>` pure-per-event transforms, so
  they are unit-testable without the engine:
  - `PlayerJoin` → add reality+observation to `World`, set `queue_joined_at`,
    schedule `PlayerQueue`.
  - `PlayerQueue` → enqueue the player (entry built from the live observation)
    and refresh `obs.queue_joined_at` to `world.time` (keeps the v0.1
    queue-time metric measuring the current join→formation wait, including
    re-queues after a match).
  - `MatchTimer` (new periodic event) → call `find_matches`, cap formation to
    the remaining `max_matches − matches_formed` budget (a formed match is an
    in-flight obligation, so over-capping on `matches_completed` would overshoot),
    emit one `MatchFormed` per proposal + re-schedule the next timer.
  - `MatchFormed` → simulate via the outcome model with `world.rng`, store the
    `MatchResult` in `active_matches` + `World.matches[InProgress]`, schedule
    `MatchEnd` at `now + duration`.
  - `MatchEnd` → `rating_system.update`, apply returned `RatingState`s back to
    `World.observations` only (truth separation), mark the match `Completed`,
    increment `matches_completed`, and re-queue all participants after
    `rejoin_delay` while `matches_formed < max_matches`.
  Forming is capped by `matches_formed` (guarantees the loop terminates at
  exactly `max_matches` completed matches); `find_matches` is invoked with the
  world's rng temporarily swapped out because the matchmaker signature takes
  `&World` + `&mut SimRng`.
- `lib.rs` — `MatchLoop { state: Arc<Mutex<MachineState>>, world, engine }`
  with `new(...)` that registers the five handlers on the `EventEngine`,
  schedules initial `PlayerJoin`s (sorted by `PlayerId.0` — `HashMap` iteration
  order is randomized via `RandomState`, so unsorted seeding would break
  determinism) plus an initial `MatchTimer`, and `run()` that ticks to
  completion. Initial `PlayerJoin` order + equal-time heap pops make the whole
  experiment deterministic for a given seed.

**`matchlab-metrics` is implemented with the v0.1 collectors** (depends on
`matchlab-core` only; metrics are the sole legitimate reader of `PlayerReality`
besides the simulation):
- `collector.rs` — `MetricCollector` trait (spec §11.2): `name()`,
  `record_match(mr, world)`, `compute() -> MetricResult`; `MetricResult` enum
  (`Scalar`, `Distribution`, `Summary { mean, median, p75, p90, p95, p99,
  stddev }`, `Histogram { buckets }`) with `serde::Serialize`.
- `engine.rs` — `MetricsEngine` (spec §11.1): `register`, `record_match`,
  `finalize()`, `results() -> &HashMap<String, MetricResult>`.
- `stats.rs` — `Summary { n, mean, median, p75, p90, p95, p99, stddev }`,
  `summary(&[f64])` (nearest-rank percentile by truncation per §14.1), and
  `summary_to_result(&[f64])` (empty sample → `Scalar(0.0)`). Duplicated from
  `matchlab-analysis` on purpose to keep the dependency boundary metrics-only-core.
- `accuracy.rs` — `RatingAccuracyCollector` ("rating_accuracy"): MAE of
  `obs.rating` vs `reality.skill.overall()`, summarized (spec §11.3). Reads
  ground truth — allowed for metrics, confirming the "MAE decreases" exit criterion.
- `quality.rs` — `MatchQualityCollector` ("match_quality"):
  `1 − (|avg_a − avg_b|/400).clamp(0,1)` from observation ratings, summarized.
- `queue.rs` — `QueueTimeCollector` ("queue_time"): wait = `world.time
  .duration_since(obs.queue_joined_at)` per participant — real join→formation
  wait, **not** match duration (v0.1 exit condition).
No collectors besides these three are in scope for v0.1 (spec §11.3's
inequality/ndcg/correlation/convergence/etc. are out).

The other two crates are still stubs; no algorithms are implemented yet.
Individually-consistent tickets from `tickets/` drive the remaining v0.1 build
order (next: Ticket 10, Config + Runner).

**Build the project following the v0.1 build order in `docs/spec.md` (section 17).** Steps 1-10 are listed there with specific deliverables and exit criteria.

---

## v0.1 Build Order (Summary)

1. **Workspace + Core Types** — `matchlab-core`: SimTime, PlayerId, MatchId, SimRng, SkillVector, PlayerReality, PlayerObservation, MatchResult
2. **Event Engine** — EventEngine, Event trait, EventKind, World, Simulation
3. **Player Population** — `matchlab-players`: PopulationGenerator, SkillProcess (static in v0.1)
4. **Game Outcome** — `matchlab-game`: OutcomeModel trait, LogisticOutcomeModel
5. **Elo Rating** — `matchlab-rating`: RatingSystem trait, Elo, FlatPoints
6. **Queue + Matchmaker** — `matchlab-matchmaking`: Queue, BatchMatchmaker
7. **Event Handlers** — Wire everything: PlayerJoin → Queue → Match → RatingUpdate
8. **Metrics** — `matchlab-metrics`: RatingAccuracy, MatchQuality, QueueTime collectors
9. **Config + Runner** — `matchlab-experiments`: YAML parsing, config inheritance, ExperimentRunner, CLI
10. **Analysis + Output** — `matchlab-analysis`: summary stats, JSON export

**v0.1 exit criteria:**
- `cargo run -- run experiments/v0_1_basic.yaml` completes
- Produces `results/` with metrics JSON
- Elo ratings converge (MAE decreases over time)
- Match quality mean > 0.85
- Queue time measures actual wait (not match duration)
- All `cargo test` pass
- Same seed → identical results across runs

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
- **Shared deps** (workspace): `serde` (with derive), `serde_yaml 0.9`, `rand 0.8`, `rand_chacha 0.3`
- **No comments in code** unless explicitly requested
- **Unit tests** live in `#[cfg(test)] mod tests` blocks within each source file
- **Crate naming:** `matchlab-{domain}` (e.g., `matchlab-core`, `matchlab-rating`)
- **File naming:** `match_.rs` (not `match.rs`, which is a Rust keyword)
- **Plugin registration:** new rating systems, matchmakers, etc. are added by editing the appropriate crate's source and adding a `from_name` arm to the registry — not runtime dynamic loading

---

## Where to Find Things

| Need | Look at |
|------|---------|
| Core types (PlayerReality, World, etc.) | `crates/matchlab-core/src/` |
| How events flow | `crates/matchlab-core/src/event.rs`, `src/lib.rs` (Simulation) |
| Rating algorithm implementations | `crates/matchlab-rating/src/` |
| Matchmaker implementations | `crates/matchlab-matchmaking/src/` |
| Metric collector implementations | `crates/matchlab-metrics/src/` |
| How to add a new rating system | Follow Elo in `crates/matchlab-rating/src/elo.rs`, register in `plugins/mod.rs` |
| Experiment YAML schema | `docs/spec.md` section 13.1 |
| v0.1 build plan | `docs/spec.md` section 17 |
| Adversarial agents (booster, deranker, etc.) | `crates/matchlab-adversarial/src/` |
| Player satisfaction model | `crates/matchlab-utility/src/satisfaction.rs` |
