# Experiment Manifest Schema

This document defines the complete schema for MatchLab experiment and study manifests.

## Overview

MatchLab uses two manifest shapes:

1. **Experiment manifest** — runs a single simulation experiment
2. **Study manifest** — runs a multi-arm comparative study with replications

Both are YAML files. An experiment manifest can declare `base:` to inherit from a base config.

## Configuration Inheritance

A manifest can declare `base: <path>` at the top level to inherit from another config:

```yaml
base: experiments/base/standard.yaml
experiment:
  name: my_experiment
  # ... overrides only
```

**Merge rules:**
- Mappings (dictionaries) merge recursively — the child overrides specific keys
- Scalars and sequences (lists) are replaced entirely by the child
- The base path is resolved relative to the manifest file's directory

**Example:** If the base has `population.size: 2000` and the child has `population.size: 500`, the result is `population.size: 500`. If the base has `metrics: [match_quality, queue_time]` and the child has `metrics: [rating_accuracy]`, the result is `metrics: [rating_accuracy]` (the entire list is replaced).

---

## Experiment Manifest

```yaml
base: <path>                    # optional, inherited base config
experiment:
  name: <string>                # required, experiment identifier
  description: <string>         # optional
  seed: <u64>                   # required, master experiment seed

  population:                   # required
    size: <u64>                 #   total players to generate
    seed: <u64>                 #   population generation seed
    archetypes:                 #   required, list of player archetypes
      - name: <string>          #     archetype identifier
        proportion: <f64>       #     fraction of population (0.0-1.0)
        skill_distribution:     #     required, how skill is sampled
          type: normal          #     distribution type
          mean: <f64>           #     distribution parameter
          stddev: <f64>         #     distribution parameter
        skill_volatility: <f64> #     per-tick skill noise (0.0 = static)
        improvement_rate: <f64> #     per-tick skill drift (0.0 = none)
        play_frequency: <f64>   #     probability of being online per tick
        session_length: <f64>   #     average session duration in seconds
        quit_probability: <f64> #     per-tick quit probability
        initial_rating: <f64>   #   optional, visible rating start value
        role: <string>          #   optional, fixed role label

  game:                         # required
    teams:                      #   optional, default 5v5
      a: <int>                  #     team A size (bare integer)
      b: <int>                  #     team B size
    # OR with roles:
    #   a: { size: 1, role: killer }
    #   b: { size: 4, role: survivor }
    script: <path>              #   optional, default plugins/game/logistic.lua
    skill_update_interval_secs: <f64>  # optional, absent = static skill
    # Additional script params (flattened):
    beta: <f64>
    noise: <f64>
    fatigue_decay_rate: <f64>
    variance_multiplier: <f64>
    performance_weight: <f64>
    momentum_factor: <f64>
    dimension_weights: <mapping>

  matchmaking:                  # required
    script: <path>              #   optional, default plugins/matchmaking/batch.lua
    max_queue_time: <f64>       #   required, seconds
    # Additional script params (flattened):
    batch_interval: <f64>
    tiers: <list>

  rating:                       # required
    systems:                    #   required, list of rating systems
      - name: <string>          #     optional, built-in name (elo, glicko2, etc.)
        script: <path>          #     optional, explicit script path
        # Additional script params (flattened):
        k_factor: <f64>
        initial_rating: <f64>
        beta: <f64>

  detection:                    # optional, absent = no detection
    enabled: <bool>             #   required
    script: <path>              #   optional, default plugins/detection/smurf.lua
    # Additional script params (flattened):
    min_games_before_action: <u64>
    min_anomalous_games: <u64>
    escalation_factor: <f64>

  ranking:                      # optional, absent = no ranking
    script: <path>              #   optional, default plugins/ranking/brackets.lua
    # Additional script params (flattened):
    brackets: <list>

  metrics:                      # required, list of metric names or scripts
    - <string>                  #   a metric name (e.g. "match_quality")
    # OR:
    - script: <path>            #   a custom metric script path
      param1: <value>           #   optional script params

  objectives:                   # optional, absent = no utility score
    match_quality: <f64>        #   all optional, weight values
    queue_time: <f64>
    rating_accuracy: <f64>
    convergence_speed: <f64>
    smurf_damage: <f64>
    false_positive_rate: <f64>
    streak_frustration: <f64>

  adversarial:                  # optional, absent = no adversarial agents
    agents:
      - script: <path>          #   required, Lua agent script
        player: <u64>           #   optional, specific player ID
        # Additional script params (flattened):
        go_afk_probability: <f64>
        target_rating: <f64>

  satisfaction:                 # optional, absent = no satisfaction model
    enabled: <bool>             #   required
    script: <path>              #   optional, default plugins/utility/satisfaction.lua
    # Additional script params (flattened):
    match_quality: <f64>
    queue_time_penalty: <f64>

  cohorts:                      # required, use [] when unused
    - name: <string>            #   cohort label
      filter:                   #   required, filter definition
        type: <string>          #     filter type

  duration:                     # required
    matches: <u64>              #   maximum matches to complete
    max_time: <f64>             #   maximum simulation time in seconds

  output:                       # required
    directory: <string>         #   output directory
    formats:                    #   required, currently only "json"
      - <string>
    plots: <bool>               #   whether to generate plots
    report: <bool>              #   whether to generate Markdown report

  replication:                  # optional, makes this a 1-arm study
    count: <u64>                #   number of replicate runs
    strategy: <string>          #   independent, crn, or counterfactual
    base_seed: <u64>            #   optional, default 42
```

---

## Study Manifest

```yaml
study:
  name: <string>                # required, study name
  base: <path>                  # required, base experiment manifest
  arms:                         # required, named arms
    - name: <string>            #   arm label
      overrides:                #   optional, dotted-path overrides
        experiment.rating.systems.0.script: <path>
  replication:                  # required
    count: <u64>                #   number of replicates
    strategy: <string>          #   independent, crn, or counterfactual
    base_seed: <u64>            #   optional, default 42
  metrics:                      # optional, overrides base metrics
    - <string>
  cohorts:                      # optional, overrides base cohorts
    - name: <string>
      filter:
        type: <string>
  estimand: <string>            # optional, declared primary estimand
  output:                       # required
    directory: <string>
    formats:                    #   optional, default ["json"]
      - <string>
```

---

## Distribution Types

The `skill_distribution` field accepts a tagged enum:

| Type | Fields | Example |
|------|--------|---------|
| `normal` | `mean: <f64>`, `stddev: <f64>` | `{ type: normal, mean: 1000, stddev: 250 }` |
| `uniform` | `low: <f64>`, `high: <f64>` | `{ type: uniform, low: 0.0, high: 1000.0 }` |
| `log_normal` | `mean: <f64>`, `stddev: <f64>` | `{ type: log_normal, mean: 7.0, stddev: 0.5 }` |

---

## Cohort Filter Types

| Type | Fields | Example |
|------|--------|---------|
| `all` | _(none)_ | `{ type: all }` |
| `archetype` | `value: <string>` | `{ type: archetype, value: smurf }` |
| `smurf_by_properties` | `min_skill: <f64>` (default 1300.0), `max_games: <u64>` (default 20) | `{ type: smurf_by_properties, min_skill: 1300.0, max_games: 20 }` |
| `games_played_range` | `low: <u64>`, `high: <u64>` | `{ type: games_played_range, low: 10, high: 100 }` |
| `skill_range` | `low: <f64>`, `high: <f64>` | `{ type: skill_range, low: 1200.0, high: 1500.0 }` |
| `party_size` | `size: <usize>` | `{ type: party_size, size: 2 }` |
| `session_length` | `min: <f64>`, `max: <f64>` | `{ type: session_length, min: 300.0, max: 3600.0 }` |

---

## Replication Strategies

| Strategy | Behavior |
|----------|----------|
| `independent` | Each arm gets a distinct seed; fully independent runs |
| `crn` | Common Random Numbers: every arm shares the replicate seed for matched-pair variance reduction |
| `counterfactual` | First arm runs live and records; other arms replay the identical match history |

---

## Metric Names

| Name | Description |
|------|-------------|
| `match_quality` | Mean match balance quality (1.0 = perfectly balanced) |
| `queue_time` | Mean queue wait time in seconds |
| `rating_accuracy` | Mean absolute error between rating and true skill |
| `match_inequality` | Gini-like inequality of match outcomes |
| `ndcg` | Normalized Discounted Cumulative Gain of rating ordering |
| `dimensionality_fidelity` | How well a 1D rating represents multidimensional skill |
| `convergence` | How quickly ratings converge to true skill |
| `responsiveness` | How quickly ratings react to skill changes |
| `stability` | Rating volatility / oscillation |
| `streaks` | Win/loss streak distribution |
| `population_health` | Overall population distribution health |
| `smurf` | Smurf detection and damage metrics |

---

## Lua Script Paths

| Layer | Scripts |
|-------|---------|
| Outcome models (`plugins/game/`) | `logistic.lua`, `variance.lua`, `composition.lua`, `performance.lua`, `fatigue.lua`, `momentum.lua` |
| Matchmakers (`plugins/matchmaking/`) | `batch.lua`, `expanding_window.lua`, `strict.lua`, `hub_spoke.lua`, `random.lua` |
| Rating systems (`plugins/rating/`) | `elo.lua`, `flat.lua`, `glicko2.lua`, `trueskill.lua`, `decay_elo.lua` |
| Detection (`plugins/detection/`) | `smurf.lua` |
| Ranking (`plugins/ranking/`) | `brackets.lua` |
| Adversarial (`plugins/adversarial/`) | `afk.lua`, `deranker.lua`, `win_trader.lua`, `booster.lua`, `rating_farmer.lua` |
| Satisfaction (`plugins/utility/`) | `satisfaction.lua` |

---

## Units

| Field | Unit | Notes |
|-------|------|-------|
| `duration.max_time` | seconds | Simulation wall-clock limit |
| `game.skill_update_interval_secs` | seconds | Interval for dynamic skill updates |
| `matchmaking.max_queue_time` | seconds | Maximum allowed queue wait |
| `population.archetypes[].session_length` | seconds | Average session duration |
| Skill values | points | Rating scale (typically centered at 1000) |
| `population.archetypes[].proportion` | fraction | 0.0 to 1.0, must sum to 1.0 |

---

## Example: Minimal v0.1 Experiment

```yaml
experiment:
  name: v0_1_basic
  seed: 42
  population:
    size: 10000
    seed: 42
    archetypes:
      - name: flat
        proportion: 1.0
        skill_distribution:
          type: normal
          mean: 1000.0
          stddev: 0.0
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 1.0
        session_length: 3600.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    teams: { a: 5, b: 5 }
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
  metrics:
    - match_quality
    - queue_time
    - rating_accuracy
  cohorts:
    - name: all
      filter: { type: all }
  duration:
    matches: 100000
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: true
```

---

## Example: Role-Gated Asymmetric Match

```yaml
base: experiments/base/standard.yaml
experiment:
  name: dbd_1v4
  population:
    archetypes:
      - name: killer
        proportion: 0.2
        skill_distribution:
          type: normal
          mean: 1250.0
          stddev: 250.0
        role: killer
      - name: survivor
        proportion: 0.8
        skill_distribution:
          type: normal
          mean: 1000.0
          stddev: 100.0
        role: survivor
  game:
    teams:
      a: { size: 1, role: killer }
      b: { size: 4, role: survivor }
```

---

## Example: Multi-Arm Study

```yaml
study:
  name: elo_vs_glicko
  base: experiments/base/standard.yaml
  arms:
    - name: elo
      overrides: {}
    - name: glicko2
      overrides:
        experiment.rating.systems.0.name: glicko2
  replication:
    count: 50
    strategy: crn
    base_seed: 42
  metrics:
    - rating_accuracy
    - match_quality
    - queue_time
  output:
    directory: results/studies/
```

---

## Optimization Manifest

```yaml
optimize:
  name: <string>                # required, optimization run name
  base: <path>                  # required, base experiment manifest
  seed: <u64>                   # required, master optimization seed
  budget: <u64>                 # required, total experiments to evaluate

  search_space:                 # required, parameters to optimize
    parameters:
      <dot.path>:               # dotted path into the experiment config
        type: float             #   parameter type
        bounds: [<f64>, <f64>]  #   [min, max]
        log_scale: <bool>       #   optional, default false
      <dot.path>:
        type: categorical       #   categorical parameter
        values: [<string>, ...] #   allowed values

  objectives:                   # required, what to optimize
    - metric: <string>          #   metric name to optimize
      direction: maximize       #   or minimize

  bo:                           # optional, Bayesian optimization settings
    initial_design: latin_hypercube  # or "random"
    initial_points: <u64>       #   optional, default 10
    kernel: matern52            #   optional: matern52, matern32, rbf, rq (default: matern52)
    acquisition: ei             #   optional: ei, ucb, pi (default: ei)
    xi: <f64>                   #   optional, EI/PI exploration parameter (default: 0.01)
    eta: <f64>                  #   optional, ParEGO scalarization (default: 0.05)
    ucb_beta: <f64>             #   optional, UCB exploration parameter (default: 2.0)

  output:                       # optional
    directory: <string>         #   default "results/optimization/"
    report: <bool>              #   default false
    checkpoint: <bool>          #   write NDJSON checkpoint after each trial (default: false)
    checkpoint_interval: <u64>  #   write checkpoint every N trials (default: every trial)
```

### Search Space Parameters

Search space parameters use dotted paths into the experiment config tree. Float bounds must satisfy `min < max`. Categorical parameters must have at least one value. These constraints are validated at load time.

Common paths:

| Path | What it controls |
|------|-----------------|
| `experiment.rating.systems.0.k_factor` | Elo K-factor |
| `experiment.rating.systems.0.beta` | Elo beta/divisor |
| `experiment.rating.systems.0.initial_rating` | Starting rating |
| `experiment.rating.systems.0.name` | Rating system choice (categorical) |
| `experiment.rating.systems.0.initial_rd` | Glicko-2 initial RD |
| `experiment.rating.systems.0.initial_volatility` | Glicko-2 initial volatility |
| `experiment.rating.systems.0.tau` | Glicko-2 tau constraint |
| `experiment.game.beta` | Outcome model logistic steepness |
| `experiment.game.noise` | Outcome model noise |
| `experiment.game.fatigue_decay_rate` | Fatigue decay rate |
| `experiment.game.momentum_factor` | Momentum factor |
| `experiment.matchmaking.max_queue_time` | Maximum queue wait |
| `experiment.matchmaking.script` | Matchmaker choice (categorical) |
| `experiment.detection.sigma_threshold` | Detection sensitivity |
| `experiment.detection.min_anomalous_games` | Detection confirmation threshold |

### Example: Optimize Elo K-factor

```yaml
optimize:
  name: elo_kfactor_optimization
  base: experiments/base/standard.yaml
  seed: 42
  budget: 50

  search_space:
    parameters:
      experiment.rating.systems.0.k_factor:
        type: float
        bounds: [1.0, 100.0]
      experiment.rating.systems.0.beta:
        type: float
        bounds: [100.0, 800.0]

  objectives:
    - metric: match_quality
      direction: maximize
    - metric: rating_accuracy
      direction: minimize

  bo:
    initial_points: 10
    acquisition: ei
```
