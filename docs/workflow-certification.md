# Research Workflow Certification

This document describes the end-to-end workflow a researcher follows in MatchLab, from formulating a question to producing a reproducible package. It serves as both a tutorial and a procedural reference.

## Overview

The workflow has seven stages:

```
Question → Design → Manifest → Simulation → Analysis → Report → Reproduction
```

Each stage produces an artifact that feeds the next. All artifacts are files — YAML configs, JSON results, Markdown reports — so the entire workflow is scriptable and version-controllable.

---

## Stage 1: Question

Formulate a precise, answerable question. MatchLab answers comparative questions of the form:

- *Under condition A vs condition B, how does metric M change?*
- *What is the effect of varying factor X on outcome Y?*
- *Does algorithm A outperform algorithm B after 50,000 matches?*

**Example questions from existing experiments:**

| Question | Experiment |
|----------|------------|
| Does Elo converge on a static population? | `experiments/v0_1_basic.yaml` |
| Does Glicko-2 track skill more accurately than Elo? | `experiments/studies/elo_vs_glicko.yaml` |
| How does matchmaker choice affect the feedback loop? | `experiments/feedback_loop/` |
| Can smurf detection identify high-skill players with low ratings? | `experiments/detection_test.yaml` |

Good questions are specific about: the factor being varied, the metric being measured, and the population or conditions under which the comparison holds.

---

## Stage 2: Design

Translate the question into an experimental design. Decide:

1. **Independent variable** — what you are varying (rating system, matchmaker, outcome model, population composition, etc.)
2. **Dependent variables** — what you are measuring (rating accuracy, match quality, queue time, etc.)
3. **Control conditions** — the baseline you compare against
4. **Population** — player archetypes, skill distribution, size
5. **Replication strategy** — how many independent runs per condition, and whether to use common random numbers (CRN), independent seeds, or counterfactual replay
6. **Estimand** — the precise quantity you want to estimate (absolute difference in mean rating accuracy, relative improvement in queue time, etc.)

### Choosing a Design Type

| Design | When to use | Example |
|--------|-------------|---------|
| **Single experiment** | Exploratory or confirmatory runs with one condition | `v0_1_basic.yaml` |
| **Paired study (CRN)** | Comparing two algorithms on identical populations | `elo_vs_glicko.yaml` |
| **Factorial design** | Varying multiple factors simultaneously | `feedback_loop/` grid |
| **Counterfactual replay** | Replaying identical match histories through different rating systems | `elo_vs_glicko_replay.yaml` |

### CRN vs Independent Seeds

- **Common Random Numbers (CRN):** Both arms share the replicate seed, so they simulate the same population and arrival sequence. The only difference is the algorithm being tested. This reduces variance and increases statistical power. Use CRN when comparing two algorithms on the same problem.
- **Independent seeds:** Each arm uses a distinct seed. Required when the algorithms themselves alter the simulation trajectory (e.g., different matchmakers produce different match sequences).

---

## Stage 3: Manifest

Write a YAML experiment or study manifest. See `docs/manifest-schema.md` for the full schema.

### Minimal Experiment (Single Condition)

```yaml
experiment:
  name: my_first_experiment
  description: "Does Elo converge on a 10k static population?"
  seed: 42

  population:
    size: 10000
    seed: 42
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0

  game:
    teams: { a: 5, b: 5 }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05

  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
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

  cohorts: []

  duration:
    matches: 50000
    max_time: 604800.0

  output:
    directory: results/
    formats: [json]
    report: true
```

### Study Manifest (Comparative)

```yaml
study:
  name: elo_vs_glicko
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
    directory: results/studies/elo_vs_glicko/
```

### Using Inheritance

Declare `base:` to inherit from a shared configuration and override only what differs. Mappings merge recursively; sequences and scalars are replaced entirely. The base path is resolved relative to the manifest file.

**Example:** The `elo_vs_glicko.yaml` study inherits `experiments/base/standard.yaml` and overrides only the rating system script and parameters for the glicko2 arm.

### Key Manifest Decisions

| Parameter | Guidance |
|-----------|----------|
| `seed` | Any u64. Use the same seed across studies for comparability. |
| `population.size` | 2,000 for development; 10,000+ for production runs. |
| `duration.matches` | 50,000 for development; 500,000–1,000,000 for production. |
| `duration.max_time` | 604,800 seconds (7 days sim time) is a common default. |
| `replication.count` | 50 for CI-fast runs; 500 for production. |
| `replication.strategy` | `crn` for algorithm comparisons; `independent` when algorithms change the simulation trajectory. |

---

## Stage 4: Simulation

Run the simulation from the command line.

### Single Experiment

```bash
cargo run -- run experiments/my_first_experiment.yaml
```

This produces:
- `results/my_first_experiment.json` — the full `ExperimentResult` (metrics, config hash, git commit)
- `results/my_first_experiment.md` — a Markdown report (when `output.report: true`)

### Comparative Study

```bash
cargo run -- study experiments/studies/elo_vs_glicko.yaml --replicates 50
```

This produces:
- `results/studies/elo_vs_glicko/<study_id>.json` — the `StudyResult`
- `results/studies/elo_vs_glicko/study_stats.json` — aggregate statistics
- A Markdown report with per-arm metric tables, pairwise effect sizes, and confidence intervals

### Comparing Results

```bash
cargo run -- compare results/elo_result.json results/glicko_result.json
```

Produces a side-by-side Markdown comparison with effect sizes and utility rankings.

### Quick Start: Your First Experiment

1. Copy `experiments/v0_1_basic.yaml` to `experiments/my_test.yaml`
2. Change `name:` to `my_test`
3. Reduce `duration.matches` to `10000` for a faster run
4. Run: `cargo run -- run experiments/my_test.yaml`
5. Read the output report in `results/my_test.md`

---

## Stage 5: Analysis

MatchLab performs analysis automatically during the run. The output report includes:

### Per-Arm Statistics

For each metric, the report shows:
- **Mean** across replicates
- **95% confidence interval** (bootstrap or Welch-t, depending on design)
- **Standard deviation**

### Pairwise Comparisons

For each metric, the report shows:
- **Δ (absolute difference)** between treatment and control
- **95% CI for Δ** — if it excludes zero, the difference is statistically significant
- **Cohen's d** — standardized effect size (small: 0.2, medium: 0.5, large: 0.8)
- **Relative Δ** — percentage change from control

### What to Look For

| Signal | Interpretation |
|--------|----------------|
| CI for Δ excludes 0 | Statistically significant difference |
| \|Cohen's d\| > 0.8 | Large practical effect |
| CI width is narrow | High precision (sufficient replication) |
| CI width is wide | Low precision (increase replication count) |
| Mean metrics are similar | Algorithms perform equivalently on this population |

### Interpreting Rating Accuracy

`rating_accuracy` reports the mean absolute error (MAE) between visible ratings and true skill. A decreasing `rating_accuracy_by_time` series indicates the rating system is converging — ratings are approaching truth.

### Interpreting Match Quality

`match_quality` measures how balanced the two teams are. Values near 1.0 indicate well-matched teams. The batch matchmaker typically produces quality near 0.96–0.98 on balanced populations.

### Interpreting Queue Time

`queue_time` measures the elapsed time between a player joining the queue and being assigned to a match. Lower is better, but there is a fundamental tradeoff between queue time and match quality.

---

## Stage 6: Report

The Markdown report is generated automatically when `output.report: true`. It includes:

1. **Experiment identity** — name, config hash, git commit, timestamp
2. **Configuration summary** — population size, algorithms used, duration
3. **Metric table** — all collected metrics with summary statistics
4. **Convergence series** — `rating_accuracy_by_time` showing learning over time

For studies, the report additionally includes:

5. **Per-arm metric table** — mean and 95% CI for each arm
6. **Pairwise effect estimates** — Δ, CI, Cohen's d, relative Δ per metric
7. **Utility ranking** — weighted multi-objective score (when objective weights are configured)

### Custom Reports

For custom analysis beyond the built-in report, use the exported JSON results programmatically. The `ExperimentResult` and `StudyResult` structs are serde-deserializable.

---

## Stage 7: Reproduction

A complete reproduction package includes:

1. **The manifest** — the exact YAML file used
2. **The result JSON** — the `ExperimentResult` or `StudyResult`
3. **The report** — the Markdown output
4. **The engine version** — recorded in the result as `git_commit`
5. **The config hash** — a SHA-256 of the resolved config + all Lua script contents, recorded in the result

### Reproducing a Result

```bash
# Clone the same commit
git checkout <commit_hash>

# Run the same manifest
cargo run -- run experiments/my_experiment.yaml

# The output JSON will have the same config_hash and metrics
# (wall-clock timestamp will differ)
```

### Determinism Guarantee

Given the same config + seed, the simulation produces byte-identical metrics. The only field that legitimately differs across runs is the wall-clock `timestamp`.

### What is Recorded

Each result JSON includes:
- `experiment_id` — `"{name}-{config_hash}"`
- `config_hash` — SHA-256 of serialized config + all referenced Lua scripts
- `git_commit` — the engine commit hash
- `metrics` — a `BTreeMap` (deterministic key order in JSON)
- `utility_score` — the weighted utility (when objectives are configured)

### Sharing Results

The result JSON is self-contained. Share it alongside the manifest and report. A consumer can:
- Read the metrics directly
- Compare against other results via `matchlab compare`
- Verify the config hash matches a given manifest

---

## Worked Examples

### Example 1: Does Elo Converge?

**Question:** Does Elo rating accuracy improve over time on a static population?

**Design:** Single experiment, 10,000 players, Elo + logistic outcome, batch matchmaker.

**Manifest:** `experiments/v0_1_basic.yaml`

**Run:**
```bash
cargo run -- run experiments/v0_1_basic.yaml
```

**Expected result:** `rating_accuracy_by_time` decreases from ~198 to ~166 over 1,000,000 matches. MAE decreases = Elo is learning.

**Reference:** Acceptance numbers in `docs/spec.md` §18.

### Example 2: Elo vs Glicko-2

**Question:** Does Glicko-2 achieve lower rating error than Elo?

**Design:** 2-arm CRN study, 50 replicates, shared population seeds.

**Manifest:** `experiments/studies/elo_vs_glicko.yaml`

**Run:**
```bash
cargo run -- study experiments/studies/elo_vs_glicko.yaml --replicates 50
```

**Expected result:** Glicko-2 shows lower `rating_accuracy` MAE. The acceptance run (N=50) recorded Δ = +208.32, 95% CI [207.43, 209.28], Cohen's d = 84.12.

### Example 3: Feedback Loop Grid

**Question:** How do rating systems and matchmakers interact in a feedback loop?

**Design:** 3×3 factorial: {Elo, Glicko-2, TrueSkill} × {random, strict, expanding_window}.

**Manifests:** `experiments/feedback_loop/feedback_*.yaml` (nine files)

**Run:**
```bash
for f in experiments/feedback_loop/feedback_*.yaml; do
  cargo run -- run "$f"
done
```

**Analysis:**
```bash
cargo run -- compare results/feedback_elo_random.json results/feedback_glicko2_expanding.json
```

### Example 4: Counterfactual Replay

**Question:** If we replay the same match history through a different rating system, what changes?

**Design:** 2-arm counterfactual study. Arm 1 runs Elo live and records history. Arm 2 replays that history through Glicko-2.

**Manifest:** `experiments/studies/elo_vs_glicko_replay.yaml`

**Run:**
```bash
cargo run -- study experiments/studies/elo_vs_glicko_replay.yaml --replicates 10
```

**Note:** Only replay-valid metrics survive (rating_accuracy, convergence, stability, streaks). Queue time and match quality are not applicable in replay mode.

---

## Checklist

Before considering a workflow complete, verify:

- [ ] Question is specific and answerable
- [ ] Design specifies factors, conditions, and estimands
- [ ] Manifest parses without errors (`cargo run -- run <manifest>`)
- [ ] Simulation completes (exit code 0)
- [ ] Result JSON contains expected metrics
- [ ] Report is generated (when `output.report: true`)
- [ ] Effect sizes and CIs are interpretable
- [ ] Config hash is recorded for reproducibility
- [ ] Git commit is recorded for version tracking
- [ ] Reproduction produces identical config hash and metrics
