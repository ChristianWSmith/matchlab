# Documentation & Research Methodology

This document describes MatchLab's conceptual model, simulation architecture, and the research methodology it supports. It is intended for researchers who want to understand what MatchLab measures, how it measures it, and what conclusions the framework can and cannot support.

---

## Ground Truth vs Observations

The central design principle in MatchLab is the separation between **ground truth** and **observations**.

### PlayerReality (Ground Truth)

Every player has a `PlayerReality` containing:
- **True skill** — a `SkillVector` with named dimensions (v0.1 uses a single `overall` dimension)
- **Improvement rate** — per-tick skill drift
- **Skill volatility** — per-tick noise in skill
- **Archetype membership** — the population segment the player belongs to
- **Games played** — total matches participated in

Ground truth is known to the simulation but is **never visible** to rating systems, matchmakers, or detection algorithms.

### PlayerObservation (Visible Data)

Every player also has a `PlayerObservation` containing:
- **Rating** — the algorithm's current estimate of the player's skill
- **Rating deviation (RD)** — uncertainty in the rating
- **Volatility** — rating system's estimate of skill instability
- **Games played** — matches observed by the rating system
- **Win rate** — derived from match history
- **Visible rank** — mapped from rating via the rank mapper
- **Queue state** — joined_at, party_id, region, role

### Why This Matters

The separation ensures that rating systems **learn from match outcomes**, not from direct access to true skill. A rating system that could read true skill would be trivially perfect — it would have nothing to learn. By restricting algorithms to observable data only, MatchLab measures how well an algorithm **infers** truth from limited, noisy evidence.

The outcome model (the game) is the only component that reads ground truth — it uses true skill to decide match winners. This is what makes the simulation meaningful: outcomes depend on reality, ratings depend on outcomes, and the gap between ratings and reality is the quantity being measured.

### The Smurf Example

A "smurf" in MatchLab is not a boolean flag. It is the **combination** of:
- High `true_skill` (in `PlayerReality`)
- Low `initial_rating` (in `PlayerObservation`)
- Few `games_played`

Detection systems must infer smurf status from observable behavior — win streaks, performance anomalies, rapid rating increases. This mirrors real-world detection, where the ground truth (this player is actually skilled) is hidden and must be inferred from evidence.

---

## Simulation Model

### Discrete-Event Simulation

MatchLab uses a discrete-event simulation engine. Time advances in nanosecond resolution (`SimTime`, a `u64`). Events are scheduled on a priority queue and processed in chronological order. The clock jumps directly to the next event — there is no wasted computation during idle periods.

**Event types:**

| Event | Description |
|-------|-------------|
| `PlayerJoin` | A player enters the population |
| `PlayerQueue` | A player enters the matchmaking queue |
| `MatchFormed` | A match is formed by the matchmaker |
| `MatchStart` | A match begins (simulated duration) |
| `MatchEnd` | A match concludes, ratings updated |
| `RatingUpdate` | Rating system processes a match result |
| `DetectionCheck` | Detection system evaluates a player |
| `SkillChange` | Periodic skill evolution (dynamic skill) |
| `MatchTimer` | Periodic matchmaker invocation |
| `PlayerQuit` | A player leaves the population |
| `PlayerDisconnect` | A player goes offline temporarily |
| `PlayerReturn` | A disconnected player returns |

### Seeded Determinism

Every experiment is deterministic given its config + seed. The `SeedManager` derives separate seeds for population generation, game outcomes, player arrivals, behavioral RNG, matchmaker decisions, and the master world RNG from a single experiment seed. Each stochastic subsystem draws from its own RNG stream, so varying one stream (e.g., behavior) while fixing another (e.g., game outcomes) produces controlled comparisons. Note: This determinism applies to individual simulation experiments. The Bayesian optimization loop (`matchlab optimize`) with `batch_size > 1` evaluates experiments concurrently, so the optimization trajectory is not fully deterministic — though each individual experiment within the trajectory is. The CLI `--threads` flag defaults to `num_cpus`, so optimization runs in batch mode by default; pass `--threads 1` to preserve trajectory determinism.

### Multi-Scale Time

Simulation time is internal nanoseconds, but experiments specify duration in seconds. The `max_time` parameter sets a wall-clock bound on simulated time (e.g., 604,800 seconds = 7 days). The `matches` parameter sets a match-count bound. The simulation stops when either bound is reached.

---

## Experiment Design

### Factors and Conditions

A **factor** is an independent variable — something the researcher controls and varies. A **condition** is a specific combination of factor levels. A **cell** in a factorial design is one condition.

**Example factors:**

| Factor | Levels |
|--------|--------|
| Rating system | elo, glicko2, trueskill |
| Matchmaker | random, batch, expanding_window, strict |
| Outcome noise | 0.0, 0.05, 0.15 |
| Population size | 1000, 5000, 10000 |

### Replication

Replication means running the same condition multiple times with different random seeds. This is essential for:
- Estimating variance in outcomes
- Computing confidence intervals
- Testing whether differences between conditions are statistically significant

**Replication strategies:**

| Strategy | Description | Use case |
|----------|-------------|----------|
| `independent` | Each arm gets a distinct seed per replicate | When algorithms alter the simulation trajectory |
| `crn` (Common Random Numbers) | Arms share the replicate seed | Comparing algorithms on identical populations |
| `counterfactual` | One arm runs live and records; others replay | Offline comparison of rating systems on identical histories |

### Estimands

An **estimand** is the precise quantity you want to estimate. MatchLab supports:

| Estimand | Definition |
|----------|------------|
| Absolute difference | E[Y_treatment] − E[Y_control] |
| Relative difference | (E[Y_treatment] − E[Y_control]) / E[Y_control] |
| Percent improvement | 100 × (E[Y_control] − E[Y_treatment]) / E[Y_control] (for lower-is-better metrics) |
| Mean paired difference | Mean of (Y_treatment,i − Y_control,i) across paired replicates |
| Median paired difference | Median of paired differences |

The choice of estimand determines which statistical method is appropriate and how results are interpreted.

---

## Matchmaking Concepts

### Match Quality

Match quality measures how balanced two teams are. In MatchLab, quality is defined as:

```
quality = 1 − (|avg_rating_team_a − avg_rating_team_b| / 400).clamp(0, 1)
```

A quality of 1.0 means perfectly balanced teams (equal average rating). A quality of 0.0 means the maximum measurable imbalance (400+ rating gap).

Quality is computed from **visible ratings only** — never from ground truth skill. This ensures the matchmaker optimizes for what it can see, not what it knows.

### Queue Time

Queue time is the elapsed time between a player joining the queue and being assigned to a match. It is measured as `now − joined_at` at the moment of formation.

Queue time is fundamentally in tension with match quality: tighter quality constraints mean fewer acceptable opponents, which increases wait times.

### The Tradeoff

The core matchmaking research question is: **How much match quality must be sacrificed to reduce queue time by X%?** MatchLab measures both metrics simultaneously, enabling researchers to map the Pareto frontier of this tradeoff.

### Matchmaker Variants

| Matchmaker | Strategy |
|------------|----------|
| `batch` | Collects players, sorts by rating, assigns alternately to teams |
| `expanding_window` | Accepts players within a skill window that widens with wait time |
| `strict` | Only matches players within a fixed skill difference |
| `hub_spoke` | Regional partitioning with hub overflow |
| `random` | Uniform random assignment (no rating balancing) |

---

## Strategic Behavior and Ecosystem Dynamics

### Adversarial Agents

MatchLab supports adversarial player agents that behave strategically:

| Agent | Behavior | Objective |
|-------|----------|-----------|
| `afk` | Goes AFK periodically | Minimize games played |
| `deranker` | Intentionally loses to lower rating | Maintain low rating |
| `win_trader` | Pairs with a partner for guaranteed wins | Win trading |
| `booster` | Plays with a lower-ranked partner and carries them | Maximize partner's rating |
| `rating_farmer` | Quits matches to keep games played low | Maximize win rate |

### Ecosystem Dynamics

The simulation captures feedback loops between components:
- Rating systems affect which players are matched
- Match quality affects player satisfaction
- Satisfaction affects retention (quit probability)
- Retention affects population composition
- Population composition affects match quality

These feedback loops are the core phenomenon the `feedback_loop/` factorial grid studies.

### Population Dynamics

Player populations are not static. Players join, leave, go offline, and return. The `EntryExitDynamics` model controls this. Dynamic skill (when `skill_update_interval_secs` is set) allows player true skill to drift over time, creating a non-stationary environment that rating systems must track.

---

## Analysis and Interpretation

### What MatchLab Can Establish

1. **Relative performance** — Under identical conditions, algorithm A produces lower rating error than algorithm B
2. **Convergence behavior** — Rating accuracy improves over time at rate R
3. **Tradeoff quantification** — Reducing queue time by 50% costs Δ match quality
4. **Sensitivity** — Performance is robust (or sensitive) to parameter variation
5. **Feedback effects** — The choice of matchmaker affects the rating system's convergence

### What MatchLab Cannot Establish

1. **Real-world validity** — MatchLab uses synthetic populations; real player behavior may differ
2. **Causal claims about real systems** — Results hold within the simulation model, not necessarily in production
3. **Optimal parameter values** — Optimal k_factor for Elo on a synthetic population may not be optimal for a real game
4. **Generalizability** — Results are specific to the population model, outcome model, and matchmaker used

### Interpretation Checklist

When interpreting MatchLab results:

- [ ] Are the confidence intervals narrow enough to support the claimed effect?
- [ ] Is the effect size (Cohen's d) practically meaningful, not just statistically significant?
- [ ] Does the result replicate across multiple seeds?
- [ ] Is the population model appropriate for the research question?
- [ ] Are the metrics relevant to the question being asked?

---

## Statistical Methods

### Confidence Intervals

MatchLab computes confidence intervals using several methods, selected by design:

| Method | When used |
|--------|-----------|
| Paired bootstrap | Paired designs (CRN, counterfactual) |
| Welch t-interval | Independent designs with n ≥ 30 |
| Student t-interval | Independent designs with n < 30 |
| Bootstrap | General fallback |

### Effect Sizes

| Measure | Definition | Interpretation |
|---------|------------|----------------|
| Cohen's d | Mean difference pooled SD | Small: 0.2, Medium: 0.5, Large: 0.8 |
| Cohen's d_z | Mean difference paired SD | For paired designs |
| Relative difference | Δ / control mean | Percentage change |

### Multiple Comparisons

When testing multiple metrics simultaneously, MatchLab applies correction:
- **Holm correction** — controls family-wise error rate (FWER)
- **Benjamini-Hochberg** — controls false discovery rate (FDR)

### Power Analysis

MatchLab includes power analysis tools:
- `required_replications` — given an expected effect size and desired power, compute the number of replicates needed
- `achieved_power` — given observed data, compute the achieved statistical power

---

## Known Limitations and Assumptions

### Population Model

- Players are drawn from parametric distributions (normal, log-normal, uniform)
- Archetypes are predefined and do not emerge from behavior
- Skill is a fixed vector sampled at generation (unless dynamic skill is enabled)
- Player behavior (play frequency, session length, quit probability) is archetype-dependent and static

### Outcome Model

- Match outcomes follow a logistic model parameterized by team skill difference
- Performance noise is Gaussian and symmetric
- The model assumes skill is additive across team members (in the default variant)
- No concept of map, role, or meta-game effects

### Rating Systems

- Elo assumes a stationary skill distribution and fixed k-factor
- Glicko-2 assumes a logistic outcome model (consistent with the game model)
- TrueSkill assumes Gaussian performance distributions
- All systems start from cold ratings (no warm-up period)

### Matchmaking

- Batch formation is periodic, not continuous
- Queue dynamics are simplified (no party formation during queue, no dodging)
- The quality metric considers only average team rating, not distribution or role balance

### Statistical

- Confidence intervals assume the metric of interest is well-behaved (finite variance)
- Bootstrap CIs require sufficient replicates (n ≥ 20 recommended)
- Effect sizes are interpreted relative to Cohen's conventions, which may not apply to all metrics

See `docs/assumptions-limitations.md` for a comprehensive treatment of assumptions and threats to validity.

---

## Validation Coverage

| Subsystem | Unit | Property | Known Answer | Integration | E2E |
|-----------|:----:|:--------:|:------------:|:-----------:|:---:|
| Configuration | ✓ | | ✓ | ✓ | |
| Population | ✓ | | ✓ | ✓ | |
| Skill | ✓ | ✓ | ✓ | ✓ | |
| Game/Outcome | ✓ | ✓ | ✓ | ✓ | ✓ |
| Rating | ✓ | ✓ | ✓ | ✓ | ✓ |
| Matchmaking | ✓ | ✓ | ✓ | ✓ | ✓ |
| Detection | ✓ | | ✓ | ✓ | |
| Ecosystem | ✓ | | | ✓ | ✓ |
| Analysis | ✓ | ✓ | ✓ | ✓ | ✓ |
| Plugins (Lua) | ✓ | | ✓ | ✓ | ✓ |
| CLI | ✓ | | ✓ | ✓ | ✓ |
