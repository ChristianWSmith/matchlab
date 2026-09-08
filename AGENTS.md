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
    ├── matchlab-experiments/   # runner, YAML config, factorial design, counterfactual eval, replication
    ├── matchlab-analysis/      # statistics, Pareto, cohorts, reports, provenance
    └── matchlab-validation/    # analytical-baseline regression tests (test-side references)
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
matchlab-validation    (depends on core + players + game + rating + matchmaking + loop + metrics + experiments; test-side references only)
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

### 2. Pluggability via Traits + Lua scripts

Every algorithm is a trait implementation, and every implementation is a Lua
script under `plugins/` (there are no inherent Rust algorithms):
- `RatingSystem` (trait) — `plugins/rating/` (elo, glicko2, trueskill, flat, …)
- `OutcomeModel` (trait) — `plugins/game/` (logistic, variance, composition, …)
- `Matchmaker` (trait) — `plugins/matchmaking/` (batch, expanding_window, …)
- `MetricCollector` (trait) — `plugins/metrics/` (one script per metric)
- `DetectionSystem` (trait) — `plugins/detection/` (smurf)
- `AdversarialAgent` / `SatisfactionModel` / `RankMapper` — the same model.

Swapping implementations is a one-line `script:` change in the manifest.

### 3. Reproducibility

Every experiment is deterministic given its config + seed. The `SeedManager` derives separate seeds for population, games, arrivals, behavior, matchmaking, and the master world RNG from a single experiment seed. `ExperimentResult` records config hash + git commit for exact reproduction.

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
├── matchlab-experiments/     (+ design.rs, validation.rs for  )
├── matchlab-analysis/
├── matchlab-detection/
├── matchlab-ranking/
├── matchlab-objective/
├── matchlab-adversarial/
├── matchlab-utility/
└── matchlab-validation/
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
  to control the ground-truth skill binding; always includes `role` — an
  observable attribute, gated by neither `include_skill` nor an information
  budget), `participant_to_table` (metrics
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
  `r#gen` in edition 2024. `derive(seed, index)` derives a per-stream seed
  (DefaultHasher over the pair); `StreamSeeds { master, game, matchmaker,
  behavior }` with `from_seed(seed)` uses indices 6/2/5/4 (population=1,
  game=2, arrival=3, behavior=4, matchmaker=5, master=6) so each stochastic
  subsystem draws from its own RNG and a study can fix one stream (common
  random numbers) while varying another (spec §13.7, ).
- `player.rs` — `PlayerId`, `Region`, `SkillVector` : `SkillDimension`
  with scale/min/max, `normalize()`/`denormalize()`, `validate()`), `VisibleRank`,
  `DetectionFlag`, `PlayerReality` (ground truth),
  `PlayerObservation`. Both carry an optional `role` (a fixed observable
  attribute sampled from the archetype at generation — `None` means "any").
- `match_.rs` — `MatchId`, `Team`, `MatchState`, `MatchResult`,
  `PlayerPerformance`, and `TeamComposition { team_size_a, team_size_b,
  role_a, role_b }` (XvY composition; `Default` = 5v5 no roles). The legacy
  `MatchConfig { team_size }` type was removed (unused).
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
  `time`, `skill_history` : temporal ground truth for dynamic skill),
  with private monotonic ID counters (`next_player_id()`/`next_match_id()`).
  Truth separation: `player.rs`/`world.rs` enforce the rule that algorithms
  access players via `observe()`/`observations`, never `players`/`reality()`.
  Temporal API: `record_skill_snapshot()`, `player_skill_at()`, `has_history()`,
  `skill_history()`.
- `simulation.rs` — `Simulation { world, engine }` with `new`, and
  `run(until)` / `run_to_completion()` (skips idle clock periods).

**`matchlab-players` is implemented with the population logic:**
- `archetype.rs` — `ArchetypeConfig` (serde `Deserialize`: `name`, `proportion`,
  `skill_distribution`, `skill_volatility`, `improvement_rate`, `play_frequency`,
  `session_length`, `quit_probability`, optional `initial_rating`, optional
  `role`,: optional `skill_dimensions`/`correlation`/`dynamics`) and
  `DistributionConfig` (tagged enum: `normal`, `uniform`, `log_normal`). The
  optional `initial_rating` overrides visible rating while true skill stays
  sampled — the seed of the smurf-like mismatch; no boolean smurf flag exists.
  A `role`-carrying archetype (e.g. `killer`) stamps every generated player's
  reality and observation; absent role means "any". adds multidimensional
  skill via `skill_dimensions` and correlation via `correlation.pairs`.
- `distribution.rs` —: `SkillDistribution` enum (`Independent` /
  `MultivariateNormal`), `Marginal`, `draw_skill_vector()`, `cholesky_decompose()`,
  `pearson_correlation()`. Generates multidimensional skill vectors with
  configurable correlation structure.
- `dynamics.rs` —: `SkillDynamics` trait with `advance()`, `DynamicsContext`,
  and built-in models: `LinearDynamics` (current behavior formalized),
  `ExperienceDynamics` (rate decreases with games played), `DecayDynamics`
  (skill declines after inactivity), `StationaryDynamics` (no-op).
- `population_dynamics.rs` —: `PopulationDynamics` trait with `tick()`,
  `PopulationEvent` enum, and built-in models: `EntryExitDynamics` (players
  join/leave), `PartyFormationDynamics` (parties form), `StrategyShiftDynamics`
  (players change strategy).
- `skill.rs` — `SkillProcess { improvement_rate, volatility }` with
  `advance(&SkillVector, &mut SimRng)` (one time step per dimension:
  `val + improvement_rate + N(0, volatility)`, floored at 0). Skills are
  **static** by default — the population is generated once at `t=0` and never
  changes — and dynamic only when an experiment sets
  `game.skill_update_interval_secs`: the loop's periodic `SkillChangeEvent`
  then advances every *online* player's reality skill each interval. With
  `improvement_rate=0, volatility=0` `advance` is the identity (no-op), the
  v0.1 baseline.
- `population.rs` — `PopulationConfig { size, archetypes }` and
  `PopulationGenerator::generate(config, rng) -> (Vec<PlayerReality>,
  Vec<PlayerObservation>)`. Each player is drawn from its archetype's
  distribution; observation uses `initial_rating` if set else the sampled skill
  (`rating_deviation: 350.0`, `games_played: 0`, etc. per §5.8). The
  archetype's `role` is copied into both the reality and the observation at
  generation.: when `skill_dimensions` is present, the generator produces
  multidimensional `SkillVector` values; otherwise falls back to single-dimension
  via `skill_distribution`. The
  observation's `skill_vector`/`hidden_mmr` carry the **true skill** so the
  outcome model can decide matches from ground truth; only `rating` is the
  initial/visible ladder value. Proportions become integer counts via the
  **largest-remainder method** so they always sum exactly to `size`.

**`matchlab-game` is implemented with the Lua-native outcome models:**
- `outcome.rs` — `OutcomeModel` trait (spec §6.1): `win_probability(team_a,
  team_b)` and `simulate(match_id, team_a, team_b, rng) -> MatchResult`. Takes
  `PlayerObservation` only — never `PlayerReality` (truth separation).
- `performance.rs` —: `PerformanceModel` trait with `realize()`,
  `PerformanceContext`, `GaussianNoiseModel` (configurable variance), and
  `DeterministicModel` (zero-noise baseline). Separates latent skill from
  realized performance.
- `team.rs` —: `TeamModel` trait with `team_strength()`, `TeamContext`,
  `AdditiveTeamModel` (sum, current behavior formalized), `WeightedTeamModel`
  (per-dimension weights), `ComplementaryTeamModel` (synergy bonus for diverse
  skill profiles).
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
  `recommend_action(&self, result) -> InterventionAction`. adds
  `EcosystemDetector` trait with `observe_match`, `evaluate` →
  `EcosystemDetectionResult`, `recommend_intervention` → `EcosystemIntervention`.
- `intervention.rs` — the `InterventionAction` enum (the escalation policy
  logic lives in the Lua script). adds `EcosystemIntervention` enum
  (Warning, Restriction, QueueModification, MatchmakingIsolation,
  RemovalFromPopulation).
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
  game_mode, role, latency_ms) — `role` is the player's queued role label, set
  from the observation at queue time — and `Queue` with `enqueue`, `remove`,
  `remove_batch`, `waiting_time` (saturating `now − joined_at` — the basis of the
  v0.1 queue-time metric), `entries`/`len`/`is_empty`, `from_entries`.
- `party.rs` —: `Party` struct (id, members, created_at, party_size_limit),
  `PartyRegistry` (tracks active parties), `check_party_integrity` (validates no
  party is split across matches). Solo players are parties of size 1.
- `latency.rs` —: `LatencyMatrix` (configurable per-region-pair latencies),
  `LatencyModel` (estimates latency from GeoLocation + jitter), `TeamLatencyStats`
  (mean, max, variance, threshold fraction).
- `constraint.rs` —: `Constraint` trait (v0.1), `HardConstraint` trait(),
  `TeamConstraint` (role requirements + team size), `RoleRequirement`,
  `TeamSizeConstraint`, `PartyIntegrityConstraint`, `RoleConstraint`,
  `MaxLatencyConstraint`, `RegionConstraint`, `ConstraintViolation`.
- `objective.rs` —: `MatchObjective` (legacy weighted sum), `MatchObjectiveVector`
  (multi-objective: skill/role/latency/party/wait/utilization), `ObjectiveWeights`,
  `SoftObjective` trait, `SkillBalanceObjective`, `LatencyObjective`, `WaitTimeObjective`.
- `policy.rs` —: `MatchPolicy` (hard constraints + weighted soft objectives),
  `HardConstraint` trait, `SoftObjective` trait.
- `candidate.rs` —: `CandidateGenerator` trait, `ExhaustiveGenerator`,
  `GreedyNearestGenerator`, `RandomSamplingGenerator`.
- `selection.rs` —: `MatchSelectionPolicy` trait, `GreedySelection`,
  `WeightedObjectiveSelection`, `LexicographicSelection`, `ThresholdSelection`.
- `matchmaker.rs` — `Matchmaker` trait (spec §7.2)
  `find_matches(queue, world, teams: &TeamComposition, now, rng) ->
  Vec<ProposedMatch>`; `ProposedMatch { team_a, team_b, quality_score }` with
  static `match_quality` = `1 − (|avg_a − avg_b| / 400).clamp(0,1)` computed
  from **observations** only.
- `constraint.rs` — `Constraint` trait (spec §7.3). No concrete constraints in
  v0.1; the matchmakers run with an empty list.
- `lua.rs` — `LuaMatchmaker`: implements `Matchmaker` by delegating to a
  script's `find_matches` function. The queue is snapshotted to a Lua array
  (player_id, rating, rating_deviation, games_played, win_rate, `idx`,
  `joined_at_secs`, `wait_secs`, region, party_id, latency_ms, game_mode,
  `role`) — observations only, never `PlayerReality`; `role` is nil-safe (absent
  ⇒ `nil`, "any"). The `&TeamComposition` is pushed as
  a second `teams` arg (`{a = {size, role}, b = {size, role}}`, role nil when
  unset). Scripts set `quality_score` or the adapter falls back to
  `ProposedMatch::match_quality`.
- The matchmakers ship as Lua scripts under `plugins/matchmaking/`:
  - `batch.lua` — spec §7.8. **Rating-balanced** formation: sort candidates by
    `rating` (ties by `joined_at_secs`, then `idx` — Lua `table.sort` is
    unstable, so the index tie-break preserves the Rust stable-sort behavior
    and keeps results byte-identical) and assign alternately to team A / team B
    in consecutive `size_a + size_b` blocks (skip a turn when the target team
    is already full, so equal sizes reproduce the pre-XvY `2 × team_size`
    alternation exactly). Adjacent-by-rating players land on
    opposite teams, so the two teams are balanced and `match_quality` stays
    ~0.96–0.98 (the naive FIFO pairing caps near 0.68 on the standard
    population, failing the quality exit criterion). **Role-aware():**
    when `teams.a.role`/`teams.b.role` are set, team A is filled exclusively
    from entries whose `role` matches `teams.a.role` and team B from the
    `teams.b.role` pool (each pool sorted by rating, consumption alternated);
    an entry matching neither waits. Both roles unset ⇒ the legacy single-queue
    alternation runs **byte-identically** (pinned by the
    `batch_roles_unset_is_byte_identical_regression` validation test).
  - `expanding_window.lua` — spec §7.6 with stepped tiers
    `[(max_secs, allowed_diff)]` (default 5s→25, 10s→50, 20s→100, 30s→200,
    fallback `max_window: 400`) — skills matched within a window that widens
    with queue wait. **Role-aware:** team A from the `teams.a.role` pool, team B
    from `teams.b.role`, role-less pool per side when unset; no cross-pool
    borrowing. When a pool can't fill its side, those players wait (stall —
    intended for strict/sparse roles).
  - `strict.lua` — spec §7.7: only matches players within a fixed skill diff;
    outliers may wait indefinitely (intended "strict" behavior). **Role-aware:**
    same per-role fill rules as expanding_window; a role-starved side simply
    waits like a skill outlier.
  - `hub_spoke.lua` — spec §7.9: partitions the queue by region (sorted region
    keys for determinism); under-capacity regions use an inlined regional
    greedy (no nested matchmakers in Lua), overflow regions fall to the hub
    path (longest-waiting first). **Role-aware:** both paths fill per-side by
    role; when both sides declare the *same* role the hub overflow uses one
    shared stream (A takes `size_a`, B the next `size_b`) so nobody is assigned
    twice.
  - `random.lua` — uniform-random formation for the feedback-loop comparison:
    draws `size_a + size_b` players at random from the queue (via
    `matchlab.rng_range`, so it is deterministic per seed) with no rating
    balancing; the third policy in the `rating × matchmaker` cell grid.
- `objective.rs` — `MatchObjective { weight_quality, weight_queue_time,
  weight_ping, weight_rating_uncertainty }` (spec §7.4) with
  `score(proposed, queue_entries, world) = w_q·Q − w_t·T − w_p·P − w_r·R`
  where `Q` is balance quality, `T` is max queue wait / 60s, `P` is a
  placeholder ping cost (0.0), and `R` is mean RD / 350.
- `search.rs` — `SearchStrategy` trait + `SearchStrategyKind` enum (spec §7.5)
  with three implementations: `GreedySearch` (nearest-by-rating fill),
  `RandomSamplingSearch { samples }` (random compositions, keep best by
  objective), and `BeamSearch { width }` (partial assignments expanded and
  truncated to `width`). All take `&TeamComposition` (per-side `team_size_a`/
  `team_size_b`; equal sizes preserve the legacy `team_size` behavior exactly —
  the role fields are carried for signature parity, role filtering happens at
  the script/queue level). NearestNeighbor/Hungarian/Genetic/IntegerProgramming/
  SimulatedAnnealing are declared in `SearchStrategyKind` but not implemented.

**`matchlab-loop` is implemented with the event-handler machine:**
- `history.rs` — `GameHistory`(): the counterfactual recording trace.
  `record(match, world)` captures each resolved match + its pre-update
  participant observation snapshot + a metrics-only `RealitySnapshot`
  (`true_skill`/`improvement_rate`/`games_played`) and `times_secs`. Lives in
  `matchlab-loop` (loop records; experiments replay; re-exported from both).
  `MachineState.history: Option<GameHistory>` is `Some` only when
  `LoopConfig.record_history` is set (a recording run is otherwise
  byte-identical to a plain one).
- `machine.rs` — `LoopConfig { teams, batch_interval_ticks, rejoin_delay,
  max_matches, skill_update_interval: Option<SimTime>, stream_seeds: StreamSeeds,
  record_history: bool }`
  and `MachineState { population: HashMap<PlayerId,
  (PlayerReality, PlayerObservation)>, queue, active_matches: HashMap<MatchId,
  MatchResult>, matches_completed, matches_formed, pub metrics: MetricsEngine,
  rating_system, outcome_model, matchmaker, game_rng, matchmaker_rng,
  behavior_rng, pub history: Option<GameHistory> }`. The `handle_*` functions are
  plain `(world, event, state)
  -> Vec<Box<dyn Event>>` pure-per-event transforms, so they are unit-testable
  without the engine. `MachineState::with_extras` seeds the three per-stream
  RNGs from `config.stream_seeds`; `MatchTimer`/`SkillChangeEvent`/`MatchFormed`/
  adversarial ticks draw from `state.matchmaker_rng`/`state.game_rng`/
  `state.game_rng`/  `state.behavior_rng` respectively().
  - `PlayerJoin` → add reality+observation to `World`, set `queue_joined_at`,
    schedule `PlayerQueue`.
- `ecosystem.rs` —: `EcosystemLoop` connecting matchmaking to adaptive
  populations. `EcosystemTickResult` with matches_formed, population_events,
  detection_events, agent_actions. The loop integrates strategic agents,
  population dynamics, and queue processing into a first-class simulation loop.
  - `PlayerQueue` → enqueue the player (entry built from the live observation,
    `QueueEntry.role` copied from `obs.role`)
    and refresh `obs.queue_joined_at` to `world.time` (keeps the
    queue-time metric measuring the current join→formation wait, including
    re-queues after a match).
  - `MatchTimer` (new periodic event) → call `find_matches` with
    `state.matchmaker_rng` (the matchmaker signature takes `&World` +
    `&mut SimRng`), cap formation to
    the remaining `max_matches − matches_formed` budget (a formed match is an
    in-flight obligation, so over-capping on `matches_completed` would overshoot),
    emit one `MatchFormed` per proposal + re-schedule the next timer.
  - `SkillChangeEvent` (periodic, only when `LoopConfig.skill_update_interval`
    is set — dynamic skill) → for every *online* player, in ascending
    `PlayerId` order, advance `reality.skill` via `SkillProcess` (drawing from
    `state.game_rng`) and refresh
    `observation.skill_vector`/`hidden_mmr` from it, then re-schedule itself.
    Only `PlayerReality.skill` mutates; ratings and matchmaking never read it
    (truth separation). Offline players are skipped, so the exact
    `S(t) = S0 + k·t` trajectory holds only for continuously-online players.
  - `MatchFormed` → simulate via the outcome model with `state.game_rng`,
    `metrics.record_match(&result, world)` (recorded at **formation** time —
    recording at MatchEnd made `queue_time` ≈ match duration, breaking the
    "queue time = actual wait" exit criterion), store the `MatchResult` in
    `active_matches` + `World.matches[InProgress]`, schedule `MatchEnd` at
    `now + duration`.
  - `MatchEnd` → if `state.history` is present ( recording run), first
    `GameHistory::record` captures this match + its **pre-update** observation
    and ground-truth snapshots (the counterfactual trace); then
    `rating_system.update` on a **budget-sanitized** result
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
  exactly `max_matches` completed matches). `MachineState::new(rating, outcome,
  matchmaker, metrics, config)` (extras default to None/empty) and
  `MachineState::with_extras(..., detection, ranker, adversarial_agents,
  satisfaction)`; `matches_formed()` getter (field private).
- `handle_detection_check` → evaluate a player via the detection system and
  apply the recommended intervention (e.g. `Ban` schedules `PlayerQuit`).
- `handle_ranking_update` → re-derive each player's `visible_rank` from their
  current rating via the ranker.
- `lib.rs` — `MatchLoop { state: Arc<Mutex<MachineState>>, world, engine }`
  with `new(rating, outcome, matchmaker, metrics, config)` (no seed — the world
  RNG is seeded from `config.stream_seeds.master`) and
  `with_extras(..., config, detection, ranker, adversarial_agents, satisfaction)`
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
  adversarial/satisfaction/cohorts/duration/output, plus optional
  `replication: Option<ReplicationSpec>` — when present the manifest is a
  1-arm replicated study, ), `PopulationSpec`/
  `ArchetypeSpec`/`DistributionSpec`
  (normal/uniform/log_normal), `GameSpec` (with `variant` + flattened params,
  `teams: TeamSpecs { a, b }` — each a `TeamSpec` untagged over int `size` or
  `{ size, role }`, default 5v5 — plus `skill_update_interval_secs:
  Option<f64>` — absent ⇒ static skill),
  `MatchmakingSpec`
  (script + flattened params, max_queue_time), `RatingSpec { systems: Vec<RatingSystemSpec> }`
  with `RatingSystemSpec { name?, script?, params }` flatten, `DetectionSpec`,
  `RankingSpec`, `ObjectiveWeightsSpec`,
  `AdversarialSpec`/`AdversarialAgentSpec` (script + params), `SatisfactionSpec`
  (script + params),
  `CohortSpec`/`CohortFilterSpec` (tagged enum), `DurationSpec { matches, max_time }`,
  `OutputSpec { directory, formats, plots, report }`. `cohorts` is a required `Vec`
  (manifests use `cohorts: []` when unused).
- `inherit.rs` — YAML-level config inheritance: `load(path)` /
  `resolve_str(text, base_dir)` / `load_value`, with `base: <path>` keys
  resolved recursively and `deep_merge` (mappings merge recursively; scalars
  and sequences are replaced by the child). Enables spec §13.1's controlled
  one-variable-differs experiments; the `experiments/base/` directory holds
  inherited base configs.
- `seed.rs` — `SeedManager` (spec §13.7): separate seeds for
  population/game/arrival/behavior/matchmaker/master derived from the one
  experiment seed via `derive(seed, index)` (re-exported from
  `matchlab_core::rng`), plus `hash_config(&ExperimentConfig)` (a
  `#[derive(Default)]` `DefaultHasher` over length-prefixed serialized
  fields **plus the contents of every referenced Lua script**, so a script edit
  changes the experiment identity) and `git_commit_hash()` for `ExperimentResult`.
- `factorial.rs` — `FactorialDesign { factors }` (spec §13.5) with
  `generate_configs(&base) -> Vec<ExperimentConfig>` producing the Cartesian
  product of factor values. Each factor is a dot-separated config path with
  values applied via `set_nested_value` (pub, re-exported) — reflects to a YAML
  tree, inserts the
  leaf key — handles both mapping keys and `systems.0.name`-style sequence
  indices — then re-deserializes). Note: fixes the spec's reference, which
  replaced the whole sub-mapping instead of the leaf.
- `counterfactual.rs` — the counterfactual replay path (spec §13.8, ).
  `GameHistory` now lives in `matchlab-loop` (re-exported
  `pub use matchlab_loop::{GameHistory, RealitySnapshot}`); its class-method
  `record(match, world)` captures each match + participant observation snapshot
  **plus a metrics-only ground-truth binding** (`RealitySnapshot`:
  `true_skill`/`improvement_rate`/`games_played`) and the sim time of each
  match. `counterfactual_eval(&history, &[(&str, Box<dyn RatingSystem>)]) ->
  HashMap<String, Vec<(PlayerId, RatingState)>>` (spec §13.6) keeps the
  returns-final-states contract: replays identical history through multiple
  rating systems, preserving full `RatingState` across matches and
  budget-sanitizing each result via `filter_match_result` before `update`.
  `ReplayEngine::replay(history, system, config, requested_metrics) ->
  Result<ExperimentResult, String>` (the study-arm path): rebuilds
  per-participant observations from evolving `RatingState`, populates
  `world.players` from the reality snapshots, records the **original** match
  through the real `MetricsEngine`, and budget-sanitizes each `update` —
  deterministic (no `SimRng`). **Start-state contract ( regression):**
  the live loop never materializes a rating system's cold `initialize` state —
  a player's first `update` is a pure function of their population-generated
  observation — so both `ReplayEngine` and `counterfactual_eval` seed a
  player's initial `RatingState` from the first recorded observation
  (`rating`/`rating_deviation`/`volatility`, `games_played = 0`), falling back
  to `initialize` only when no snapshot was recorded (seeding from 1000
  instead inflated replay `rating_accuracy` several-fold on the standard
  population). With the fix, replay ratings track the recorded live ratings
  bit-for-bit. Replay-evaluable metric contract:
  `REPLAY_VALID_METRICS = [rating_accuracy, convergence, stability, streaks]`;
  N/A metrics (queue_time, match_quality, ndcg, population_health) are
  silently dropped because they need matchmaking/population state that doesn't
  exist offline.
- `runner.rs` — `ExperimentRunner::run(&ExperimentConfig) ->
  Result<ExperimentResult, String>` (delegates to `run_recording(config,
  record_history)` which returns `(ExperimentResult, Option<GameHistory>)`;
  recording adds no noise): generates the population, builds the
  rating system via `matchlab_rating::registry::from_script`/`from_name`
  (params flattened to a `serde_yaml::Value::Mapping`), builds the outcome
  model via `matchlab_game::lua::LuaOutcomeModel::load` (script path from
  `game.script`), builds the matchmaker via
  `matchlab_matchmaking::lua::LuaMatchmaker::load` (`matchmaking.script`),
  builds optional detection via
  `matchlab_detection::lua::LuaDetectionSystem::load` (`detection.script`),
  ranking (`LuaRankMapper`), adversarial agents, and satisfaction model,
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
  registration, and expanding_window/fatigue runs. `ExperimentResult`
  derives `PartialEq`. `lib.rs` re-exports
  the public API.
- `replicate.rs` — the replication engine (spec §13.8, ): `SeedStrategy`
  (`independent`/`crn`/`counterfactual`, serde YAML names) and
  `ReplicationSpec { count, strategy, base_seed }` where per-replicate seed =
  `derive(base_seed, replicate_index)` and per-arm seed is `derive(replicate,
  arm_index)` for independent arms, the shared replicate seed for CRN, and the
  live seed / derived for counterfactual(). `ReplicationRunner::run_single`
  (embedded `replication:` block) and `::run_arms(&[ArmConfig], spec)` set
  `config.experiment.seed` per replicate; for `strategy: counterfactual` the
  first arm runs live via `ExperimentRunner::run_recording(cfg, true)` and each
  other arm replays that replicate's captured history through
  `ReplayEngine::replay` with its own rating system —
  yielding serde `ReplicateResult { replicate_index, seed, parent_seed, result }`
  / `ArmResult` / `StudyResult { study_id = "{name}-{hash8}-{strategy}-r{count}",
  name, config_hash, git_commit, strategy, replication_count, arms }`. Tests
  cover the seed-contract math, CRN/independent distinctness, JSON round-trip
  (structural — floats can re-parse one ULP off), deep-equal determinism
  net of the wall-clock `timestamp`.
- `study.rs` — the top-level study manifest (spec §13.8, ):
  `StudyConfig { study: StudySpec { name, base, arms, replication, metrics,
  cohorts, output } }` with `ArmSpec { name, overrides: BTreeMap<String, Value> }`
  — dotted-path arm overrides applied to the deep-cloned resolved base via the
  pub `set_nested_value`; `experiment.seed` is rejected as an override
  (replication owns seeds). `StudyRunner::load(path)` resolves `base:` relative
  to the study file's dir; `StudyRunner::run(&StudyConfig)` resolves base via
  `inherit::load`, applies study-level `metrics`/`cohorts`, builds the
  `ArmConfig`s, delegates to `ReplicationRunner::run_arms`, then overwrites
  `name`/`study_id` with the study name (id = `"{name}-{hash8}-{strategy}-r{n}"`).
  `config_hash` = hash of arm[0]'s resolved full config. Tests: both manifest
  shapes parse to the same `ReplicationSpec`, overrides touch only their leaf,
  seed override rejected, 2-arm CRN mini study deterministic net of timestamps.
- `design.rs` — first-class experimental designs(): `ExperimentalDesign`
  enum with variants `Independent`, `Paired`, `Crn`, `Counterfactual`, `Blocked`.
  Each carries structural metadata (pairing key, block factor, parent study).
  `DesignType` is a derived flat summary for serde back-compat.
- `validation.rs` — design validation(): `DesignValidation` with errors
  and warnings; `validate_design` checks for missing factor levels, insufficient
  replication, zero conditions, missing counterfactual parent, empty blocks,
  and design×factor consistency.
- `factorial.rs` extensions(): `FactorGrid` (typed factors/levels),
  `Condition` (factor→level assignments), `DesignGenerator` (balanced/randomized
  allocation), `FractionalFactorial` (resolution-III half-fraction).
- `replicate.rs` extensions(): `StudyExtension`, `ExtendedStudyResult`,
  `StudyResult::extend` for incremental experiment growth.
- `identity.rs` —: `StudyId`, `ExperimentId`, `ReplicationId`, `RunId`,
  `RunMetadata`, `RunStatus`, `StudyMetadata`, `ExperimentMetadata`,
  `ReplicationMetadata` — first-class identity for the experiment hierarchy.
- `store.rs` —: `ExperimentStore` trait, `SqliteStore` (SQLite-backed,
  Mutex-wrapped for thread safety), `StoreError`.
- `observation.rs` —: `ObservationPolicy` (AggregateOnly/ReplicationLevel/
  MatchLevel/PlayerLevel/EventLevel), `ObservationWriter` trait,
  `NullObservationWriter`, `FileObservationWriter` (JSONL), `BufferedObservationWriter`.
- `checkpoint.rs` —: `Checkpoint`, `CheckpointManager` trait,
  `FileCheckpointManager`, `WorldSnapshot`, `QueueSnapshot`, `MetricsSnapshot`.
- `parallel.rs` —: `WorkerJob`, `WorkerResult`, `Worker` trait,
  `LocalWorker`, `WorkerPool` for replication-level parallelism.
- `scheduler.rs` —: `ExecutionScheduler`, `SchedulerStatus` for managing
  job lifecycle, retries, and resource limits.
- `formats.rs` —: `ArtifactFormat`, `ExportConfig`, `VersionedArtifact`,
  `validate_format_version` for stable serialization formats.
- `distributed.rs` —: `DistributedJob`, `DistributedResult`,
  `DistributedWorker` trait, `DistributedCoordinator`, `LocalDistributedWorker`
  — clean worker boundary for future distributed execution.
- Binary `src/main.rs` — `matchlab study <study.yaml> [--replicates N] [--json]` loads via `StudyRunner::load`
(base resolved relative to the study file's dir), runs, and writes the `StudyResult`
as pretty JSON to `<study.output.directory>/<study_id>.json` plus the aggregate
`study_stats.json` (via `matchlab_analysis::study::write_study_result_json`), then
renders the **study report** (`generate_study_report`) — the exit-criterion
block: per-arm metric table (mean + 95% CI) and pairwise `Δ`, `95% CI [..]`,
`Cohen's d`, and `relative Δ` lines per metric. `--replicates` overrides the
manifest count (CI smoke runs); `--json` prints the `StudyStats` aggregate JSON on
stdout (completion lines go to stderr). `matchlab run` with an embedded
`replication:` block renders the same report for its 1-arm set.
`matchlab run <manifest.yaml>` (exit 0/1/2): loads via
  `inherit::load`, runs, and delegates output to `matchlab-analysis` — writes
  the result as pretty JSON to `<output.directory>/<experiment.name>.json`
  (`export::write_result_json`) and, when `output.report: true`, a Markdown
  report to `<name>.md` (`report::generate_report`) with config hash, git
  commit, and the metrics table. Prints a `features:` summary line listing
  enabled subsystems (detection/ranking/adversarial/satisfaction/outcome
  variant/non-batch matchmaker) and the utility score when configured.
  `matchlab compare <result.json>... [--json]` reads one or more exported
  result JSONs (`ExperimentResult` is `Deserialize`), prints a
  `generate_comparison_report` (Markdown by default, or the JSON report with
  `--json`), and finishes with a utility ranking (`Comparator::ranking`) when
  any result carries a `utility_score`.
- `experiments/v0_1_basic.yaml` — the spec §17 minimal v0.1 manifest (10,000
   players, team size 5, cold ladder start with `initial_rating: 1000`, flat
   skill, no detection/ranking/objective/cohorts, capped by `max_time: 604800`).
   The `initial_rating` deviates from the literal §17 snippet to provide a
   meaningful convergence scenario: visible ratings start at 1000 while true
   skill is sampled from N(1000, 250), so Elo has something to learn.
 - `experiments/base/standard.yaml` — inherited base config (2,000 players,
   mixed archetypes incl. a smurf archetype, elo + logistic + batch).
 - `experiments/lua_systems_test.yaml` — smoke test that Lua-native systems
   (elo/logistic/batch) run through the standard population.
 - `experiments/glicko_comparison.yaml` — Glicko-2 (script) on the standard
   population.
 - `experiments/matchmaker_comparison.yaml` — expanding_window (script) on the
   standard population.
 - `experiments/detection_test.yaml` — smurf detection enabled.
  - `experiments/dbd_1v4.yaml` — Dead-by-Daylight-style 1v4 asymmetric
    manifest: killer archetype (role killer, N(1250,250)) vs survivor archetype
    (role survivor, N(1000,100)), teams 1v4 role-gated, batch matchmaker, elo.
  - `experiments/full_featured.yaml` — all subsystems enabled: fatigue outcome,
   smurf detection, Lua rank brackets, adversarial agents (afk + deranker),
   satisfaction, all 12 metrics, objectives.
 - `experiments/novel_rating.yaml` — the dogfood example: a rating system with
   no Rust equivalent (`plugins/rating/decay_elo.lua`, Elo + idle decay) and a
   custom metric (`plugins/metrics/avg_rating_gap.lua`) added purely as Lua.
 - `experiments/feedback_loop/*.yaml` — nine factorial cells
   `feedback_{elo,glicko2,trueskill}_{random,strict,expanding}.yaml` (the
   `rating × matchmaker` grid, all inheriting `base/standard.yaml`); the
   `factorial_hand_derives_feedback_loop_cells` test proves
   `FactorialDesign::generate_configs` derives the same nine from a base config,
   and `matchlab compare results/feedback_*.json` reads them back for the
   side-by-side feedback-loop comparison.
- `experiments/studies/elo_vs_glicko.yaml` — the 2-arm CRN study():
    elo vs glicko2 on the shared `base/standard.yaml` (only the rating system
    leaf differs), 50 replicates by default (CI-fast `--replicates 50`,
    production `--replicates 500`),
    `matchlab study experiments/studies/elo_vs_glicko.yaml`.
    The acceptance run (N = 50) is recorded in docs/spec.md §18.8:
    rating_accuracy Δ = +208.32, 95% CI [207.43, 209.28], Cohen's d = 84.12.
  - `experiments/studies/elo_vs_glicko_replay.yaml` — the same 2-arm
    comparison under `strategy: counterfactual` , renamed from
    `elo_vs_glicko_counterfactual.yaml` in ): the elo arm runs live
    and records, the glicko2 arm replays that identical 50k-match history
    through `ReplayEngine` (only rating_accuracy-family metrics survive).
    The acceptance run (N = 10) is also recorded in spec §18.8.

**`matchlab-analysis` is implemented with the statistical layer + reporting:**
- `stats.rs` — re-exports `matchlab_metrics::stats` as the `summary`/
  `Summary`/`summary_to_result` API (spec §14.1). The canonical implementation
  stays in `matchlab-metrics` to keep the metrics-only-core boundary.
- `pareto.rs` — spec §14.2: `ParetoPoint { label, values }` and
  `pareto_front(points, higher_is_better) -> Vec<&ParetoPoint>` — the set of
  non-dominated points (a point dominates another if at least as good on all
  dimensions and strictly better on one).
- `effect.rs` —  : the hand-rolled aggregate statistics module.
  `extract_scalar(&MetricResult) -> Option<f64>` reduces any metric to a scalar
  (`Scalar`→value, `Summary`→mean, `TimeSeries`→mean of bucket means;
  `Distribution`/`Histogram`→`None`). CIs: `bootstrap_ci`, `paired_bootstrap_ci`,
  `student_t_ci`, `welch_ci` (Welch–Satterthwaite df, ), `wilson_ci`, all
  returning `ConfidenceInterval { lower, upper, conf, method: CiMethod }`.
  `ci(control, treatment, conf, seed, paired)` dispatches PairedBootstrap for
  paired, WelchT for independent n≥30, Bootstrap otherwise().
  `EffectSize { mean_delta, ci_lo, ci_hi, cohen_d, d_z, relative_diff,
  percent_improvement, ci_method }` from `effect_sizes(control, treatment,
  paired, conf, seed)`. `paired_t_pvalue` / `welch_pvalue` for p-values().
  `arm_stat` for per-arm descriptive statistics (N, mean, SD, median, CI).
  `effect_size_for` convenience constructor().
- `hierarchy.rs` —: `MetricObservation { condition_id, replicate_index,
  metric, value: MetricValue }` with `MetricValue::Scalar/Missing/NotScalarizable`;
  `ReplicationScalar` type barrier (private field, no `From<f64>`);
  `per_replication::from_metric` / `from_distribution` documented aggregation
  steps; `replication_scalars` canonical per-replication extractor.
- `aggregation.rs` —: `ReplicationObservation` with provenance +
  `scalar: Option<ReplicationScalar>`; `ReplicationTable = BTreeMap<(String,
  String), Vec<ReplicationObservation>>`; `build_replication_table(&StudyResult)`
  canonical extraction path; `replication_scalar(&MetricResult)` extends
  extract_scalar to include Distribution→mean-of-sample.
- `estimand.rs` —: `Estimand` enum (`AbsoluteDifference`,
  `RelativeDifference`, `PercentImprovement`, `MeanPairedDifference`,
  `MedianPairedDifference`) each carrying `EstimandDef { outcome, treatment,
  control }`; `estimate_point` / `effect_for` with bootstrap CIs on the
  estimand-specific functional.
- `cohorts.rs` — spec §14.3 +: `CohortResult` and `analyze_cohort` for
  per-player slicing; `HeterogeneityResult` + `analyze_heterogeneity` for
  per-cohort replication-level effects().
- `comparator.rs` — spec §14.6: `Comparator` with side-by-side metrics and
  utility ranking.
- `report.rs` — spec §14.4: `generate_report` (single Markdown) and
  `generate_comparison_report` (Markdown/JSON).
- `study.rs` — spec §14.8 (    ): `StudyStats` with
  `design: DesignType` + `ci_method` on `ArmMetricStat`/`PairEffect`;
  `compute_study_stats` consumes table; `ArmMetricStat`/`PairEffect` carry
  `ci_method`; `generate_study_report` (v0.2, byte-identical);
  `generate_research_report` ( v2: sectioned Markdown with Study/Design/
  Conditions/Primary Outcomes/Effect Estimates/Reproducibility; descriptive/
  inferential/causal labeling; "No causal claim is made" sentinel).
- `export.rs` — spec §14.5: `RawDataExporter` + `write_result_json`.
- `result.rs` —  : `StatisticalResult { estimand, estimate,
  uncertainty: Uncertainty, effect_size: EffectSizeSummary, sample: SampleSummary,
  family, p_value, adjusted_p, provenance: Provenance }` serde
  `Serialize`/`Deserialize`; `Provenance { study_id, config_hash, git_commit,
  engine_version, condition_ids, metric, design, aggregation: AggregationKind,
  method, confidence, bootstrap_iterations, seed, n_replications }`.
- `multiple_comparisons.rs` —: `Correction` enum (`Holm`, `BenjaminiHochberg`);
  `holm(ps)` (FWER) and `benjamini_hochberg(ps)` (FDR) corrections.
- `power.rs` —: `PowerSpec { alpha, target_power, minimum_effect }`;
  `required_replications(effect_sd, spec)` and `achieved_power(effect_sd, n,
  effect, alpha)` using normal approximation.
- `factors.rs` —: `FactorLevel { factor, level }`, `FactorDesign =
  BTreeMap<String, Vec<FactorLevel>>`, `validate_factor_design`,
  `main_effects(factor, design, arm_scalars, paired, conf, seed)`.
- `robustness.rs` —: `RobustnessCheck { name, passed, primary_delta,
  alternative_delta, category: RobustnessCategory }`, `RobustnessReport`,
  `RobustnessSpec` (MeanVsMedian, CiMethod, PairedVsIndependent),
  `run_robustness(control, treatment, specs, conf, seed)`.
- `power.rs` extensions(): `StudyPlan`, `PlanAssumptions`,
  `AllocationStrategy`, `plan_study(assumptions)` with Bonferroni correction
  for multi-condition studies.
- `factors.rs` extensions(): `EffectTerm` (Main/Interaction),
  `EffectStructure` with `validate(factor_names)` for estimability checks.
- `incremental.rs` —: `IncrementalAggregator` trait, `WelfordMean`,
  `WelfordVariance`, `CountAggregator`, `AggregatedValue` enum,
  `AggregationStrategy` (Incremental/Full) for streaming aggregation.
- `stability.rs` —: `StabilityMetrics`, `PopulationSnapshot`,
  `StabilityVerdict` (Stable/Oscillating/Diverging/Transient),
  `compute_stability(metrics)` for ecosystem analysis.
- `query.rs` —: `AnalysisQuery`, `QueryResult`, `DataValue` for unified
  experiment data retrieval.
- `dataframe.rs` —: `ResearchDataFrame`, `Column`, `DataType`, `DataValue`,
  `DataProvenance` for stable research data interchange.
- `api.rs` —: `AnalysisAPI` connecting statistical analyses to study
  estimands, design, replication structure, and provenance.
- `comparison.rs` —: `ComparisonEngine`, `ConditionComparison`,
  `LevelComparison` for first-class comparison operations.
- `visualization.rs` —: `PlotSpec`, `Mark`, `PlotRenderer`, `TextRenderer`,
  `JsonRenderer`, convenience functions (plot_distribution, plot_comparison,
  plot_timeseries, plot_tradeoff, plot_interaction).
- `pareto_explorer.rs` —: `ParetoExplorer`, `TradeoffSummary`,
  `FrontierComparison` for Pareto analysis.
- `navigator.rs` —: `StudyNavigator`, `NavigationView` for interactive
  study exploration.
- `report_v2.rs` —: `ResearchReport`, `ReportProvenance`,
  `generate_research_report`, `render_markdown` for automated research reports.
  writing `matches.json`/`observations.json`. `write_result_json(&result,
  &dir)` writes the metrics JSON under `OutputSpec.directory`.
  `RawDataExporter` is a standalone utility in v0.1 — per-match loop wiring is
  left to a later ticket.
- Determined output: `ExperimentResult.metrics` is a `BTreeMap` and the
  exporter sorts observations, so two runs with identical seed produce
  byte-identical files (the wall-clock `timestamp` field is the only thing
  that legitimately differs).
- `study.rs` — spec §14.8(): `StudyReportConfig { conf, seed }`
  (default conf 0.95), `compute_study_stats(&StudyResult, &config) ->
  StudyStats` (scalarizable-enabled metrics only, per-arm `ArmMetricStat`
  mean+CI and pairwise `PairEffect`s via `effect_sizes`; bootstrap seeds derive
  per (metric, arm/pair)), `generate_study_report` — the Markdown
  exit-criterion block (`| arm | mean | 95% CI |` table + `Δ = ` / `95% CI [..]` /
  `Cohen's d = ` / `relative Δ = ` lines) with a providence header (config hash,
  git commit, engine version), `generate_study_report_json` (the serialized
  `StudyStats` aggregate), and `write_study_result_json` writing
  `<study_id>.json` + `study_stats.json` (stats byte-identical across reruns;
  study JSON structural-equal net of the wall-clock timestamp).

**`matchlab-ranking` is implemented with the Lua-native rank mapper + leaderboard:**
- `ranker.rs` — `RankMapper` trait (spec §10.1): `rating_to_rank(rating) ->
  Rank` and `rank_to_rating_range(rank) -> (f64, f64)`, `Rank { tier,
  division }` (serde `Deserialize`).
- `lua.rs` — `LuaRankMapper`: implements `RankMapper` by delegating to a
  script's `rating_to_rank` / `rank_to_rating_range`; the bracket table is
  `config.brackets`.
- `plugins/ranking/brackets.lua` — `rating_to_rank` finds the first bracket
  where `min <= rating < max`; ratings outside all brackets clamp to the
  **last** bracket (the spec's reference behavior, not the first);
  `rank_to_rating_range` returns `{0,0}` for unknown ranks.
- `leaderboard.rs` — `Leaderboard` (spec §10.2) with `update(player_id,
  rating, rank, games_played)` (insert-or-replace, then re-sort by rating
  descending), `rank_of(player_id) -> Option<usize>`, `top_n(n) -> &[LeaderboardEntry]`
  (clamps when `n > len`), `entries()`/`len()`/`is_empty()`. `LeaderboardEntry {
  player_id, rating, rank, games_played }`.
- `lib.rs` — re-exports `Leaderboard`, `LeaderboardEntry`, `LuaRankMapper`,
  `Rank`, `RankMapper`.

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
  `MaintainLowRating`, `WinTrade { partner }`, `Derate`). adds
  `StrategicAgent` trait with `observe`/`select_action`/`objective`/`update`,
  `AgentObservations`, `AgentAction` enum, `AgentOutcome`, `PlayerObjective`
  trait, and `OrdinaryPlayer` (trivial policy).
- `objectives.rs` —: `PlayerObjective` implementations: `WinRateObjective`,
  `RatingGainObjective`, `RatingVarianceObjective`, `QueueTimeObjective`,
  `MatchQualityObjective`, `TimeSpentObjective`, `LatencyObjective`,
  `CompositeObjective` (weighted combination).
- `policy.rs` —: `AdaptivePolicy` trait with `decide`; built-ins:
  `ThresholdQueuePolicy`, `LossAversionPolicy`, `RegionSwitchPolicy`,
  `PartyFormationPolicy`, `PatternDetectionPolicy`.
- `manipulation.rs` —: `ManipulationStrategy` trait; built-ins:
  `RatingDumpStrategy`, `QueueGamingStrategy`, `PartyExploitStrategy`,
  `RegionHoppingStrategy`, `InformationInferenceStrategy`.
- `lua.rs` — `LuaAdversarialAgent`: implements `AdversarialAgent` by
  delegating to a script's `tick` / `objective` functions. The adapter exposes
  a `behavior` table (quit_probability, party_id, tilt_level, win_rate,
  is_online) plus the player's observation, and writes the returned behavior
  back to reality/observations. Randomness flows through `matchlab.rng_*` from
  the behavior stream RNG passed to `tick` (the trait takes
  `tick(&mut self, player_id, rng: &mut SimRng, world)`); the objective is read
  at load and cached.
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

**`matchlab-utility` is implemented with the Lua-native satisfaction model:**
- `satisfaction.rs` — `SatisfactionModel` trait (spec §16.1):
  `satisfaction(&PlayerExperience)`, `retention_probability(f64)`,
  `rematch_probability(f64)`; and `PlayerExperience` (recent match qualities,
  queue times, outcomes, `current_streak`, `rank_change`, `perceived_fairness`,
  `rematch_rate`; `new()`/`record_match()` helpers).
- `lua.rs` — `LuaSatisfactionModel`: implements the trait by delegating to a
  script's `satisfaction` / `retention_probability` / `rematch_probability`;
  the weights live in the script's config.
- `plugins/utility/satisfaction.lua` — the weighted sum (loss-streak penalty
  only kicks in below −3), `retention_probability()` is the logistic
  `1/(1+e^−s)`, and `rematch_probability()` requires a higher threshold
  (`1/(1+e^−0.5(s−2))`).
- `lib.rs` — re-exports `LuaSatisfactionModel`, `SatisfactionModel`,
  `PlayerExperience`.

**`matchlab-validation` is the analytical-baseline test crate (ticket ):**
- `src/lib.rs` — test-side helpers only: `logistic_win_probability(diff, beta)`
  (the outcome model's natural-scale reference),
  `interleaved_two_class_population(total, skill_high, skill_low, initial_rating,
  seed)` (two skill classes re-numbered so adjacent ids alternate class, which
  makes the batch matchmaker form cross-class matches),
  `single_class_config`/`two_class_config` (serde-built `ExperimentConfig`
  manifests), `run_loop(...) -> LoopOutcome` (drives a `MatchLoop` directly
  with elo + logistic(noise 0) + batch and returns finalized metrics +
  completion stats), and `build_loop(population, teams, max_matches, seed,
  metrics)` (same stack with an explicit `TeamComposition` — XvY sizes +
  optional roles; `run_loop` is `build_loop` with an equal-size role-less
  composition). `pub mod reference` exposes the reference math.
- `tests/elo.rs` — Elo baselines: (1) with a two class population, the
  observed high-class win rate must match the logistic ground truth
  `1/(1+exp(-500/400)) ≈ 0.7773` within 6σ; (2) on a homogeneous `N(1000,250)`
  cold-start population the `rating_accuracy_by_time` series must drop below
  87% of its first bucket (observed 198.5 → 166.2); (3) same-seed runs are
  byte-identical. Nothing in this crate is wired into the loop — these are
  test-side reference implementations, and a disagreement is a bug in the Lua
  script, never a reason to patch the tests.
- `src/reference/` (ticket ) — independent math implementations used to
  anchor posterior-based rating systems:
  - `glicko2.rs` — Glickman 2012 (§5, eqs 1–10): `Opponent { mu, phi, sigma,
    outcome }`, `single_period(...) -> PeriodResult`, `scale`/`unscale`
    (factor 173.7178, center 1500), `idle_step` (periods without games grow
    RD at `σ` and leave `μ`/`σ` unchanged). The Newton volatility iteration is
    bracketed identically to `glicko2.lua`; verified against the paper's
    worked example (r' = 1464.06, RD' = 151.52, σ' = 0.05999).
  - `trueskill.rs` — TrueSkill 1v1 truncated-Gaussian conditioning
    `update_head_to_head(...)` mirroring `trueskill.lua`, including the
    script's `w = v*(v - alpha)` convention for the winner's uncertainty
    factor and the `u = Φ⁻¹((1+p)/2)` probabilistic draw margin. Also
    `draw_update_equal_players` — a reference-only true-draw posterior
    (equal ratings keep μ, shrink σ) documenting what a future draw-capable
    game path must hit.
- `tests/glicko.rs` (ticket ) — Glicko-2 baselines: (1) the paper's
  worked example driven as three sequential 1v1 games, each step checked
  script-vs-reference at 1e-6 relative (final values reproduce the paper's
  1464.06/151.52/0.05999 within the loose legacy bound — serialized single-game
  periods differ from one multi-opponent period by ~0.27 rating points, so the
  tight assertion is per-step); (2) a two-period chain whose idle first period,
  passed to the script, grows RD by exactly `volatility*172800`; (3) an
  eight-opponent period whose outcome distribution is informative enough to
  stay on the Newton loop and keep all three outputs stable; (4) a negative
  control — a deliberately perturbed reference (crude `epsilon`, flipping the
  volatility iteration's safeguard) must diverge from both script and correct
  reference by more than 1e-5 relative, proving the comparison has teeth.
- `tests/trueskill.rs` (ticket ) — TrueSkill baselines: (1) a 1v1 win and
  a 1v1 loss (script-vs-`update_head_to_head` at 1e-6 relative) for the
  symmetric case, plus an asymmetric 1500-vs-1000 game preserving ordering and
  widening the gap; (2) `draw_probability > 0` engages the margin math (u>0
  makes the win update larger — both must diverge from the no-margin values and
  agree with the reference); (3) the reference-only draw-posterior shrinkage.
  Both test files drive the script through a `match_result(vec![player], ids,
  winner)` with the solo player always on team A, so `winner == Team::B` means
  the player lost.
- `tests/matchmaking.rs` (ticket ) — match-quality baselines through the
  `LuaMatchmaker` + `Queue`: uniform 1200 population forms a match with
  quality exactly `1.0`; directly-separated all-1000 vs all-1400 teams clamp at
  exactly `0.0` (and never go negative past the clamp); a fixed multiset
  `[1000..1250]` reproduces the alternate-assignment analytic quality `0.875`
  (with a negative control proving the uniform-average value `1.0` is
  rejected); and a truth-separation guard proves quality tracks the visible
  rating, never the ground-truth `skill_vector`.
- `tests/queue.rs` (ticket ) — wait-time baselines: exact-tick saturated
  waits (`now − joined_at` in `ticks()`), the saturating boundary when
  `now < joined_at` (reverting `duration_since` to `wrapping_sub` makes the
  suite fail — asserted locally), and a fixed-interval arrival tape under
  batch 1v1 producing analytic per-match waits (exactly `Δt` for
  every formed match, pairing oldest-unmatched players).
- `tests/dynamic_skill.rs` (ticket ) — dynamic-skill baselines: with
  `skill_update_interval_secs` set, improvers sit at exactly `S0 + k·t` after
  `t` seconds and stables never drift (exact trajectory); over two days of sim
  time improver ratings rise above `S0` while stable ratings fall (Elo responds
  to the drift), and `lag = skill − rating` grows — a measured result
  documented in spec §5.6, since matches resolve too sparsely to chase a
  continuous target; same-seed determinism leaves skills + ratings
  byte-identical; and the negative control proves *no* interval flag ⇒ no
  drift even with `improvement_rate` set.
- `tests/xvy.rs` (ticket ) — XvY 1v4 (DbD) role baselines: uniform
  killers+survivors (1000) form two matches under the role-aware batch
  path with quality exactly `1.0`, and a known-gap population (1500 killers vs
  1400 survivors) produces exactly `1 − 100/400 = 0.75` per match — matching
  on *unequal* team sizes is the analytic formula, proving the quality measure
  survives the team-size asymmetry; a `RoleCompositionGuard` metric collector
  registers on a live loop run and asserts *every* formed match is exactly
  one killer + four survivors (the role path, never the counts-only fallback);
  a killer-only population stalls (0 formed/completed, queue grows — role
  gating is what produces scarcity); and the full `dbd_1v4.yaml` experiment
  is deterministic across two same-seed runs (`build_loop` drives the loop
  with an explicit `TeamComposition`).
- `tests/invariants.rs` (ticket ) — invariants as deterministic property
  tests over a seeds × config-shape grid (single-class, two-class, 1v4
  role-gated, expanding-window, strict): test-only guards verify every formed
  match's team sizes/roles, no NaN/±inf in any finalized metric or `RatingState`
  (a live `SanityRatingSystem` wrapper refuses non-finite `update` output), no
  player formed before joining the queue (raw ticks — `duration_since` saturates),
  `win_rate` ∈ [0,1], single-match membership (interval guard + a mid-run scan
  of `active_matches`), population-conserved counts, and same-seed
  byte-identical metrics/ratings across the grid. Guard teeth are proven by
  negative controls: zero-size compositions refused at construction, and role
  leaks, win_rate 2.0, join-after-formation, overlapping intervals, and NaN
  inputs all fire.
- `tests/metamorphic.rs` (ticket ) — three metamorphic relationships
  anchored to analytic constants and measured values rather than goldens:
  (1) `zero_noise_favors_true_skill_and_converges` — outcome `noise → 0` lowers
  `rating_accuracy` MAE vs a `noise: 0.2` twin (zero-mean noise shifts *which*
  matches upset, so the aggregate win rate barely moves — MAE is the honest
  signal), and a decisive 2400-vs-500 skill gap at zero noise favors the strong
  class ≥ 0.97 (the outcome model's 0.99 probability clamp, sampled over enough
  matches for the deterministic Bernoulli rate to land near it); (2)
  `double_population_is_scale_invariant` — doubling the single-class population
  with the same seed keeps the normalized MAE within ±0.10 and doubles
  `matches_completed` (ratio ∈ [1.7, 2.3]); (3)
  `widening_window_monotonically_improves_queue_and_quality` — a nested
  strict{25} → strict{150} → batch ladder on an identical population/seed shows
  mean `queue_time` non-increasing as the window widens while `match_quality`
  stays on the balanced plateau (cold-start identical ratings make the tightest
  window ~1.0, so the claim is batch/wide never degrade past a 0.01 band, not
  strict monotonicity). Note: expanding_window is *not* used as the wide rung —
  its initial 25-diff tier is tighter than strict's fixed 75, inverting
  queue-time monotonicity; a same-script knob ladder keeps the comparison sound.
  `crates/matchlab-validation/src/lib.rs` gained `elo_params`,
  `build_loop_full`/`run_loop_full` (the extra args are script paths + YAML
  param strings; the old `build_loop`/`run_loop` delegate with identical params,
  byte-identical behavior).
- `plugins/_test/` (ticket ) — **test-only spy scripts** (never referenced
  by a manifest in `experiments/`) that error loudly if the simulation hands a
  layer data outside its budget, plus `tests/info_budget.rs` proving the budget
  contract is enforced end-to-end: `spy_rating.lua` mirrors `elo.lua` whose
  `update` refuses any run whose sanitized result still carries non-zero scores/
  duration, non-empty performances, or ground-truth skill keys (a passing
  loop-level run proves `filter_match_result` + `into_match_result` are wired
  into `handle_match_end`); `spy_matchmaker.lua` errors if any queue entry
  carries `skill_vector`/`skill_overall`/`hidden_mmr`/`true_skill` (queue
  snapshots are observations-only); `spy_collector.lua` is the positive control
  — metrics legitimately see reality, so it errors *unless*
  `true_skill`/`skill_overall`/`skill_vector` are present. A `#[should_panic]`
  negative control calling `update` on an *unfiltered* result proves the spies
  are live.
- `tests/replication.rs` (ticket ) — the research-pipeline baselines.
  Study determinism (byte-identical `study_stats.json` + structural
  `<study_id>.json` net of timestamps); CRN pairing honesty (identical arms
  share the replicate seed, run byte-identically, mean delta provably zero
  with CI containing 0 — and the *independent* negative control diverges);
  counterfactual honesty (replay through the same system reproduces the live
  arm's per-player ratings bit-for-bit, aggregates agree to 1e-9 relative,
  and the recording run is byte-identical to a plain run of the same seed);
  distinct systems (elo vs glicko2 CRN → disjoint `rating_accuracy`
  distributions, effect CI excludes 0); the per-stream guarantee (varying
  only the `behavior` seed leaves a run byte-identical without adversarial/
  satisfaction); and the CI-width contract (paired CI at 96 replicates is
  narrower than at 24). These tests are what caught the replay
  start-state bug (see the `counterfactual.rs` bullet).
- The outcome scripts (logistic, variance, momentum, fatigue) guard `noise ==
  0.0` by skipping the empty-range `rng_range(-noise, noise)` draw (deterministic
  outcomes when configured; RNG behavior is unchanged for `noise > 0`, so
  v0_1_basic byte-identity is preserved).

The twelve-step v0.1 build order is complete **and the v0.2 …
"experimental rigor" plan is complete** ( invariants, metamorphic,
 info-budget spies, multi-stream RNG, replication, study
manifests, counterfactual history/replay, aggregate statistics,
 study report CLI, replication-pipeline validation + acceptance demo).
The recorded v0.2 acceptance numbers live in `docs/spec.md` §18.8. The
v0.1 acceptance numbers older commit notes may carry are superseded — see
spec §18 for the current baselines.

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
11. **Acceptance** — `cargo run -- run experiments/v0_1_basic.yaml` produces metrics JSON, Elo MAE decreases (196.4 → 162.3), match quality mean 0.98, queue time 5.01s, all tests pass, deterministic.

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
- **Plugin model (Lua-native):** every algorithm is a Lua script under
  `plugins/<layer>/` implementing a per-layer contract (see `docs/spec.md` §3.3
  and each crate's `lua.rs`). Rust holds the types, the event loop, and thin
  trait adapters (`*::lua::Lua*System`); there are **no inherent Rust
  algorithms**. Manifests reference systems by `script:` path. Scripts receive
  `config` (YAML params) + a persistent `context` table (passed by reference,
  stored in the VM) and may draw deterministically via `matchlab.rng_*`.
- **Lua scripts are pure.** No `math.random` — all randomness comes from `SimRng`
  via `matchlab.rng_*`. Scripts receive only observable data, never
  `PlayerReality` (the outcome model and metric scripts get the ground-truth
  skill binding / reality fields — metrics are the legitimate reality reader).
- **Adding a system** = writing one `.lua` file implementing the layer's
  contract and referencing it by path (or by `name:` for built-in rating
  systems, which maps to a script). See `plugins/rating/decay_elo.lua` and
  `plugins/metrics/avg_rating_gap.lua` for examples with no Rust equivalent.

---

## Where to Find Things

| Need | Look at |
|------|---------|
| Core types (PlayerReality, World, etc.) | `crates/matchlab-core/src/` |
| How events flow | `crates/matchlab-core/src/event.rs`, `src/lib.rs` (Simulation) |
| Rating algorithm scripts | `plugins/rating/` (elo, flat, glicko2, trueskill, decay_elo) |
| Matchmaker scripts | `plugins/matchmaking/` (batch, expanding_window, strict, hub_spoke) |
| Metric collector scripts | `plugins/metrics/` (one per metric, incl. custom) |
| Lua system contracts + adapter | `crates/matchlab-{trait}/src/lua.rs`, `crates/matchlab-lua/src/` |
| How to add a system | Write a `.lua` file in `plugins/`, reference via `script:` in YAML |
| Experiment YAML schema | `docs/spec.md` section 13.1 |
| Build plan | `docs/spec.md` §17 build order |
| Adversarial agents (booster, deranker, etc.) | `crates/matchlab-adversarial/src/` |
| Player satisfaction model | `crates/matchlab-utility/src/satisfaction.rs` |
