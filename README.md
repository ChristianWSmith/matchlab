# matchlab

A **discrete-event simulation framework** for evaluating competitive
matchmaking and rating systems. matchlab generates synthetic player populations
with known ground truth, runs them through a simulated matchmaking ecosystem —
queues, matchmakers, rating systems, smurf detection, adversarial behavior, and
player retention — and measures what actually matters with real metrics.

It exists to answer questions like:

- *Under what conditions does Elo outperform Glicko-2?*
- *How much match quality must be traded to cut average queue time in half?*
- *How do rating systems track players with changing skill?*
- *How much damage do smurfs and boosters do, and does detection actually help?*
- *Does optimizing individual skill produce the same match quality as optimizing team composition?*
- *Which rating systems remain accurate as outcome noise increases?*

Because every player has a **known true skill**, matchlab can compare what a
rating system *believes* against what is *actually true* — something that is
impossible to measure on real production data, where ground truth is never
observable.

---

## Why matchlab?

### Biology, not chemistry

On a real ladder, a match is a single data point. It tells you how the two teams
were matched and who won, but the players' true skills are unknown. Any
conclusion you draw is confounded by exactly the thing you are trying to study.

matchlab inverts that. The simulation **knows** every player's true skill, so:

- **rating_accuracy** is measured as the actual distance between ratings and
  ground truth — a real mean absolute error, not a proxy.
- The **same population** can be replayed through different rating systems,
  matchmakers, or outcome models while every other variable stays fixed. Any
  difference in the metrics is caused by the change.
- **Counterfactual evaluation** replays the identical match history through
  multiple rating systems to isolate their relative performance.

### Everything is swappable, nothing is recompiled

Every algorithm in the simulation is a small **Lua script** under `plugins/`.
You can swap Elo for Glicko-2, batch matchmaking for an expanding skill window,
or the logistic outcome model for a fatigue model with a **one-line change** in
an experiment manifest. There are no Rust algorithms to learn or rebuild — a
genuinely new system is a single `.lua` file.

Rust handles the heavy lifting (the simulation engine, time, events, worlds,
statistics) and calls into Lua where decisions are made. Scripts are pure and
deterministic: they receive a config table and a persistent state table, and all
randomness comes from the simulation's seeded RNG (via `matchlab.rng_*`
helpers). The same seed always reproduces the same experiment, byte for byte.

### Deterministic and reproducible

Every experiment is fully reproducible given its config and seed:

- A single seed derives separate sub-seeds for population generation, match
  outcomes, player arrivals, and behavior.
- Each `ExperimentResult` records a **config hash** (which includes the contents
  of every referenced Lua script — editing a script changes the experiment's
  identity) and the **git commit** it was produced from.
- The queue-time, match-quality, and rating-accuracy measurements are
  deterministic; two runs with the same seed produce identical results except
  for the wall-clock timestamp.

---

## Quick start

### Requirements

- Linux, macOS, or Windows with Rust 2024 edition toolchain.
  Use <https://rustup.rs> if you do not have it.

### Build and test

```bash
cargo build
cargo test
```

### Run your first experiment

```bash
cargo run -- run experiments/v0_1_basic.yaml
```

This runs the v0.1 baseline: 10,000 players with flat (static) skill, 5v5
matches decided by a logistic outcome model, rating-balanced batch
matchmaking, and Elo, for one simulated week (capped at 1M matches). It writes
`results/v0_1_basic.json`.

Key numbers from the baseline run:

| Metric | Value | What it means |
|--------|-------|---------------|
| rating_accuracy mean | 167.65 | Mean absolute error between Elo ratings and true skill |
| rating_accuracy (start) | 196.61 | Elo starts cold (all ratings at 1000) |
| rating_accuracy (end) | 158.49 | Elo learns: error decreases over the week |
| match_quality mean | 0.981 | Teams are near-even on average |
| queue_time mean | 5.02 s | Average real wait from join to formation |

### Getting started examples

The `experiments/examples/` directory contains three small manifests designed
for new users:

| Manifest | What it demonstrates |
|----------|----------------------|
| `examples/hello_matchlab.yaml` | Minimal 100-player experiment — the smallest possible run |
| `examples/rating_comparison.yaml` | 2-arm CRN study comparing Elo vs Glicko-2 |
| `examples/matchmaking_tradeoff.yaml` | 2-arm CRN study comparing batch vs expanding-window matchmakers |

### CLI

The binary provides several commands:

```bash
matchlab run <manifest.yaml>           # Run a single experiment
matchlab study <study.yaml>            # Run a replicated multi-arm study
matchlab compare <result.json>...      # Compare experiment results side-by-side
matchlab package <manifest.yaml>       # Create a reproduction package
matchlab analyze <result.json>         # Analyze a stored result
matchlab compare-stats <study.json>... # Compare study results statistically
matchlab power                         # Compute power analysis
```

CLI flags for controlling output:

| Flag | Purpose |
|------|---------|
| `--verbose` | Enable debug-level logging |
| `--log-level <LEVEL>` | Set log level (trace, debug, info, warn, error) |
| `--log-file <PATH>` | Write logs to a file in addition to stdout |
| `--json-logs` | Output logs as JSON Lines (for tooling) |
| `--json` | Print comparison output as structured JSON |

---

## The experiment catalog

The repo ships the following experiment manifests:

### Baseline and comparison experiments

| Manifest | What it demonstrates |
|----------|----------------------|
| `v0_1_basic.yaml` | Minimal baseline: Elo + logistic + batch on a flat population |
| `glicko_comparison.yaml` | Glicko-2 on the standard population |
| `matchmaker_comparison.yaml` | Expanding-window matchmaking |
| `detection_test.yaml` | Smurf detection enabled |
| `full_featured.yaml` | Everything on: fatigue, detection, ranks, adversarial agents, satisfaction, all metrics, objectives |
| `dbd_1v4.yaml` | Dead-by-Daylight-style 1v4 asymmetric: role-gated killer vs survivors |
| `novel_rating.yaml` | A rating system with *no Rust equivalent* (`decay_elo.lua`) and a custom metric (`avg_rating_gap.lua`) |
| `golden_study.yaml` | Comprehensive end-to-end study exercising all subsystems |

### Getting started

| Manifest | What it demonstrates |
|----------|----------------------|
| `examples/hello_matchlab.yaml` | Minimal 100-player experiment |
| `examples/rating_comparison.yaml` | Elo vs Glicko-2 study |
| `examples/matchmaking_tradeoff.yaml` | Batch vs expanding-window study |

### Studies

| Manifest | What it demonstrates |
|----------|----------------------|
| `studies/elo_vs_glicko.yaml` | 2-arm CRN study with 50+ replicates |
| `studies/elo_vs_glicko_replay.yaml` | Counterfactual replay study |

### Feedback loop grid

| Directory | What it demonstrates |
|-----------|----------------------|
| `feedback_loop/` | 9-cell factorial grid: 3 rating systems × 3 matchmakers |

---

## How the simulation works

matchlab is a **discrete-event simulation**. Instead of evaluating the world at
every simulated tick, it processes events in timestamp order — players join,
queue, get matched, play, re-queue — and **skips idle time**. If the next event
is three days away, the clock jumps straight there. A 10,000-player week-long
experiment with ≈330,000 matches runs in well under a minute.

### The loop

1. **Population generation.** Players are drawn from archetypes (see below).
   Every player gets a *true skill*; the rating ladder starts cold (everyone
   begins at the same visible rating).
2. **Arrivals and queues.** Players join, wait in a queue, and are grouped into
   teams by the matchmaker.
3. **Match execution.** The outcome model (the "game") decides the winner using
   ground-truth skill with stochastic noise.
4. **Rating update.** Each rating system updates its players from the match
   result — and, crucially, **sees only the data its information budget
   permits.**
5. **Optional ecosystem layers.** Smurf detection evaluates behavior, rank
   mapping assigns visible ranks, adversarial agents misbehave on cue, and the
   satisfaction model decides who churns.
6. **Metrics.** Collectors accumulate raw evidence across every match and
   produce the summary statistics in the result file.

### Two views of every player

This is the core idea that makes matchlab's measurements trustworthy:

- **Ground truth** (`PlayerReality`): multidimensional latent skill,
  improvement rate, volatility, and behavior parameters. The simulation knows
  it; **algorithms never see it.**
- **Observable view** (`PlayerObservation`): rating, rating deviation, games
  played, queue wait, and everything else a real matchmaking service could
  know. Only the outcome model (the simulator of reality) and the metric
  collectors read ground-truth-derived values; rating systems, matchmakers, and
  detection work exclusively from observations.

The outcome pipeline separates latent skill from realized performance:

```text
True Skill (latent) → Performance Model → Realized Performance → Game Model → Outcome
```

A player can be highly skilled but perform poorly in a particular match. The
performance model introduces stochastic variation between what a player is
capable of and how they perform in a specific game.

That separation is what makes "Elo converges; MAE decreases" a real property of
the simulation. Match winners are decided by true skill while ratings are
updated from match results, so the ladder genuinely *learns*.

---

## The experiment manifest

Experiments are YAML files. A manifest describes the population, the systems to
test, the metrics to collect, and when to stop. The full structure:

```yaml
experiment:
  name: my_experiment
  description: "What I'm testing and why"
  seed: 42            # deterministic; change to sample new runs

  population:          # the synthetic player pool
    size: 10000
    archetypes: [...]  # see below

  game:                # how matches are decided
    teams: { a: 5, b: 5 }  # XvY supported: { a: { size: 1, role: killer }, b: { size: 4, role: survivor } }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05

  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0

  rating:
    systems:
      - script: plugins/rating/elo.lua
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0

  metrics:
    - match_quality
    - queue_time
    - rating_accuracy

  cohorts:             # slice results by player subpopulations
    - name: all
      filter: { type: all }

  duration:            # stop when either bound is hit
    matches: 1000000
    max_time: 604800.0 # simulated seconds (1 week)

  output:
    directory: results/
    formats: [json]
    report: true       # also write a Markdown report
```

For the complete schema with all fields, types, defaults, and units, see
[`docs/manifest-schema.md`](docs/manifest-schema.md).

### Config inheritance

The `base:` key lets one manifest inherit from another and override specific
fields. This is how controlled experiments are built — change **one variable**
and compare.

```yaml
base: base/standard.yaml

experiment:
  name: glicko_comparison        # everything from base/standard.yaml
                                 # is inherited except what you override:
  rating:
    systems:
      - script: plugins/rating/glicko2.lua
```

The shipped `experiments/base/standard.yaml` defines a mixed population of
2,000 players across five archetypes (stable, improving, declining, returning,
and smurf) with Elo, logistic outcomes, and batch matchmaking. Comparison
manifests inherit from it and override a single system.

---

## Building a population

Players are drawn from **archetypes** — named groups with a proportion, a skill
distribution, and behavioral parameters. Proportions are converted to exact
player counts that always sum to the population size.

```yaml
population:
  size: 10000
  seed: 42
  archetypes:
    - name: stable            # the bulk of the ladder
      proportion: 0.60
      skill_distribution: { type: normal, mean: 1000, stddev: 250 }
      skill_volatility: 5.0   # skill drift over time
      improvement_rate: 0.0   # 0 = static skill
      play_frequency: 0.8     # how often the player joins a session
      session_length: 1800.0  # seconds between matches
      quit_probability: 0.01  # chance to churn after a match

    - name: smurf             # high skill, low visible start
      proportion: 0.02
      skill_distribution: { type: normal, mean: 1500, stddev: 100 }
      improvement_rate: 0.0
      play_frequency: 0.95
      session_length: 3600.0
      quit_probability: 0.002
      initial_rating: 700     # cold ladder start → looks like a newcomer
      role: killer            # optional: gates team assignment in role-aware matchmakers
```

### Multidimensional skill (v0.5+)

Archetypes can define multiple skill dimensions with independent or correlated
distributions:

```yaml
- name: mechanical
  proportion: 0.20
  skill_distribution: { type: normal, mean: 1200, stddev: 200 }
  skill_dimensions:
    aim:
      distribution: { type: normal, mean: 1500, stddev: 200 }
    movement:
      distribution: { type: normal, mean: 1100, stddev: 150 }
  correlation:
    pairs:
      aim↔movement: 0.7
  dynamics:
    type: experience
    learning_rate: 0.005
    plateau_games: 500
```

Key mechanics:

- **`initial_rating`** overrides the *visible* rating the ladder starts at,
  while **true skill** stays sampled from the archetype. A large gap between
  them is the seed of the smurf problem — and of every convergence scenario.
  Leave it unset to start everyone's visible rating at their true skill.
- **Skill distributions** may be `normal`, `uniform`, or `log_normal`.
- **Multidimensional skill** uses `skill_dimensions` with per-dimension
  distributions and optional `correlation.pairs`. When present, the archetype
  produces `SkillVector` values with N dimensions instead of a single `overall`.
- **Skill dynamics** are configured via `dynamics:` (types: `linear`,
  `experience`, `decay`) or driven by `improvement_rate` and `skill_volatility`.
  With both at zero, skill is static — the population is fixed at generation
  (the v0.1 default). Turn them on to study environments where players get
  better, decline, or return.
- **Team models** in the game config control how individual skills aggregate:
  `additive` (sum, the default), `weighted` (per-dimension weights), or
  `complementary` (synergy bonus for diverse skill profiles).

---

## The system catalog

All systems ship as Lua scripts under `plugins/`. The table lists the config
parameters each script accepts (everything is optional — defaults are sane). For
the full plugin API contract, see [`docs/plugin-api.md`](docs/plugin-api.md).

### Rating systems (`plugins/rating/`)

| Script | What it does | Key config |
|--------|--------------|------------|
| `elo.lua` | Classic Elo on a logistic scale (`divisor = β·ln 10`) consistent with the game model | `k_factor`, `initial_rating`, `beta` |
| `glicko2.lua` | Full Glicko-2 (Glickman 2012): RD plus volatility, Newton–Raphson iteration. Verified against the paper's worked example (r′=1464.06, RD′=151.52, σ′=0.05999) | `initial_rating`, `initial_rd`, `initial_volatility`, `tau`, `epsilon` |
| `trueskill.lua` | TrueSkill (Herbrich, Minka, Graepel): each player is N(μ, σ²), truncated-Gaussian conditioning with draw margin | `initial_mean`, `initial_variance`, `beta`, `dynamics`, `draw_probability` |
| `flat.lua` | Fixed points for a win/loss — a baseline that shows why adaptive systems are needed | `win_points`, `loss_points`, `initial_rating` |
| `decay_elo.lua` | **Novel, no Rust equivalent.** Elo plus idle decay: absent players drift back toward the initial rating | `k_factor`, `initial_rating`, `beta`, `decay_rate` |
| `bradley_terry.lua` | Bradley-Terry pairwise comparison model: P(i>j) = sigmoid(r_i - r_j) | `initial_rating`, `learning_rate` |
| `thurstone.lua` | Thurstone-Mosteller: P(i>j) = Φ((μ_i - μ_j)/√(σ_i² + σ_j²)) | `initial_rating`, `initial_sigma`, `learning_rate` |
| `massey.lua` | Massey rating: uses score differentials via linear system Mr = p | `initial_rating`, `learning_rate`, `regularization` |
| `colley.lua` | Colley rating: regularized win/loss system C r = b | `initial_rating` |
| `whr.lua` | Whole-History Rating: Bayesian logistic with Gaussian prior | `initial_rating`, `prior_variance`, `learning_rate` |
| `trueskill_through_time.lua` | TrueSkill with skill drift: σ² += γ² between observations | `initial_mean`, `initial_variance`, `beta`, `gamma` |
| `openskill.lua` | OpenSkill: team-weighted TrueSkill variant | `initial_mean`, `initial_variance`, `beta` |
| `rank_centrality.lua` | Rank Centrality: spectral inference over match graph | `initial_rating`, `damping_factor`, `iterations` |
| `pagerank.lua` | PageRank-style rating: eigenvector centrality on match graph | `initial_rating`, `damping_factor`, `iterations` |
| `bayesian_logistic.lua` | Bayesian logistic: MAP estimate with Gaussian prior | `initial_rating`, `prior_variance`, `learning_rate` |
| `bayesian_hierarchical.lua` | Bayesian hierarchical: population-level shrinkage | `initial_rating`, `population_prior_mean`, `learning_rate` |

*Information budgets:* rating systems declare what match data they may read
(e.g. Elo and Glicko-2 read only win/loss). The loop enforces the budget by
sanitizing match results before a system sees them, so a WinLoss-only system can
never peek at scores, performances, or durations.

### Outcome models (`plugins/game/`)

The "game" — the part of the simulation that decides reality. All are logistic
at their core; each variant shifts the effective skill used for the
win-probability computation.

| Script | What it does | Key config |
|--------|--------------|------------|
| `logistic.lua` | Baseline: P(A wins) = logistic of the average true-skill difference | `beta`, `noise` |
| `variance.lua` | Scaled noise envelope → more upsets at a given skill gap | `beta`, `noise`, `variance_multiplier` |
| `composition.lua` | Effective skill is a weighted skill vector; team totals add synergy — the multidimensional-skill research model | `beta`, `dimension_weights`, `synergy_bonus` |
| `performance.lua` | Recent performances tilt effective skill → hot/cold streaks | `beta`, `performance_weight` |
| `fatigue.lua` | Skill decays with games played in a session | `beta`, `noise`, `fatigue_decay_rate` |
| `momentum.lua` | Skill scales with win-rate momentum | `beta`, `noise`, `momentum_factor` |

### Matchmakers (`plugins/matchmaking/`)

| Script | What it does | Key config |
|--------|--------------|------------|
| `batch.lua` | **Rating-balanced:** sorts by visible rating (ties by join order) and assigns alternatingly to teams. Near-even teams at ~0.98 quality; the default | — |
| `expanding_window.lua` | Matches within a skill window that **widens with queue wait**, via stepped tiers — the classic quality-vs-wait knob | `tiers` (`{max_secs, allowed_diff}`), `max_window` |
| `strict.lua` | Only matches players within a fixed skill difference; outliers may wait indefinitely (intended) | `max_skill_diff` |
| `hub_spoke.lua` | Partitions the queue by region; under-capacity regions match regionally, overflow falls to a longest-waiting hub | `spoke_capacity` |
| `random.lua` | Uniform-random formation for the feedback-loop comparison — no rating balancing | — |

All five matchmakers are **role-aware**: when the manifest sets `teams.a.role`
and `teams.b.role`, each side is filled exclusively from queue entries whose
`role` matches. Roles unset (the default) means "any player fills any slot" —
the legacy behavior. See `experiments/dbd_1v4.yaml` for a working example.

### Detection (`plugins/detection/`)

| Script | What it does | Key config |
|--------|--------------|------------|
| `smurf.lua` | Infers smurf status from behavior: expected performance from visible rating vs. actual from performance stats. Consecutive anomalies ramp the suspicion probability; an escalation ladder walks from none → accelerate rating → flag → restrict → temp ban → probation → ban | `sigma_threshold`, `min_anomalous_games`, `min_games_before_action`, `escalation_factor`, `ladder` |

### Ranking (`plugins/ranking/`)

| Script | What it does | Key config |
|--------|--------------|------------|
| `brackets.lua` | Maps a rating to a tier/division bracket; the visible rank updates as ratings move | `brackets` (`{tier, division, min, max}`) |

### Adversarial agents (`plugins/adversarial/`) — misbehavior on cue

Attach agents to specific players (`player: <id>`) to inject realistic abuse,
so you can measure how much damage it does and whether countermeasures help.

| Script | What it does | Key config |
|--------|--------------|------------|
| `afk.lua` | Goes AFK with a probability — abandoned games | `go_afk_probability` |
| `deranker.lua` | Intentionally loses (throws) while above a target rating | `target_rating` |
| `win_trader.lua` | Two players party up and alternate wins | `partner`, `alternating` |
| `booster.lua` | A boost duo: one carries a partner to a 1.0 win rate | `boost_target`, `boostee` |
| `rating_farmer.lua` | Queues then quits — keeps games played minimal (smurf-account farming) | `quit_probability`, `quit_after_minutes` |

### Satisfaction (`plugins/utility/`)

| Script | What it does | Key config |
|--------|--------------|------------|
| `satisfaction.lua` | Models player satisfaction and retention. Satisfaction is a weighted sum of match quality, queue time, wins, loss-streak penalty (only below −3), rank progression, fairness, and rematch bonus; retention = logistic of satisfaction; rematch needs a higher threshold | `match_quality`, `queue_time_penalty`, `win_bonus`, `loss_streak_penalty`, `rank_progression_bonus`, `fairness_sensitivity`, `rematch_bonus` |

Satisfaction turns the simulation into an **ecology**: bad match quality or long
queues make players churn mid-experiment, so the system must keep the population
healthy to keep going. The `population_health` metric then reflects the
consequences of matchmaking policy on live population.

---

## Metrics

Each metric is a Lua collector under `plugins/metrics/` that accumulates
evidence across matches; the engine folds its results into the output. The
metrics answer the questions researchers actually care about:

| Metric | Question | Direction |
|--------|----------|-----------|
| `rating_accuracy` | Mean absolute error between ratings and *true skill* | lower = better |
| `rating_accuracy_by_time` | Same error bucketed over the sim's time — the **convergence curve** | should descend |
| `match_quality` | `1 − (|avgA − avgB| / 400)`, team balance | higher = better |
| `queue_time` | Real join→formation wait per player | lower = better |
| `match_inequality` | Distribution of expected win probabilities; tight around 0.5 = well-matched | variance = bad |
| `ndcg` | Discounted cumulative gain over match qualities — are good matches served early? | higher = better |
| `dimensionality_fidelity` | Correlation of 1D ratings vs. skill-vector predictions against true skill; how much multiD improves over 1D | higher = better |
| `convergence` | Matches until `|rating − skill|` drops below threshold | lower = better |
| `responsiveness` | Fraction of updates moving in the direction the outcome predicts | higher = better |
| `stability` | Rating variance for stable players only | lower = better |
| `streaks` | Probability of 3/5/8/10-game win/loss streaks | context |
| `population_health` | Rating inflation/deflation and compression over the run | near-flat = good |
| `smurf` | Unfairness of matches containing a smurf (identified by properties, never a flag) | lower = better |

Every collector that supports it also emits a `{name}_by_time` time series,
letting you plot convergence and degradation over simulated time.

### Objectives (multi-objective scoring)

An experiment may declare `objectives:` weights to collapse the metric bundle
into a single **utility score**:

```yaml
objectives:
  match_quality: 1.0
  queue_time: 0.5        # lower-is-better → subtracted
  rating_accuracy: 1.0   # lower-is-better → subtracted
  convergence_speed: 0.8
  smurf_damage: 2.0
  false_positive_rate: 1.5
  streak_frustration: 0.3
```

Higher-is-better metrics add their weighted mean; lower-is-better metrics
subtract. Two experiments can then be compared on a single number — or their raw
metric maps can be compared directly (the raw data is never discarded).

### Reports

With `output.report: true`, the runner writes a Markdown report
(`results/<name>.md`) containing the config hash, git commit, and the full
metric table. JSON output (`results/<name>.json`) is the complete
`ExperimentResult` and is what downstream tools should consume.

---

## Adding a custom system

No Rust. No recompile. To test a genuinely new idea, write one Lua file and
reference it in a manifest.

As an example, the repo ships `plugins/rating/decay_elo.lua` — classic Elo plus
an idle-decay term (a returning player drifts back toward the initial rating).
It is a *novel* rating system with no Rust equivalent, plugged in purely as
Lua, and exercised by `experiments/novel_rating.yaml`.

A rating-system script's contract is four functions plus a global:

```lua
information_budget = { "WinLoss" }          -- what match data this system may read

function initialize(player_id, config, context)
    -- Return the player's initial { rating, rating_deviation, volatility, games_played }.
    -- `context` is your persistent state table; keep per-player state here.
end

function predict(team_a, team_b, config, context)
    -- Expected P(team_a wins), 0..1, from observations only.
end

function update(match_result, observations, config, context)
    -- Return { player_id = new_rating_state, ... }.
end
```

The other layers have analogous contracts (`find_matches` for matchmakers,
`win_probability`/`simulate` for outcome models, `observe`/`evaluate`/
`recommend_action` for detection, `on_record`/`compute` for metrics, `tick`/
`objective` for agents, `satisfaction`/`retention_probability` for utility,
`rating_to_rank`/`rank_to_rating_range` for ranking). See
[`docs/plugin-api.md`](docs/plugin-api.md) for the full contract reference,
and [`docs/plugin-cookbook.md`](docs/plugin-cookbook.md) for step-by-step
guides for each plugin type.

**Rules for scripts:**

- **Pure randomness.** Never call `math.random` (the loader bans it). Draw from
  `matchlab.rng_range`, `matchlab.rng_bool`, `matchlab.rng_normal`, or
  `matchlab.rng_u64` — these are deterministic given the experiment seed.
- **Observable data only.** Ratings, waits, results. You never receive
  `PlayerReality` (true skill) — except outcome models and metric collectors,
  which are the designated ground-truth readers.
- **`context` is how you remember.** Anything you need across calls (per-player
  evidence, last-update ticks, accumulated samples) lives in the context table,
  keyed however you like.
- **Deterministic iteration.** Lua table order is unspecified; iterate with
  `ipairs` over the arrays the adapters give you, not `pairs` over your own
  tables, wherever output order matters.

---

## Reproducing results

Each result JSON records `config_hash` (a hash of the serialized config *and*
the contents of every referenced Lua script) and `git_commit`. To reproduce an
experiment exactly:

1. Check out the recorded commit: `git checkout <git_commit>`
2. Run the manifest: `cargo run -- run <manifest.yaml>`
3. Verify the `config_hash` in the new JSON matches the recorded one.

The only field that legitimately differs between identical runs is the
`timestamp`.

You can also create a **reproduction package** that bundles the manifest,
all referenced Lua scripts, and metadata:

```bash
matchlab package experiments/v0_1_basic.yaml -o reproduction.json
```

---

## Comparing experiments

Run two or more experiments, then compare them side-by-side:

```bash
cargo run -- run experiments/glicko_comparison.yaml
cargo run -- run experiments/matchmaker_comparison.yaml
cargo run -- compare results/glicko_comparison.json results/matchmaker_comparison.json
```

`matchlab compare` prints a Markdown table of per-metric differences and, when
any result carries an `objectives:` utility score, ranks them by that score.
Add `--json` to get the comparison as structured JSON instead.

This is how controlled experiments work: inherit a base manifest, change one
variable, run both, and compare. The metric table shows exactly what changed
and by how much.

For replicated studies with confidence intervals:

```bash
matchlab study experiments/studies/elo_vs_glicko.yaml
```

---

## Observability

matchlab provides structured logging for debugging and monitoring. Control
verbosity from the command line:

```bash
matchlab run experiment.yaml --verbose              # debug-level output
matchlab run experiment.yaml --log-level trace       # trace-level output
matchlab run experiment.yaml --log-file run.log      # log to file
matchlab run experiment.yaml --json-logs             # JSON Lines for tooling
```

Or use the `RUST_LOG` environment variable for per-crate control:

```bash
RUST_LOG=matchlab_matchmaking=debug cargo run -- run experiment.yaml
```

See [`docs/observability.md`](docs/observability.md) for the full guide,
including debugging workflows for matchmaking quality, rating convergence,
and simulation problems.

---

## Project layout

```
plugins/                  Lua system scripts (one per algorithm)
  rating/ game/ matchmaking/ detection/ ranking/
  metrics/ adversarial/ utility/
experiments/              YAML manifests (including base/ for inheritance)
  examples/               Getting-started manifests
  studies/                Multi-arm study manifests
  feedback_loop/          Factorial grid experiments
  base/                   Inherited base configs
crates/                   Rust crate workspace
  matchlab-core/          simulation engine, time, events, world, RNG, core types
  matchlab-lua/           Lua foundation (VM, config, context, deterministic RNG)
  matchlab-players/       population generation, skill dynamics, distributions
  matchlab-game/          outcome-model adapter, performance model, team model
  matchlab-matchmaking/   queue, matchmakers, search strategies, parties, latency
  matchlab-rating/        rating systems, information budgets
  matchlab-detection/     smurf detection
  matchlab-ranking/       rank mapping, leaderboard
  matchlab-metrics/       metric collectors, statistics
  matchlab-objective/     multi-objective utility scoring
  matchlab-adversarial/   adversarial agents
  matchlab-utility/       satisfaction / retention
  matchlab-loop/          the simulation loop (event handlers)
  matchlab-experiments/   manifest parsing, config inheritance, runner, factorial design, replication
  matchlab-analysis/      statistics, Pareto, cohorts, reports, provenance
  matchlab-validation/    analytical-baseline regression tests (test-side only)
docs/                     Documentation
  manifest-schema.md      Complete manifest schema reference
  plugin-api.md           Plugin API contracts for all 7 types
  plugin-cookbook.md       Step-by-step plugin development guides
  methodology.md          Research methodology and conceptual model
  assumptions-limitations.md  Threats to validity
  observability.md        Logging and debugging guide
  workflow-certification.md   End-to-end researcher workflow
  getting-started.md      Installation and first experiment
  terminology.md          Canonical vocabulary
  units-conventions.md    Units and numerical conventions
  api-stability.md        Public API stability policy
  artifact-formats.md     Research artifact schemas
  security-review.md      Trust model and security
```

---

## Further reading

| Document | What it covers |
|----------|----------------|
| [`docs/manifest-schema.md`](docs/manifest-schema.md) | Complete manifest schema with all fields, types, and defaults |
| [`docs/plugin-api.md`](docs/plugin-api.md) | Plugin API contracts for all 7 Lua plugin types |
| [`docs/plugin-cookbook.md`](docs/plugin-cookbook.md) | Step-by-step guides for writing each type of plugin |
| [`docs/methodology.md`](docs/methodology.md) | Research methodology and conceptual model |
| [`docs/assumptions-limitations.md`](docs/assumptions-limitations.md) | Assumptions, limitations, and threats to validity |
| [`docs/observability.md`](docs/observability.md) | Logging, debugging, and monitoring guide |
| [`docs/workflow-certification.md`](docs/workflow-certification.md) | End-to-end researcher workflow |
| [`docs/guides/getting-started.md`](docs/guides/getting-started.md) | Installation and first experiment |
| [`docs/terminology.md`](docs/terminology.md) | Canonical vocabulary |
| [`docs/units-conventions.md`](docs/units-conventions.md) | Units and numerical conventions |
| [`docs/api-stability.md`](docs/api-stability.md) | Public API stability policy |
| [`docs/artifact-formats.md`](docs/artifact-formats.md) | Research artifact schemas |
| [`docs/security-review.md`](docs/security-review.md) | Trust model and security |
| [`docs/reproduction-studies.md`](docs/reproduction-studies.md) | How to conduct independent reproduction studies |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Development setup and contribution guide |
