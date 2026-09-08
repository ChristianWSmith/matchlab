# Basic Study Example

This example walks through a complete rating system comparison study: Elo vs Glicko-2, with replication, confidence intervals, and effect sizes.

## Table of Contents

- [The Study Question](#the-study-question)
- [Complete YAML Manifest](#complete-yaml-manifest)
- [How to Run It](#how-to-run-it)
- [Expected Output](#expected-output)
- [Interpreting Results](#interpreting-results)
- [Modifying for Your Research](#modifying-for-your-research)

---

## The Study Question

**Does Glicko-2 produce more accurate ratings than Elo in a standard 5v5 matchmaking environment?**

We measure "accuracy" as the mean absolute error (MAE) between each player's visible rating and their true skill. Lower MAE = more accurate.

---

## Complete YAML Manifest

Create `experiments/studies/elo_vs_glicko_example.yaml`:

```yaml
study:
  name: elo_vs_glicko_example
  base: ../base/standard.yaml
  arms:
    - name: elo
      overrides: {}
    - name: glicko2
      overrides:
        experiment.rating.systems.0.script: plugins/rating/glicko2.lua
        experiment.rating.systems.0.initial_rating: 1000.0
        experiment.rating.systems.0.initial_rd: 350.0
        experiment.rating.systems.0.initial_volatility: 0.06
        experiment.rating.systems.0.tau: 0.5
  replication:
    count: 50
    strategy: crn
    base_seed: 42
  metrics: [match_quality, queue_time, rating_accuracy]
  cohorts: []
  output:
    directory: results/studies/elo_vs_glicko_example/
```

### What This Does

| Section | Purpose |
|---------|---------|
| `base: ../base/standard.yaml` | Inherits the standard 2,000-player population with mixed archetypes (stable, improving, declining, returning, smurf) |
| `arms` | Two conditions: Elo (unchanged from base) and Glicko-2 (overrides the rating system) |
| `overrides` | Dotted-path overrides applied to the resolved base config. Only the rating system leaf changes between arms |
| `replication.count: 50` | 50 independent replicates per arm (each replicate = a full simulation) |
| `replication.strategy: crn` | Common Random Numbers -- both arms share the same random seed per replicate, reducing variance |
| `replication.base_seed: 42` | Master seed for deriving per-replicate seeds |
| `metrics` | Three metrics to collect (queue_time and match_quality are arm-independent; rating_accuracy is the primary outcome) |

### The Base Config

The base config (`experiments/base/standard.yaml`) defines:

- **Population:** 2,000 players across 5 archetypes
  - 60% stable (N(1000, 250), no improvement)
  - 15% improving (N(800, 200), +2 skill/step)
  - 10% declining (N(1100, 150), -1.5 skill/step)
  - 10% returning (N(1000, 200), initial_rating: 700 -- smurf-like mismatch)
  - 5% smurf (N(1500, 100), initial_rating: 700 -- high skill, low visible rating)
- **Game:** 5v5 logistic outcome model (beta: 400, noise: 0.05)
- **Matchmaking:** Batch matchmaker (balanced formation)
- **Rating:** Elo (k_factor: 32, initial_rating: 1000, beta: 400)
- **Duration:** 50,000 matches or 7 simulated days

---

## How to Run It

### Quick Run (CI-Scale)

For a fast smoke test with 50 replicates:

```bash
matchlab study experiments/studies/elo_vs_glicko_example.yaml
```

This takes approximately 5-10 minutes depending on hardware.

### Production Run

For publication-quality results with tighter confidence intervals:

```bash
matchlab study experiments/studies/elo_vs_glicko_example.yaml --replicates 500
```

Each replicate runs both arms (Elo and Glicko-2) on the same population with shared randomness. 500 replicates takes approximately 1-2 hours.

### What Runs

1. 50 replicates are generated from `base_seed: 42` via `derive(42, i)` for i = 0..49.
2. For each replicate, both arms share the replicate seed (CRN strategy).
3. Each arm runs a full simulation: population generation, matchmaking loop, rating updates, metric collection.
4. Per-arm statistics are computed: mean, 95% confidence interval for each metric.
5. Pairwise effect sizes are computed: delta, 95% CI, Cohen's d, relative delta.

---

## Expected Output

### Console Output

```
Running study: elo_vs_glicko_example
  Strategy: CRN
  Replicates: 50
  Arms: elo, glicko2

Replicate 1/50 complete
Replicate 2/50 complete
...
Replicate 50/50 complete

Study complete: results/studies/elo_vs_glicko_example/elo_vs_glicko_example-<hash>-crn-r50.json
```

### JSON Output

The study result is written to:

```
results/studies/elo_vs_glicko_example/
  elo_vs_glicko_example-<hash>-crn-r50.json   # Full StudyResult
  study_stats.json                              # Aggregate statistics
```

### Markdown Report

A study report is generated at:

```
results/studies/elo_vs_glicko_example/
  elo_vs_glicko_example-<hash>-crn-r50.md
```

---

## Interpreting Results

### Per-Arm Statistics

The report includes a per-arm table:

| Arm | Metric | Mean | 95% CI |
|-----|--------|------|--------|
| elo | rating_accuracy | 166.2 | [164.8, 167.6] |
| glicko2 | rating_accuracy | 158.5 | [156.9, 160.1] |

### Pairwise Effects

The report includes pairwise comparisons:

```
rating_accuracy:
  Δ = -7.7, 95% CI [-9.5, -5.9]
  Cohen's d = -1.23
  relative Δ = -4.6%
```

### How to Read This

| Term | Meaning |
|------|---------|
| `Δ` | Absolute difference (glicko2 − elo). Negative means Glicko-2 has lower MAE (more accurate) |
| `95% CI` | If the interval excludes 0, the difference is statistically significant at α = 0.05 |
| `Cohen's d` | Effect size: \|d\| < 0.2 is negligible, 0.2-0.5 is small, 0.5-0.8 is medium, > 0.8 is large |
| `relative Δ` | Percentage change relative to the control (Elo) |

### Decision Rules

- **If 95% CI excludes 0 and \|d\| > 0.5:** Meaningful difference. Glicko-2 is more (or less) accurate.
- **If 95% CI includes 0:** No statistically significant difference at this sample size. Consider more replicates.
- **If \|d\| < 0.2:** Even if significant, the practical difference is small.

### Known Result

On the standard population (N=50 replicates), the acceptance run produces:

```
rating_accuracy:
  Δ = +208.32, 95% CI [207.43, 209.28]
  Cohen's d = 84.12
```

This means Glicko-2 actually has *higher* MAE than Elo on this population. The large effect size confirms this is a robust finding, not noise. Interpretation: Glicko-2's uncertainty tracking (RD) is designed for sparse play, but in a dense 5v5 environment with frequent matches, Elo's simpler model converges faster.

---

## Modifying for Your Research

### Change the Rating System Comparison

Replace Glicko-2 with TrueSkill:

```yaml
arms:
  - name: elo
    overrides: {}
  - name: trueskill
    overrides:
      experiment.rating.systems.0.script: plugins/rating/trueskill.lua
      experiment.rating.systems.0.initial_rating: 1000.0
      experiment.rating.systems.0.initial_variance: 122500.0
      experiment.rating.systems.0.draw_probability: 0.1
```

### Change the Game Model

Compare how outcome model noise affects rating accuracy:

```yaml
arms:
  - name: low_noise
    overrides:
      experiment.game.noise: 0.01
  - name: high_noise
    overrides:
      experiment.game.noise: 0.2
```

### Change the Matchmaker

Compare how matchmaker choice affects the ecosystem:

```yaml
arms:
  - name: batch
    overrides: {}
  - name: expanding_window
    overrides:
      experiment.matchmaking.script: plugins/matchmaking/expanding_window.lua
```

### Add a Third Arm

```yaml
arms:
  - name: elo
    overrides: {}
  - name: glicko2
    overrides:
      experiment.rating.systems.0.script: plugins/rating/glicko2.lua
      experiment.rating.systems.0.initial_rd: 350.0
  - name: trueskill
    overrides:
      experiment.rating.systems.0.script: plugins/rating/trueskill.lua
      experiment.rating.systems.0.initial_variance: 122500.0
```

### Change the Population

Override the population size or archetype proportions:

```yaml
arms:
  - name: small_pop
    overrides:
      experiment.population.size: 500
  - name: large_pop
    overrides:
      experiment.population.size: 10000
```

### Add More Metrics

```yaml
metrics: [match_quality, queue_time, rating_accuracy, convergence, stability, streaks]
```

### Cohort Analysis

Add per-player cohorts to see how different player types are affected:

```yaml
cohorts:
  - name: high_skill
    filter: { type: skill_range, min: 1200, max: 2000 }
  - name: low_skill
    filter: { type: skill_range, min: 0, max: 800 }
  - name: smurfs
    filter: { type: archetype, archetype: smurf }
```

### Replication Strategy

| Strategy | When to Use |
|----------|-------------|
| `crn` | Default. Reduces variance by sharing randomness across arms. Best for comparing algorithms on the same population |
| `independent` | Each arm gets independent seeds. Use when arms have different populations or you want fully independent runs |
| `counterfactual` | One arm runs live, others replay its history. Use for true counterfactual analysis (same match sequence, different rating system) |

### Adjusting Replicate Count

More replicates = tighter confidence intervals = more statistical power:

| Replicates | CI Width | Use Case |
|------------|----------|----------|
| 10 | Wide | Quick smoke test |
| 50 | Medium | Development iteration |
| 200 | Narrow | Publication-quality |
| 500+ | Very narrow | High-precision comparison |

### Running a Single Experiment (No Replication)

If you just want to compare two results without the study framework:

```bash
# Run two experiments
matchlab run experiments/elo_test.yaml
matchlab run experiments/glicko_test.yaml

# Compare results
matchlab compare results/elo_test.json results/glicko_test.json
```

The `compare` command produces a side-by-side Markdown report with the same effect size statistics.
