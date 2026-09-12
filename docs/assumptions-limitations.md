# Assumptions, Limitations & Threats to Validity

This document catalogues the scientific assumptions underlying MatchLab's simulation model, identifies the limitations of those assumptions, and classifies threats to the validity of conclusions drawn from MatchLab experiments. It is intended for researchers who need to assess whether MatchLab results support claims about real-world matchmaking systems.

---

## Synthetic Population Assumptions

### Distributional Assumptions

Player skill is drawn from parametric distributions: normal, log-normal, or uniform. These are configured per-archetype via `skill_distribution`.

**Assumption:** The chosen distribution adequately represents the skill distribution of a real player population.

**Limitation:**
- Real player skill distributions are rarely Gaussian. They may be multimodal (casual vs competitive players), heavy-tailed (a small number of extremely skilled players), or truncated (skill floors and ceilings imposed by game design).
- Log-normal may better represent skill in some games (many low-skill players, fewer high-skill), but the parameters must be calibrated.
- Uniform distributions are unrealistic but useful for controlled experiments.

**Threat:** If the simulated distribution differs structurally from the real population, rating system performance rankings may not transfer.

### Archetype Assumptions

Players are assigned to discrete archetypes (e.g., "stable", "improving", "smurf") with fixed proportions. Each archetype has independent parameters for skill distribution, play frequency, session length, and quit probability.

**Assumption:** Real player populations can be meaningfully segmented into a small number of behavioral archetypes.

**Limitation:**
- Real players exhibit continuous, overlapping behavioral spectra — not discrete categories.
- Archetype proportions are set by the researcher and may not match any real population.
- Within-archetype variance is modeled; between-archetype behavioral heterogeneity is not (all "stable" players have the same `play_frequency`).
- Archetype assignment is static — players never change archetype.

**Threat:** Archetype-based populations may produce emergent behaviors (e.g., matching patterns) that do not occur in real populations.

### Population Size Assumptions

Experiments typically use 2,000–10,000 players. The v0.1 baseline uses 10,000.

**Assumption:** The simulated population is large enough for statistical properties to stabilize.

**Limitation:**
- Real matchmaking systems serve millions of players. Queue dynamics, match quality distributions, and rating convergence behave differently at scale.
- Small populations may produce artifacts (e.g., the same players repeatedly matching each other).

### Smurf Modeling

Smurfs are modeled as players with high `true_skill` but low `initial_rating`. This is the only mechanism for creating a skill-rating mismatch at population generation.

**Assumption:** This captures the essential structure of smurfing.

**Limitation:**
- Real smurfs may also have unusual behavioral signatures (high kill/death ratios, fast game completion, unusual play patterns) not captured by the archetype model.
- Real smurfing may involve account sharing, queue manipulation, or intentional deranking — behaviors not modeled.

---

## Skill Model Assumptions

### Static vs Dynamic Skill

By default, player skill is **static** — sampled once at population generation and never changing. Dynamic skill is enabled via `game.skill_update_interval_secs`, which causes periodic `SkillChangeEvent`s that advance each online player's true skill.

**Assumption:** Static skill is a reasonable approximation for the duration of the experiment (typically 7 days sim time).

**Limitation:**
- Real players improve, decline, and fluctuate in skill over time.
- Dynamic skill in MatchLab uses linear drift (`val + improvement_rate + N(0, volatility)`), which is a simplification of real learning curves (which may be logarithmic, step-function, or context-dependent).
- Only online players receive skill updates; offline players maintain their last skill level.

### Multidimensional Skill

Skill can be represented as a `SkillVector` with named dimensions (e.g., "aim", "game_sense", "teamwork"). The `SkillDimension` type supports configurable scale, min, and max values.

**Assumption:** Skill can be decomposed into independent or correlated dimensions.

**Limitation:**
- v0.1 implementations use a single `overall` dimension.
- Multidimensional skill is supported by the `composition.lua` outcome model but requires the researcher to define dimensions and correlations.
- The relationship between skill dimensions and match outcomes is researcher-specified, not empirically derived.

### Skill as a Continuous Quantity

Skill is a continuous f64 value. There are no discrete skill levels or tiers in the ground truth.

**Assumption:** Continuous skill adequately represents real player ability.

**Limitation:**
- Some games have discrete skill tiers (bronze, silver, gold) that may affect player behavior in ways not captured by continuous skill.

---

## Outcome Model Assumptions

### Logistic Model

The default outcome model (`logistic.lua`) computes win probability as:

```
P(A wins) = 1 / (1 + exp(-(skill_a - skill_b) / beta))
```

where `beta` is a scaling parameter (default 400).

**Assumption:** The logistic function accurately models the relationship between skill difference and win probability.

**Limitation:**
- The logistic curve is symmetric and monotonically increasing — it assumes that a given skill gap produces the same advantage regardless of absolute skill level.
- Real game outcomes may be asymmetric (e.g., a 100-point skill gap matters more at low skill than at high skill).
- The logistic model does not account for game-specific mechanics (draft, map, role).

### Performance Noise

Match outcomes include Gaussian noise controlled by the `noise` parameter. Higher noise produces more upsets.

**Assumption:** Performance noise is Gaussian, symmetric, and independent across matches.

**Limitation:**
- Real performance noise may be skewed (players occasionally have very bad games but rarely have proportionally very good games).
- Noise may correlate with skill level (lower-skill players may have higher variance).
- Noise may correlate within teams (team chemistry effects).

### Team Composition

The default team model is additive — team strength is the sum of member skills. Variants include weighted, complementary (synergy), and fatigue models.

**Assumption:** Additive team strength is a reasonable approximation.

**Limitation:**
- Real team performance depends on role balance, communication, synergy, and coordination — none of which are modeled in the additive case.
- The complementary model adds a synergy bonus for diverse skill profiles, but the formula is researcher-specified.

### Information Budget

Rating systems declare an `information_budget` — the types of match data they can observe. The loop sanitizes `MatchResult` before calling `update`, stripping disallowed data.

**Assumption:** The budget categories (WinLoss, Score, PerformanceData, etc.) adequately represent the data available to real rating systems.

**Limitation:**
- Real systems may have access to continuous in-game data (damage dealt, objectives completed, time spent dead) that does not map cleanly to the budget categories.
- The budget model is binary (data is either allowed or stripped), not noisy.

---

## Matchmaking Model Assumptions

### Batch Formation

The default matchmaker collects players into a queue and forms matches periodically (every `batch_interval` ticks). Players are sorted by rating and assigned alternately to teams.

**Assumption:** Periodic batch formation approximates real matchmaking behavior.

**Limitation:**
- Real matchmaking systems typically form matches continuously as suitable opponents are found, not in fixed-interval batches.
- Batch formation introduces artificial synchronization — all players in a batch are matched at the same time, regardless of when they joined.
- The batch interval affects queue time metrics: shorter intervals reduce wait but may reduce match quality.

### Queue Dynamics

Players join the queue, wait, and are matched. Queue time is measured as `now − joined_at` at formation.

**Assumption:** The queue operates as a simple FIFO with no priority, no dodging, and no party formation during wait.

**Limitation:**
- Real queues have priority tiers, connection quality filtering, and geographic constraints.
- Players may dodge (cancel) queue entries or leave matches early.
- Party formation during queue wait is not modeled.

### Quality Metric

Match quality is `1 − (|avg_a − avg_b| / 400).clamp(0, 1)`, computed from average team ratings.

**Assumption:** Average team rating difference is a sufficient measure of match balance.

**Limitation:**
- Two teams with the same average rating but different distributions (e.g., one balanced team vs one team with a carry and four weak players) may feel very different to play.
- The metric does not account for role balance, party size, or skill dimension distribution.
- The 400-point normalization constant is arbitrary and game-specific.

### Max Queue Time

Experiments set `max_queue_time` (default 60 seconds sim time). Players exceeding this wait are matched regardless of quality.

**Assumption:** 60 seconds is a reasonable maximum acceptable wait.

**Limitation:**
- Real acceptable wait times vary by game, platform, time of day, and player population.
- The hard cutoff creates a discontinuity in the queue time distribution.

---

## Strategic Agent Assumptions

### Simplified Behaviors

Adversarial agents implement simplified behavioral models:
- AFK agents go AFK with a fixed probability
- Derankers quit matches above a target rating
- Win traders form pairs and trade wins
- Boosters carry a partner

**Assumption:** These simplified behaviors capture the essential dynamics of strategic play.

**Limitation:**
- Real strategic behavior is adaptive — players respond to detection, change strategies, and coordinate in complex ways.
- The agents do not learn or adapt within a session.
- Agent parameters are fixed and set by the researcher, not derived from empirical data.
- Real adversaries may be more sophisticated (e.g., losing intentionally by a small margin to avoid detection, not just quitting).

### Satisfaction and Retention

The satisfaction model uses a weighted sum of recent match experiences to compute a retention probability.

**Assumption:** Player retention is a logistic function of recent satisfaction.

**Limitation:**
- Real player retention is influenced by many factors outside the game (competition, life events, content updates).
- The satisfaction weights are researcher-specified, not empirically calibrated.
- The model does not capture network effects (friends quitting causes others to quit).

---

## Statistical Assumptions

### Independence

Confidence intervals and hypothesis tests assume that replicates are independent.

**Assumption:** Each replicate produces an independent sample of the metric of interest.

**Limitation:**
- With CRN (Common Random Numbers), replicates within an arm are not independent — they share the same population and arrival sequence. This is by design (it reduces variance), but standard independent-sample CIs are inappropriate. MatchLab uses paired methods for CRN designs.
- Replicates across arms in a CRN study are paired, not independent.

### Normality

Welch t-intervals assume the metric distribution is approximately normal.

**Assumption:** The sampling distribution of the metric is approximately Gaussian.

**Limitation:**
- Some metrics (e.g., queue time) may be highly skewed, especially with batch matchmakers.
- Small replicate counts (n < 30) may not satisfy the central limit theorem.
- MatchLab falls back to bootstrap CIs when n < 30, but bootstrap also requires that the empirical distribution is representative.

### Sufficient Replication

Confidence interval width scales as 1/√n. Small replicate counts produce wide intervals that may not resolve meaningful effects.

**Assumption:** The replicate count is sufficient for the expected effect size.

**Limitation:**
- A pilot study with 10 replicates may fail to detect effects that 500 replicates would reveal.
- Power analysis (`required_replications`) can estimate the needed count, but requires an assumed effect size.

### Metric Stability

Some metrics converge slowly or are sensitive to outliers.

**Assumption:** The metric of interest is stable enough to estimate with the available replicates.

**Limitation:**
- `match_quality` may be sensitive to a few highly imbalanced matches in the tail.
- `queue_time` may be skewed by a small number of players who wait much longer than average.
- `rating_accuracy` depends on the rating system's convergence rate, which varies by population and configuration.

### Optimization Trajectory Determinism

Bayesian optimization with `batch_size > 1` evaluates multiple experiments concurrently via rayon. This means the order in which trial results are added to the GP model is non-deterministic, which can affect the optimization trajectory.

**Assumption:** The optimization trajectory is reproducible given the same seed and config.

**Limitation:**
- When `batch_size > 1`, concurrent evaluation order varies across runs, producing different optimization trajectories even with the same seed.
- Each individual experiment within the trajectory remains fully deterministic.
- The final Pareto set may differ across runs due to trajectory divergence.
- The CLI `--batch-size` flag defaults to `num_cpus`, so `matchlab optimize` runs in batch mode by default. Pass `--batch-size 1` or set `batch_size: 1` in YAML to preserve full trajectory determinism.

**Threat:** If optimal hyperparameters depend on a specific optimization trajectory, researchers should run multiple optimization replicates and report the distribution of solutions rather than a single run.

---

## Simulation-to-Reality Gap

### What Transfers

The following properties of MatchLab results are likely to transfer to real systems:

1. **Relative algorithm performance** — If Elo outperforms Glicko-2 in MatchLab, it is likely (but not certain) to outperform in similar real conditions.
2. **Qualitative tradeoff direction** — Reducing queue time constraints will degrade match quality. The direction of this tradeoff is robust.
3. **Sensitivity to parameters** — If a rating system is sensitive to k_factor in MatchLab, it will likely be sensitive in production.
4. **Feedback loop dynamics** — The qualitative behavior of feedback loops (e.g., matchmaker choice affecting rating convergence) is likely to hold.

### What Does Not Transfer

The following properties are simulation-specific and should not be directly applied to real systems:

1. **Absolute metric values** — A rating accuracy of 162 in MatchLab does not mean a real system would achieve 162.
2. **Optimal parameter values** — The optimal k_factor for Elo on a synthetic N(1000,250) population is not necessarily optimal for a real game.
3. **Effect sizes** — Cohen's d values are specific to the population model and may differ in magnitude in real systems.
4. **Queue time values** — Queue times depend on population size, arrival rate, and matchmaker implementation — all of which are researcher-specified.
5. **Convergence rates** — How many matches Elo needs to converge depends on the population structure, which is simulated.

### Key Gaps Between Simulation and Reality

| Simulation | Reality |
|------------|---------|
| Parametric skill distributions | Empirical, often multimodal distributions |
| Static archetypes | Continuous, evolving player behavior |
| Additive team strength | Complex role/synergy/coordination effects |
| Periodic batch matching | Continuous match formation |
| No network effects | Friends, communities, social influence |
| No content updates | Patches, balance changes, new content |
| No matchmaking ranks/tiers | Visible rank affects player behavior |
| No connection quality | Latency, packet loss, hardware differences |

---

## Threats to External Validity

External validity concerns whether MatchLab results generalize to real-world matchmaking systems.

1. **Population validity** — Synthetic populations may not represent real player skill distributions, behavioral patterns, or demographic composition.
2. **Ecological validity** — The simulation environment (no voice chat, no toxicity, no content updates) differs from real gaming environments.
3. **Temporal validity** — Results hold for the simulated duration (typically 7 days) and may not generalize to longer time horizons.
4. **Algorithm validity** — The Lua script implementations of rating systems are faithful to the mathematical specifications but may differ from production implementations in edge cases.

---

## Threats to Internal Validity

Internal validity concerns whether the observed effects are caused by the manipulated factor, not by confounds.

1. **Seed sensitivity** — Results may be specific to the chosen seed. Replication across multiple seeds mitigates this.
2. **Population composition** — Archetype proportions directly affect results. Changing proportions may change which algorithm performs best.
3. **Matchmaker-rating interaction** — The matchmaker determines which matches the rating system sees. Different matchmakers produce different learning trajectories.
4. **Metric selection** — The choice of metrics determines what is measured. A metric that favors one algorithm may not capture aspects that favor another.
5. **Duration effects** — Short experiments may show different rankings than long experiments (e.g., Elo may start strong but Glicko-2 may converge better over time).
6. **Confound with population size** — Larger populations produce different queue dynamics than smaller ones, potentially interacting with the factor being tested.

---

## Recommended Caveats for Researchers

When reporting MatchLab results, include the following caveats:

### Always State

1. **The population model** — "Results hold for a synthetic population of N players drawn from [distribution] with [archetype descriptions]."
2. **The outcome model** — "Match outcomes follow a logistic model with parameter β = [value] and noise σ = [value]."
3. **The replication strategy** — "N replicates using [CRN/independent/counterfactual] seeding."
4. **The confidence level** — "95% confidence intervals computed via [method]."

### When Making Comparative Claims

5. **Effect size, not just significance** — Report Cohen's d alongside p-values. A statistically significant difference with d = 0.1 may not be practically meaningful.
6. **Precision** — Report CI width. A wide CI means the estimate is imprecise, even if the point estimate suggests a large effect.
7. **Replication count** — State the number of replicates. Fewer than 30 replicates may produce unreliable CIs for non-normal metrics.

### When Generalizing

8. **Qualify generalizability** — "These results suggest that [algorithm] may outperform [algorithm] under similar conditions, but real-world validation is needed."
9. **Acknowledge the gap** — "MatchLab uses synthetic populations and does not model [real-world factor]. Results should be validated against production data."
10. **Report sensitivity** — If you varied parameters and the ranking changed, say so. "Elo outperformed Glicko-2 for k_factor ∈ [16, 48] but not for k_factor = 8."

### When Presenting Tradeoffs

11. **Map the frontier** — Show the full queue-time vs match-quality curve, not just a single operating point.
12. **State the context** — "Reducing queue time by 50% cost [Δ] match quality under [population description] with [matchmaker]."
13. **Acknowledge dimensionality** — "This tradeoff was measured on [metric]. Other tradeoffs (e.g., detection accuracy vs false positive rate) were not evaluated."

---

## Summary of Key Assumptions

| Component | Core Assumption | Severity | Mitigation |
|-----------|-----------------|----------|------------|
| Population | Parametric skill distributions | High | Use empirical distributions when available |
| Population | Discrete behavioral archetypes | Medium | Increase archetype count, use continuous models |
| Skill | Static unless dynamic enabled | Low | Enable dynamic skill for temporal studies |
| Outcome | Logistic win probability | Medium | Validate against real win-rate curves |
| Outcome | Gaussian performance noise | Low | May not affect relative comparisons |
| Matchmaker | Batch formation | Medium | Compare against continuous matchmakers |
| Quality | Average rating difference | Medium | Develop richer quality metrics |
| Agents | Simplified strategic behavior | High | Validate against real adversarial data |
| Statistics | Independent replicates | Low (with CRN: medium) | Use paired methods for CRN designs |
| Statistics | Sufficient replication | Medium | Run power analysis before experiments |

The severity rating reflects how much each assumption is likely to affect the transferability of results to real-world systems. High-severity assumptions warrant the most caution when generalizing.
