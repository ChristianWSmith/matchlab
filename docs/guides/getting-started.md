# Getting Started

This guide walks you through installing matchlab, running your first experiment, and understanding the output.

## Table of Contents

- [Installation](#installation)
- [Running Your First Experiment](#running-your-first-experiment)
- [Understanding the Output](#understanding-the-output)
- [Next Steps](#next-steps)

---

## Installation

### Build from Source

matchlab requires Rust (edition 2024). Install Rust via [rustup](https://rustup.rs/), then:

```bash
git clone https://github.com/ChristianWSmith/matchlab.git
cd matchlab
cargo build --release
```

The binary will be at `target/release/matchlab`.

### Install with Cargo

```bash
cargo install --path .
```

This installs the `matchlab` binary into your Cargo bin directory.

### Verify

```bash
matchlab --help
```

You should see the CLI help listing `run`, `study`, and `compare` subcommands.

---

## Running Your First Experiment

The simplest experiment is `v0_1_basic.yaml` -- 10,000 players, Elo rating, logistic game model, 5v5 matches:

```bash
matchlab run experiments/v0_1_basic.yaml
```

Or equivalently with `cargo run` from the repo root:

```bash
cargo run -- run experiments/v0_1_basic.yaml
```

### What Happens

1. A population of 10,000 synthetic players is generated with skills drawn from N(1000, 250).
2. Players join a queue and the batch matchmaker pairs them into 5v5 matches.
3. Match outcomes are decided by the logistic game model (true skill determines winners).
4. After each match, Elo updates each player's visible rating.
5. Metrics are recorded throughout: match quality, queue time, and rating accuracy.
6. The experiment runs until 1,000,000 matches are completed or 7 simulated days elapse.

### Runtime

On a modern machine this takes approximately 2-5 minutes depending on hardware.

### Output

Results are written to `results/` (as configured in the YAML):

```
results/
  v0_1_basic.json      # ExperimentResult as JSON
```

If `report: true` is set in the manifest, a Markdown report is also generated:

```
results/
  v0_1_basic.json
  v0_1_basic.md        # Human-readable report
```

---

## Understanding the Output

### JSON Result

The JSON file contains the full `ExperimentResult`:

```json
{
  "experiment_id": "v0_1_basic-abc123...",
  "name": "v0_1_basic",
  "config_hash": "abc123def456...",
  "git_commit": "deadbeef...",
  "timestamp": "2026-01-15T12:00:00Z",
  "matches_completed": 1000000,
  "matches_formed": 1000000,
  "simulated_time_secs": 604800.0,
  "metrics": {
    "match_quality": {
      "mean": 0.98,
      "median": 0.99,
      "p90": 1.0,
      "p95": 1.0,
      "p99": 1.0,
      "stddev": 0.03
    },
    "queue_time": {
      "mean": 5.01,
      "median": 3.2,
      "p90": 12.5,
      "p95": 18.0,
      "p99": 30.0,
      "stddev": 4.2
    },
    "rating_accuracy": {
      "mean": 162.3,
      "median": 145.0,
      ...
    }
  },
  "utility_score": null
}
```

### Key Metrics

| Metric | What It Measures | What to Look For |
|--------|-----------------|------------------|
| `match_quality` | Balance between teams (1.0 = perfectly balanced) | > 0.9 is good; < 0.8 suggests matchmaker issues |
| `queue_time` | Seconds from queue join to match formation | Lower is better; depends on population size and matchmaker |
| `rating_accuracy` | Mean absolute error between rating and true skill | Should decrease over time as ratings converge |
| `rating_accuracy_by_time` | Rating accuracy over simulated time | Shows convergence trajectory |
| `match_inequality` | Rating gap between matched teams | Lower = more balanced matches |
| `ndcg` | Ranking quality (Normalized Discounted Cumulative Gain) | Higher = better ranking quality |
| `convergence` | How quickly ratings stabilize | Measured in games or time |
| `responsiveness` | How quickly ratings react to skill changes | Higher = more responsive |
| `stability` | Rating volatility | Lower = more stable |
| `streaks` | Win/loss streak distribution | Distribution of streak lengths |
| `population_health` | Population balance and diversity | Overall ecosystem health |
| `smurf` | Smurf detection accuracy | Detection rate vs false positive rate |

### Markdown Report

The report provides a human-readable summary:

```markdown
# Experiment: v0_1_basic

**Config Hash:** abc123def456...
**Git Commit:** deadbeef...
**Matches Completed:** 1,000,000
**Simulated Time:** 7.0 days

## Metrics

| Metric | Mean | Median | P90 | P95 | P99 | StdDev |
|--------|------|--------|-----|-----|-----|--------|
| match_quality | 0.98 | 0.99 | 1.0 | 1.0 | 1.0 | 0.03 |
| queue_time | 5.01 | 3.20 | 12.5 | 18.0 | 30.0 | 4.20 |
| rating_accuracy | 162.3 | 145.0 | 280.0 | 320.0 | 380.0 | 85.0 |
```

---

## Next Steps

### Customize the Experiment

Edit the YAML manifest to change parameters:

```yaml
experiment:
  name: my_experiment
  seed: 42
  population:
    size: 5000
    archetypes:
      - name: casual
        proportion: 0.7
        skill_distribution: { type: normal, mean: 1000, stddev: 200 }
        # ...
      - name: competitive
        proportion: 0.3
        skill_distribution: { type: normal, mean: 1200, stddev: 300 }
        # ...
  game:
    teams: { a: 5, b: 5 }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/expanding_window.lua
    # ...
  rating:
    systems:
      - script: plugins/rating/elo.lua
        k_factor: 32.0
        # ...
```

### Compare Systems

Use `matchlab compare` to compare two experiment results side by side:

```bash
# Run two experiments
matchlab run experiments/elo_experiment.yaml
matchlab run experiments/glicko_experiment.yaml

# Compare them
matchlab compare results/elo_experiment.json results/glicko_experiment.json
```

### Run a Study

For multi-arm comparisons with replication, use a study manifest:

```bash
matchlab study experiments/studies/elo_vs_glicko.yaml
```

This runs both rating systems on the same population across multiple replicates, computing confidence intervals and effect sizes.

### Write a Custom Plugin

See the [Plugin Cookbook](../plugin-cookbook.md) for a step-by-step guide to writing your own rating systems, outcome models, matchmakers, metrics, detection systems, adversarial agents, and satisfaction models.

### Config Inheritance

Experiments support YAML-level inheritance. Create a base config and override specific fields:

```yaml
# experiments/base/my_base.yaml
experiment:
  name: _base
  seed: 42
  population:
    size: 2000
    # ... shared defaults

# experiments/my_experiment.yaml
experiment:
  name: my_experiment
  base: experiments/base/my_base.yaml
  rating:
    systems:
      - script: plugins/rating/glicko2.lua
        # override rating system only
```

The child deep-merges over the parent: mappings merge recursively, scalars and sequences are replaced.
