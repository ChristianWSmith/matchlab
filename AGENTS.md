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
    ├── matchlab-lua/           # Lua-native system foundation (VM, context, rng, validation)
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
    ├── matchlab-lua            (+ mlua)
    ├── matchlab-players
    ├── matchlab-game           (+ mlua via matchlab-lua)
    ├── matchlab-matchmaking    (+ mlua via matchlab-lua)
    ├── matchlab-rating         (+ mlua via matchlab-lua)
    ├── matchlab-detection      (+ mlua via matchlab-lua)
    ├── matchlab-ranking
    ├── matchlab-metrics        (depends on core + matchlab-lua)
    ├── matchlab-objective      (depends on core + metrics)
    ├── matchlab-adversarial    (depends on core + matchlab-lua)
    └── matchlab-utility        (depends on core + matchlab-lua)

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

The workspace is fully implemented: 15 crates under `crates/`, a binary at `src/main.rs`. The root `Cargo.toml` is a Cargo workspace:

```
crates/
├── matchlab-core/
├── matchlab-lua/
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

**`matchlab-lua` is implemented with the Lua-native system foundation:**
- `vm.rs` — `LuaVm` (a `Mutex<Lua>` wrapper): `load(path, params, required)`
  resolves + validates + execs a script, stores its `config` (from YAML params),
  registers the `matchlab.rng_*` helpers, and provides
  `call_with_context(name, args, context)` (args + `config` + `context` are
  pushed in that order; the script may return `(value, context)` or mutate the
  passed context in place) and `get_global` (reads `information_budget`,
  `name`, `time_buckets` globals). `with_rng(&mut SimRng, f)` makes the RNG
  available to `matchlab.rng_*`.
- `context.rs` — `Context` is an ordered `serde_yaml::Value` (defaults to an
  empty mapping): arbitrary script-defined state persisted on the Rust model and
  threaded through every call. `yaml_to_lua`/`lua_to_yaml` round-trip it (a Lua
  table whose keys are exactly `1..=n` becomes a sequence).
- `rng.rs` — deterministic randomness routing: a thread-local `*mut SimRng` slot
  set/cleared around every guarded call; `matchlab.rng_range`/`rng_bool`/
  `rng_normal`/`rng_u64` draw from it. Scripts must never call `math.random`.
- `convert.rs` — core↔Lua marshalling: `observation_to_table` (with `include_skill`
  to control the ground-truth skill binding), `participant_to_table` (metrics
  only — adds `true_skill`/`improvement_rate`/`reality_games_played`),
  `match_result_to_table`, `metric_snapshot`, `region_str`/`team_str`.
- `validate.rs` — `validate_script(path, required)`: parse/exec + required
  function presence + the `math.random` source ban.
- `resolve.rs` — `resolve_script_path`/`workspace_root`: resolves `plugins/...`
  paths from the workspace root (walk up to the `[workspace]` Cargo.toml), so
  crate tests and the CLI both find scripts.
- The algorithm crates (`rating`, `game`, `matchmaking`, `detection`, `metrics`,
  `adversarial`, `utility`, `ranking`) depend on `matchlab-lua`; their Lua
  adapters live in each crate's `lua.rs`.

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
  13 concrete events (PlayerJoin/Leave/Queue/Quit/Return/Disconnect, MatchFormed,
  MatchStart, MatchEnd, RatingUpdate, DetectionCheck, SkillChange, MatchTimer),
  plus a checked `downcast::<T>()` helper.
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

**`matchlab-game` is implemented with the Lua-native outcome models:**
- `outcome.rs` — `OutcomeModel` trait (spec §6.1): `win_probability(team_a,
  team_b)` and `simulate(match_id, team_a, team_b, rng) -> MatchResult`. Takes
  `PlayerObservation` only — never `PlayerReality` (truth separation).
- `lua.rs` — `LuaOutcomeModel`: implements `OutcomeModel` by delegating to a
  script's `win_probability`/`simulate` functions; `simulate` runs inside
  `with_rng` so scripts draw deterministically via `matchlab.rng_*`. Observation
  tables carry `skill_overall`/`skill_vector` (`include_skill`), so match
  winners are decided by ground truth and Elo genuinely learns from results.
- The variants ship as Lua scripts under `plugins/game/`:
  - `logistic.lua` — spec §6.2. `effective_skill` = `skill_overall` (falling
    back to `rating` only for skill-vacuous observations); `win_probability` is
    the logistic of the average-team-skill difference; `simulate` adds noise,
    picks a winner, and builds a fully populated `MatchResult` (team ids,
    scores, per-player performances, duration, `variance`). Draw order mirrors
    the reference so results are byte-identical for a seed.
  - `variance.lua` — spec §6.3; logistic with a `variance_multiplier`-scaled
    noise envelope → more upsets at a given skill gap.
  - `composition.lua` — spec §6.3; effective skill is each player's
    `skill_vector` weighted by `config.dimension_weights`; team totals add a
    `synergy_bonus` per player. The multidim research model — can a 1D rating
    represent multidimensional skill?
  - `performance.lua` — spec §6.3; `recent_performances` mean (scaled by
    `performance_weight × beta`) shifts effective skill, so hot/cold streaks
    tilt win probability.
  - `fatigue.lua` — spec §6.3; decays each player's skill by
    `1 − decay_rate × games_played` (games played is the observable
    session-length proxy) before the logistic math.
  - `momentum.lua` — spec §6.3; scales each player's skill by
    `1 + momentum_factor × (win_rate − 0.5)` (streak proxy) before the logistic
    math.
- Ticket 12 grounded outcomes this way; the Lua ports reproduce the Rust
  results byte-for-byte (v0_1_basic acceptance numbers unchanged through the
  all-Lua game path).

**`matchlab-rating` is implemented with the Lua-native rating systems:**
- `system.rs` — `RatingSystem` trait (spec §8.1): `information_budget()`,
  `initialize(player_id)`, `predict(team_a, team_b)`,
  `update(match_result, observations) -> HashMap<PlayerId, RatingState>`, plus
  `rating()`/`uncertainty()` conveniences. `RatingState { rating,
  rating_deviation, volatility, games_played }`, 11-variant `ObservationType`.
- `lua.rs` — `LuaRatingSystem`: implements `RatingSystem` by delegating to a
  script's `initialize`/`predict`/`update` functions; reads the script's
  `information_budget` global at load; threads a `Context` through every call.
- `plugins.rs` — `registry` with `known_systems()` (name → script map: elo →
  `plugins/rating/elo.lua`, flatpoints, glicko2, trueskill), `from_script(path,
  params)`, and `from_name(name, params)`.
- `filter.rs` — `filter_match_result(&MatchResult, &[ObservationType]) ->
  FilteredMatchResult` (spec §8.2) with `into_match_result(&self, MatchId) ->
  MatchResult` producing a budget-sanitized `MatchResult` (scores,
  per-player performances, and durations zeroed/emptied for non-permitted
  data). `matchlab-loop` calls this in `handle_match_end`, so a WinLoss-only
  system never sees score/perf/duration leaks.
- The classic systems ship as Lua scripts under `plugins/rating/`:
  - `elo.lua` — spec §8.4. `divisor = beta * ln(10)` keeps the log10 Elo scale
    consistent with the logistic game model (both compute the same win
    probability for a rating gap). `update` applies `k_factor * (actual −
    expected)` per team member. Declares `information_budget = { "WinLoss" }`.
  - `flat.lua` — spec §8.3; fixed ±points baseline.
  - `glicko2.lua` — spec §8.5; full 6-step Glicko-2: scale to (μ, φ, σ) →
    `g`/`E` per opponent → `v`, `Δ` → Newton-Raphson volatility iteration →
    `φ*` → `φ'`, `μ'` → scale back. Verified against Glickman's paper worked
    example (r'=1464.06, RD'=151.52, σ'=0.05999).
  - `trueskill.lua` — spec §8.6; each player is N(μ, σ²); team performance =
    sum of member performances; truncated-Gaussian conditioning with the
    inverse-Mills-ratio factors `v,w` and draw margin from `draw_probability`.
    `initial_variance` is stored as `rating_deviation` = σ (sqrt of variance).
- Elo/Flat only declare `WinLoss` in their information budget; a **matching-info
  budget** is enforced: loop handlers sanitize the `MatchResult` via
  `filter.rs` before `update`.
- The Lua ports reproduce the Rust results byte-for-byte (v0_1_basic acceptance
  numbers unchanged through the all-Lua rating path).

**`matchlab-detection` is implemented with the Lua-native detection systems:**
- `detector.rs` — `DetectionSystem` trait (spec §9.1): `observe(&mut self,
  match_result, world)`, `evaluate(&self, player_id, world) -> DetectionResult`,
  `recommend_action(&self, result) -> InterventionAction`.
- `intervention.rs` — the `InterventionAction` enum (the escalation policy
  logic lives in the Lua script).
- `lua.rs` — `LuaDetectionSystem`: implements `DetectionSystem` by delegating
  to a script's `observe`/`evaluate`/`recommend_action`; threads a `Context`
  (per-player evidence) through every call; maps action strings to
  `InterventionAction`.
- `plugins/detection/smurf.lua` — the smurf detector: per-player state in
  `context`, expected performance scales with visible rating, actual from
  `impact + kills/10`; consecutive anomalies past `min_anomalous_games` ramp
  the anomaly probability; `recommend_action` walks the threshold ladder
  (`config.ladder`, default 0.3 None … 0.99 Ban) with escalation
  (`escalation_factor` per prior intervention) and a `min_games_before_action`
  gate. Smurf status is inferred from behavior — never a boolean flag.

**`matchlab-matchmaking` is implemented with the queue + Lua matchmakers:**
- `queue.rs` — `QueueEntry` (player_id, joined_at, observation, region, party_id,
  game_mode, role, latency_ms) and `Queue` with `enqueue`, `remove`,
  `remove_batch`, `waiting_time` (saturating `now − joined_at` — the basis of the
  v0.1 queue-time metric), `entries`/`len`/`is_empty`, `from_entries`.
- `matchmaker.rs` — `Matchmaker` trait (spec §7.2)
  `find_matches(queue, world, team_size, now, rng) -> Vec<ProposedMatch>`;
  `ProposedMatch { team_a, team_b, quality_score }` with static `match_quality`
  = `1 − (|avg_a − avg_b| / 400).clamp(0,1)` computed from **observations** only.
- `constraint.rs` — `Constraint` trait (spec §7.3). No concrete constraints in
  v0.1; the matchmakers run with an empty list.
- `lua.rs` — `LuaMatchmaker`: implements `Matchmaker` by delegating to a
  script's `find_matches` function. The queue is snapshotted to a Lua array
  (player_id, rating, rating_deviation, games_played, win_rate, `idx`,
  `joined_at_secs`, `wait_secs`, region, party_id, latency_ms, game_mode) —
  observations only, never `PlayerReality`. Scripts set `quality_score` or the
  adapter falls back to `ProposedMatch::match_quality`.
- The matchmakers ship as Lua scripts under `plugins/matchmaking/`:
  - `batch.lua` — spec §7.8. **Rating-balanced** formation: sort candidates by
    `rating` (ties by `joined_at_secs`, then `idx` — Lua `table.sort` is
    unstable, so the index tie-break preserves the Rust stable-sort behavior
    and keeps results byte-identical) and assign alternately to team A / team B
    in consecutive `2 × team_size` blocks. Adjacent-by-rating players land on
    opposite teams, so the two teams are balanced and `match_quality` stays
    ~0.96–0.98 (the naive FIFO pairing caps near 0.68 on the standard
    population, failing the quality exit criterion).
  - `expanding_window.lua` — spec §7.6 with stepped tiers
    `[(max_secs, allowed_diff)]` (default 5s→25, 10s→50, 20s→100, 30s→200,
    fallback `max_window: 400`) — skills matched within a window that widens
    with queue wait.
  - `strict.lua` — spec §7.7: only matches players within a fixed skill diff;
    outliers may wait indefinitely (intended "strict" behavior).
  - `hub_spoke.lua` — spec §7.9: partitions the queue by region (sorted region
    keys for determinism); under-capacity regions use an inlined regional
    greedy (no nested matchmakers in Lua), overflow regions fall to the hub
    path (longest-waiting first).
- `objective.rs` — `MatchObjective { weight_quality, weight_queue_time,
  weight_ping, weight_rating_uncertainty }` (spec §7.4) with
  `score(proposed, queue_entries, world) = w_q·Q − w_t·T − w_p·P − w_r·R`
  where `Q` is balance quality, `T` is max queue wait / 60s, `P` is a
  placeholder ping cost (0.0), and `R` is mean RD / 350.
- `search.rs` — `SearchStrategy` trait + `SearchStrategyKind` enum (spec §7.5)
  with three implementations: `GreedySearch` (nearest-by-rating fill),
  `RandomSamplingSearch { samples }` (random compositions, keep best by
  objective), and `BeamSearch { width }` (partial assignments expanded and
  truncated to `width`). NearestNeighbor/Hungarian/Genetic/IntegerProgramming/
  SimulatedAnnealing are declared in `SearchStrategyKind` but not implemented.

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
    Also: detection `observe` (if a `DetectionSystem` is present), ranking
    `rating_to_rank` → `obs.visible_rank` (if a `RankMapper` is present),
    adversarial-agent `tick` per participant, satisfaction-based retention
    (if a `SatisfactionModel` is present: `retention_probability` below the
    threshold schedules `PlayerQuit` instead of re-queue), and emits
    `RatingUpdateEvent` + `DetectionCheckEvent`s. The satisfaction queue-time
    input is the real join→formation wait captured at **formation** time in a
    `pending_queue_times` map (`handle_match_formed`) — computing it at MatchEnd
    would measure the match duration and drive every player's satisfaction to
    quit.
  Forming is capped by `matches_formed` (guarantees the loop terminates at
  exactly `max_matches` completed matches); `find_matches` is invoked with the
  world's rng temporarily swapped out because the matchmaker signature takes
  `&World` + `&mut SimRng`. `MachineState::new(rating, outcome, matchmaker,
  metrics, config)` (extras default to None/empty) and
  `MachineState::with_extras(..., detection, ranker, adversarial_agents,
  satisfaction)`; `matches_formed()` getter (field private).
- `handle_detection_check` → evaluate a player via the detection system and
  apply the recommended intervention (e.g. `Ban` schedules `PlayerQuit`).
- `handle_ranking_update` → re-derive each player's `visible_rank` from their
  current rating via the ranker.
- `lib.rs` — `MatchLoop { state: Arc<Mutex<MachineState>>, world, engine }`
  with `new(rating, outcome, matchmaker, metrics, config, seed)` and
  `with_extras(..., seed, detection, ranker, adversarial_agents, satisfaction)`
  registering the seven handlers on the `EventEngine`, scheduling initial
  `PlayerJoin`s
  (sorted by `PlayerId.0` — `HashMap` iteration order is randomized via
  `RandomState`, so unsorted seeding would break determinism) plus an initial
  `MatchTimer`, `run()` that ticks to completion, `run_until(SimTime)`
  (uses `engine.peek_time()`), and `finalize_metrics()`. Initial `PlayerJoin`
  order + equal-time heap pops make the whole experiment deterministic for a
  given seed. Re-exports `LoopConfig`, `MachineState`, and the `handle_*` fns.

**`matchlab-metrics` is implemented with the Lua-native collectors** (metrics
are the sole legitimate reader of `PlayerReality` besides the simulation):
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
- `lua.rs` — `LuaMetricCollector`: implements `MetricCollector` by delegating
  to a script's `on_record` / `compute`; reads the script's `name` global
  (required) and optional `time_buckets` function and `needs_population = true`
  global (which makes the snapshot carry the full population, not just match
  participants). The snapshot includes observation + reality fields (`true_skill`,
  `improvement_rate`, `reality_games_played`) — metrics only. Scripts accumulate
  samples in the VM context table (O(1) per call, no per-call round-trip).
- Metric scripts ship under `plugins/metrics/` (one per built-in metric):
  match_quality, queue_time, rating_accuracy (with `time_buckets` → the
  `rating_accuracy_by_time` convergence series), match_inequality, ndcg,
  dimensionality_fidelity, convergence, responsiveness, stability, streaks,
  population_health, smurf. Each reproduces the reference collector's semantics
  (rating_accuracy_by_time is byte-identical through the all-Lua path).
- `cohort.rs` — `CohortFilter` enum (All, SkillRange, Archetype,
  GamesPlayedRange, Region, PartySize, SessionLength, RankTier,
  IsSmurfByProperties) + `tier_for_skill(skill) -> tier` string mapping.

**`matchlab-experiments` is implemented with the config + runner:**
- `config.rs` — serde types for the full experiment manifest (spec §13.2):
  `ExperimentConfig`, `ExperimentSpec` (name, optional description, seed,
  population/game/matchmaking/rating/detection/ranking/metrics/objectives/
  adversarial/satisfaction/cohorts/duration/output), `PopulationSpec`/
  `ArchetypeSpec`/`DistributionSpec`
  (normal/uniform/log_normal), `GameSpec` (with `variant` + flattened params),
  `MatchmakingSpec`
  (algorithm + flattened params, max_queue_time), `RatingSpec { systems: Vec<RatingSystemSpec> }`
  with `RatingSystemSpec { name, params }` flatten, `DetectionSpec`,
  `SmurfDetectionSpec`, `RankingSpec`, `RankBracketSpec`, `ObjectiveWeightsSpec`,
  `AdversarialSpec`/`AdversarialAgentSpec`, `SatisfactionSpec`/
  `SatisfactionWeightsSpec`,
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
- `factorial.rs` — `FactorialDesign { factors }` (spec §13.5) with
  `generate_configs(&base) -> Vec<ExperimentConfig>` producing the Cartesian
  product of factor values. Each factor is a dot-separated config path with
  values applied via `set_nested_value` (reflects to a YAML tree, inserts the
  leaf key — handles both mapping keys and `systems.0.name`-style sequence
  indices — then re-deserializes). Note: fixes the spec's reference, which
  replaced the whole sub-mapping instead of the leaf.
- `counterfactual.rs` — `GameHistory` (`record(match, world)` captures each
  match + participant observation snapshot) and `counterfactual_eval(&history,
  &[(&str, Box<dyn RatingSystem>)]) -> HashMap<String, Vec<(PlayerId,
  RatingState)>>` (spec §13.6): replays identical history through multiple
  rating systems, preserving full `RatingState` across matches and
  budget-sanitizing each result via `filter_match_result` before `update`.
- `runner.rs` — `ExperimentRunner::run(&ExperimentConfig) ->
  Result<ExperimentResult, String>`: generates the population, builds the
  rating system via `matchlab_rating::registry::from_script`/`from_name`
  (params flattened to a `serde_yaml::Value::Mapping`), builds the outcome
  model via `matchlab_game::lua::LuaOutcomeModel::load` (script path from
  `game.script`), builds the matchmaker via
  `matchlab_matchmaking::lua::LuaMatchmaker::load` (`matchmaking.script`),
  builds optional detection via
  `matchlab_detection::lua::LuaDetectionSystem::load` (`detection.script`),
  ranking (`BracketRankMapper`), adversarial agents, and satisfaction model,
  registers the named metric
  collectors (all 13, errors on unknown names), and runs `MatchLoop` to the
  `DurationSpec` bound. Computes `utility_score` from `objectives` weights via
  `ObjectiveFunction`. Returns
  `ExperimentResult { experiment_id = "{name}-{config_hash}", name,
  config_hash, git_commit, timestamp, matches_completed, matches_formed,
  simulated_time_secs, metrics, utility_score }` (`utility_score` is `None`
  unless objective weights are configured); the timestamp is a
  hand-rolled ISO-8601 UTC
  string (no chrono dep). `metrics` is a `BTreeMap` (not `HashMap`) so JSON
  serialization key order is deterministic across processes. Each registered
  collector's `time_buckets()` (if present) is folded into `{name}_by_time`
  `TimeSeries` by the engine. Unit tests include same-seed determinism
  (identical metrics), a sim-time bound check, objective scoring, all-metric
  registration, and expanding_window/fatigue runs. `lib.rs` re-exports
  the public API.
- Binary `src/main.rs` — `matchlab run <manifest.yaml>` (exit 0/1/2): loads via
  `inherit::load`, runs, and delegates output to `matchlab-analysis` — writes
  the result as pretty JSON to `<output.directory>/<experiment.name>.json`
  (`export::write_result_json`) and, when `output.report: true`, a Markdown
  report to `<name>.md` (`report::generate_report`) with config hash, git
  commit, and the metrics table. Prints a `features:` summary line listing
  enabled subsystems (detection/ranking/adversarial/satisfaction/outcome
  variant/non-batch matchmaker) and the utility score when configured.
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
- `pareto.rs` — spec §14.2: `ParetoPoint { label, values }` and
  `pareto_front(points, higher_is_better) -> Vec<&ParetoPoint>` — the set of
  non-dominated points (a point dominates another if at least as good on all
  dimensions and strictly better on one).
- `cohorts.rs` — spec §14.3: `CohortResult { name, player_count, metrics }`
  and `analyze_cohort(name, filter, world, full_metrics)` slicing players by a
  `CohortFilter` and reporting per-cohort `rating_accuracy` (per-player MAE).
- `comparator.rs` — spec §14.6: `Comparator { results, baseline }` with
  `metric_comparison() -> HashMap<String, Vec<MetricComparison>>` (side-by-side
  per metric) and `ranking() -> Vec<(&ExperimentResult, f64)>` sorted by
  `utility_score` descending (skips results without one).
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

**`matchlab-adversarial` is implemented with the Lua-native agent types:**
- `agent.rs` — `AdversarialAgent` trait (spec §15.1): `tick(&mut self,
  player_id, world)` + `objective() -> AdversarialObjective` (6-variant enum:
  `MaximizeRating`, `MinimizeGamesPlayed`, `MaximizeWinRate { target_games }`,
  `MaintainLowRating`, `WinTrade { partner }`, `Derate`). Agents act as the
  player's behavior controller (like the outcome model), so they may adjust
  reality behavior params (e.g. `quit_probability`) as well as observable
  signals.
- `lua.rs` — `LuaAdversarialAgent`: implements `AdversarialAgent` by
  delegating to a script's `tick` / `objective` functions. The adapter exposes
  a `behavior` table (quit_probability, party_id, tilt_level, win_rate,
  is_online) plus the player's observation, and writes the returned behavior
  back to reality/observations. Randomness flows through `matchlab.rng_*` from
  `world.rng`; the objective is read at load and cached.
- The agents ship as Lua scripts under `plugins/adversarial/`:
  - `afk.lua` — with `matchlab.rng_bool(go_afk_probability)` sets
    `quit_probability = 1.0`. Objective `MinimizeGamesPlayed`.
  - `deranker.lua` — while rating is above `target_rating`, raises
    `quit_probability` to 0.9 and `tilt_level` to 1.0. Objective
    `MaintainLowRating`.
  - `win_trader.lua` — links the pair into a party. Objective `WinTrade`.
  - `booster.lua` — links the duo into a party and boosts the boostee's
    `win_rate` to 1.0. Objective `MaximizeRating`.
  - `rating_farmer.lua` — with `matchlab.rng_bool(quit_probability)` sets
    `quit_probability = 1.0` and goes offline to keep `games_played` minimal.
    Objective `MaximizeWinRate`.
- `lib.rs` — re-exports `LuaAdversarialAgent` + `AdversarialAgent`/
  `AdversarialObjective`.

**`matchlab-utility` is implemented with the satisfaction model:**
- `satisfaction.rs` — `SatisfactionModel` (spec §16.1) with
  `SatisfactionWeights` (serde `Deserialize`: `match_quality`,
  `queue_time_penalty`, `win_bonus`, `loss_streak_penalty`,
  `rank_progression_bonus`, `fairness_sensitivity`, `rematch_bonus`;
  `Default` matches §16.1) and `PlayerExperience` (recent match qualities,
  queue times, outcomes, `current_streak`, `rank_change`,
  `perceived_fairness`, `rematch_rate`; `new()`/`record_match()` helpers).
  `satisfaction()` is the weighted sum (loss-streak penalty only kicks in
  below −3), `retention_probability()` is the logistic `1/(1+e^−s)`, and
  `rematch_probability()` requires a higher threshold (`1/(1+e^−0.5(s−2))`).
  `lib.rs` re-exports `SatisfactionModel`, `SatisfactionWeights`,
  `PlayerExperience`.

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
| Build plan | `docs/spec.md` §17 build order |
| Adversarial agents (booster, deranker, etc.) | `crates/matchlab-adversarial/src/` |
| Player satisfaction model | `crates/matchlab-utility/src/satisfaction.rs` |
