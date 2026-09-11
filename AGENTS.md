# matchlab — Agent Orientation

## What This Project Is

matchlab is a **discrete-event simulation framework** written in Rust (edition 2024) for evaluating competitive matchmaking and rating systems. It generates synthetic player populations with known ground truth, runs them through a simulated matchmaking ecosystem, and measures algorithm performance with real metrics.

It answers questions like: *Under what conditions does Elo outperform Glicko-2?* and *How much match quality must be sacrificed to reduce queue time by 50%?*

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
matchlab/               # workspace root
├── Cargo.toml          # workspace root (NOT a library crate)
├── src/main.rs         # CLI binary: `matchlab run <manifest>`
└── crates/
    ├── matchlab-core/          # simulation engine, time, events, world, RNG, core types
    ├── matchlab-lua/           # Lua-native system foundation (VM, context, rng, validation)
    ├── matchlab-players/       # archetypes, population generation, skill process
    ├── matchlab-game/          # outcome models, match execution
    ├── matchlab-matchmaking/   # queue, matchmaker, constraints, search strategies
    ├── matchlab-rating/        # rating systems (Elo, Glicko-2, TrueSkill, Flat, Bradley-Terry, Thurstone, Massey, Colley, WHR, OpenSkill, Rank Centrality, PageRank, Bayesian Logistic, Bayesian Hierarchical)
    ├── matchlab-detection/     # smurf detection, interventions
    ├── matchlab-ranking/       # rank mapping, leaderboard
    ├── matchlab-loop/          # simulation loop, event handlers, machine state
    ├── matchlab-metrics/       # metric collectors (accuracy, quality, queue time, etc.)
    ├── matchlab-objective/     # weighted utility, multi-objective scoring
    ├── matchlab-adversarial/   # adversarial player agents (boosters, derankers, etc.)
    ├── matchlab-utility/       # player satisfaction / retention model
    ├── matchlab-experiments/   # runner, YAML config, factorial design, counterfactual eval, replication
    ├── matchlab-analysis/      # statistics, Pareto, cohorts, reports, provenance
    ├── matchlab-validation/    # analytical-baseline regression tests (test-side references)
    └── matchlab-optimize/      # Bayesian hyperparameter optimization (GP, EI, ParEGO)
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
matchlab-optimize      (depends on core + experiments + metrics + ndarray)
matchlab (binary)      (depends on experiments + analysis + optimize)
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

The workspace is fully implemented: 16 crates under `crates/`, a binary at `src/main.rs`. `cargo build --workspace`, `cargo test --workspace`, and `cargo check --workspace` all pass.

- `[workspace.dependencies]` declares `serde` (derive), `serde_yaml 0.9`, `rand 0.8`, `rand_chacha 0.3`, `mlua 0.10` (luau, vendored); `[workspace.package]` sets `edition = "2024"`.
- `src/main.rs` is the `matchlab` binary (`matchlab run`, `matchlab study`, `matchlab compare`, `matchlab optimize`); depends on `matchlab-experiments`, `matchlab-analysis`, and `matchlab-optimize`.
- All algorithms are Lua scripts under `plugins/`. Rust holds types, traits, and thin `Lua*System` adapters — there are no inherent Rust algorithms.
- The v0.1 build order (steps 1–12) and v0.2 experimental rigor features are complete.

### Key subsystems

| Crate | Role |
|-------|------|
| `matchlab-core` | SimTime, PlayerId, MatchId, SimRng, SkillVector, PlayerReality, PlayerObservation, MatchResult, World, EventEngine, Simulation |
| `matchlab-lua` | Lua VM, context threading, deterministic RNG routing, script validation, core↔Lua marshalling |
| `matchlab-players` | Archetypes, population generation, skill dynamics, population dynamics |
| `matchlab-game` | OutcomeModel trait, Lua outcome scripts (logistic, variance, composition, performance, fatigue, momentum) |
| `matchlab-rating` | RatingSystem trait, information budget filtering, Lua rating scripts (elo, flat, glicko2, trueskill) |
| `matchlab-matchmaking` | Queue, Matchmaker trait, constraints, search strategies, Lua matchmaker scripts (batch, expanding_window, strict, hub_spoke, random) |
| `matchlab-detection` | DetectionSystem trait, Lua detection scripts (smurf) |
| `matchlab-ranking` | RankMapper trait, Leaderboard, Lua rank scripts (brackets) |
| `matchlab-loop` | Event handlers, MachineState, MatchLoop, GameHistory (counterfactual recording) |
| `matchlab-metrics` | MetricCollector trait, MetricsEngine, 13 Lua metric scripts |
| `matchlab-objective` | ObjectiveWeights, weighted utility scoring |
| `matchlab-adversarial` | AdversarialAgent trait, Lua agent scripts (afk, deranker, win_trader, booster, rating_farmer) |
| `matchlab-utility` | SatisfactionModel trait, Lua satisfaction script |
| `matchlab-experiments` | YAML config, inheritance, factorial design, replication, counterfactual replay, study manifests |
| `matchlab-analysis` | Statistics (CIs, effect sizes, power), Pareto, cohorts, reporting, provenance |
| `matchlab-validation` | Analytical-baseline regression tests (Elo, Glicko-2, TrueSkill, matchmaking, invariants, metamorphic, info-budget) |
| `matchlab-optimize` | Bayesian hyperparameter optimization: GP surrogate, Matern 5/2 kernel, EI/EHVI acquisition, ParEGO multi-objective, Latin Hypercube sampling |

### CLI commands

- `matchlab run <manifest.yaml>` — run a single experiment
- `matchlab study <study.yaml> [--replicates N] [--json]` — run a multi-arm replicated study
- `matchlab compare <result.json>... [--json]` — compare exported results
- `matchlab optimize <optimize.yaml>` — Bayesian hyperparameter optimization

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
11. **Acceptance** — `cargo run -- run experiments/v0_1_basic.yaml` produces metrics JSON, Elo MAE decreases, match quality mean 0.98, queue time 5.01s, all tests pass, deterministic.

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

The minimal v0.1 manifest is at `experiments/v0_1_basic.yaml`.

---

## Conventions

- **Rust edition:** 2024
- **Shared deps** (workspace): `serde` (with derive), `serde_yaml 0.9`, `rand 0.8`, `rand_chacha 0.3`, `mlua 0.10` (luau, vendored)
- **No comments in code** unless explicitly requested
- **Unit tests** live in `#[cfg(test)] mod tests` blocks within each source file
- **Crate naming:** `matchlab-{domain}` (e.g., `matchlab-core`, `matchlab-rating`)
- **File naming:** `match_.rs` (not `match.rs`, which is a Rust keyword)
- **Plugin model (Lua-native):** every algorithm is a Lua script under
  `plugins/<layer>/` implementing a per-layer contract (see each crate's
  `lua.rs`). Rust holds the types, the event loop, and thin
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
| Adversarial agents (booster, deranker, etc.) | `crates/matchlab-adversarial/src/` |
| Player satisfaction model | `crates/matchlab-utility/src/satisfaction.rs` |
