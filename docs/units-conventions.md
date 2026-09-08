# Units & Numerical Conventions

All units, scales, and numerical conventions used across the matchlab codebase, manifests, and analysis layer.

---

## Time

| Context | Unit | Range | Notes |
|---------|------|-------|-------|
| `SimTime` (internal) | nanoseconds (`u64`) | 0 – 2^64 | Monotonic, never wraps. Event engine skips idle periods. |
| `SimTime::from_secs` | seconds → nanoseconds | — | Multiply by 1_000_000_000. |
| `SimTime::as_secs_f64` | nanoseconds → seconds | — | Divide by 1_000_000_000. Returns `f64`. |
| Manifests (`duration.max_time`) | seconds (`u64`) | — | Parsed as seconds, converted internally. |
| Manifests (`skill_update_interval_secs`) | seconds (`f64`) | — | Optional; absent means static skill. |
| Queue time metric | seconds (`f64`) | ≥ 0 | `now - joined_at` at formation time. Saturating. |
| Match duration | seconds (`f64`) | > 0 | Set by outcome model script. Stored in `MatchResult.duration`. |
| `time_buckets` (metrics) | seconds (`f64`) | — | Bucket edges for time-series metrics (e.g., `rating_accuracy_by_time`). |

---

## Ratings & Skill

| Quantity | Unit | Scale | Default | Notes |
|----------|------|-------|---------|-------|
| Rating | points | Centered at 1000 | 1000 | Elo, Glicko-2, TrueSkill all use this center. |
| Rating deviation (RD) | points | 0 – ∞ | 350 (Elo) | Lower = more confident. Glicko-2 initializes from archetype. |
| Volatility | dimensionless | 0 – ∞ | varies | Glicko-2 only. Per-player, evolves via Newton iteration. |
| Skill (1D) | points | Same scale as rating | — | `SkillVector.overall()`. Drawn from archetype distribution. |
| Skill (multidimensional) | points per dimension | `min..max` per dimension | — | Normalized to [0, 1] internally, denormalized for display. |
| Improvement rate | points per step | 0 – ∞ | 0 | Added to skill each `skill_update_interval`. Zero = static. |
| Skill volatility | points per step | 0 – ∞ | 0 | Stddev of noise added each step. Zero = deterministic trajectory. |

**Rating-to-skill gap convention:** A 400-point gap corresponds to a ~90.9% win probability under the logistic model (analogous to Elo's traditional 800-point scale for 90%). The Elo divisor `k = beta * ln(10)` keeps the log10 scale consistent.

---

## Probabilities

| Quantity | Unit | Range | Notes |
|----------|------|-------|-------|
| Win probability | fraction | [0, 1] | Logistic of skill difference. Clamped to [0.01, 0.99]. |
| Retention probability | fraction | [0, 1] | Logistic of satisfaction score. |
| Rematch probability | fraction | [0, 1] | Logistic of satisfaction (higher threshold). |
| Quit probability | fraction | [0, 1] | Per-player, set by archetype or adversarial agent. |
| Draw probability | fraction | [0, 1] | TrueSkill parameter; 0 for win/loss only. |
| Anomaly probability | fraction | [0, 1] | Smurf detector's per-player evidence score. |
| Intervention threshold | fraction | [0, 1] | Detection ladder thresholds (0.3 – 0.99). |
| `go_afk_probability` | fraction | [0, 1] | AFK agent parameter. |

---

## Match Quality

| Quantity | Unit | Range | Formula |
|----------|------|-------|---------|
| Match quality | fraction | [0, 1] | `1 - (\|avg_a - avg_b\| / 400).clamp(0, 1)` |
| 1.0 | — | Perfect balance | Teams have equal average ratings. |
| 0.0 | — | Maximum imbalance | Teams are 400+ points apart. |

---

## Effect Sizes (Cohen's d)

| Category | Cohen's d | Interpretation |
|----------|-----------|----------------|
| Small | \|d\| < 0.2 | Negligible practical difference |
| Medium | 0.2 ≤ \|d\| < 0.5 | Small but noticeable |
| Large | 0.5 ≤ \|d\| < 0.8 | Clear practical difference |
| Very large | \|d\| ≥ 0.8 | Dominant effect |

**Convention:** d = (mean_treatment - mean_control) / pooled_stddev. Positive d means treatment is higher.

---

## Confidence Intervals

| Quantity | Unit | Range | Default | Notes |
|----------|------|-------|---------|-------|
| Confidence level | fraction | (0, 1) | 0.95 | 95% CI is the standard. |
| Alpha (α) | fraction | (0, 1) | 0.05 | α = 1 - conf. |
| Bootstrap iterations | integer | ≥ 1000 | 1000 | More iterations = more stable CI bounds. |

---

## Population & Proportions

| Quantity | Unit | Range | Notes |
|----------|------|-------|-------|
| Population size | integer | ≥ 1 | Total players generated. |
| Archetype proportion | fraction | [0, 1] | Sum must equal 1.0. Converted to integer counts via largest-remainder method. |
| Team size | integer | ≥ 1 | Players per side. Default 5v5. |
| Party size limit | integer | ≥ 1 | Max players in a party. Solo = party of 1. |

---

## Latency

| Quantity | Unit | Range | Notes |
|----------|------|-------|-------|
| Player latency | milliseconds (`u32`) | 0 – ∞ | Stored in `QueueEntry.latency_ms`. |
| Latency threshold | fraction | [0, 1] | `MaxLatencyConstraint` threshold relative to max. |
| Region-to-region latency | milliseconds | 0 – ∞ | Configured via `LatencyMatrix`. |

---

## Elo-Specific Constants

| Constant | Value | Notes |
|----------|-------|-------|
| K-factor | `beta * ln(10)` | Keeps log10 scale consistent with logistic model. |
| Beta | Configurable (default varies) | Base divisor before log10 scaling. |
| Initial rating | 1000 | Configurable per archetype via `initial_rating`. |
| Rating floor | 100 | Clamp minimum in some scripts. |
| Rating ceiling | 3000 | Clamp maximum in some scripts. |

---

## Glicko-2 Constants

| Constant | Value | Notes |
|----------|-------|-------|
| Scale factor | 173.7178 | Converts between Glicko and Glicko-2 scales. |
| Center | 1500 | Glicko-2 internal center. |
| Convergence tolerance | 0.000001 | Newton-Raphson volatility iteration stop condition. |
| Max volatility iterations | 50 | Safety cap on Newton iteration. |

---

## TrueSkill Constants

| Constant | Value | Notes |
|----------|-------|-------|
| Initial variance | Derived from RD | `rating_deviation = sqrt(variance)`. |
| Draw margin | `u * sqrt(2) * beta` | From `draw_probability`; 0 for win/loss only. |
| Beta | Configurable | Performance stddev per player. |

---

## Metric Result Types

| Type | Internal representation | Notes |
|------|------------------------|-------|
| `Scalar` | `f64` | Single value. |
| `Summary` | `{ n, mean, median, p75, p90, p95, p99, stddev }` | Nearest-rank percentiles. |
| `Distribution` | `Vec<f64>` (samples) | Raw sample values; serialized as array. |
| `Histogram` | `{ buckets: Vec<(f64, u64)> }` | Pre-bucketed counts. |
| `TimeSeries` | `{ bucket_means: Vec<f64> }` | One value per time bucket. |

---

## Serialization Conventions

| Context | Format | Notes |
|---------|--------|-------|
| Experiment results | JSON | `BTreeMap` keys for deterministic key order. |
| Study results | JSON | `study_stats.json` + `<study_id>.json`. |
| Manifests | YAML | Parsed via `serde_yaml 0.9`. |
| Config hash | `DefaultHasher` | Over length-prefixed serialized fields + Lua script contents. |
| Timestamps | ISO-8601 UTC | Hand-rolled (no chrono dependency). |

---

## Numerical Safety

- All probability outputs are clamped to [0.01, 0.99] to avoid log(0) or division by zero.
- Queue time is computed via saturating subtraction (`duration_since`), preventing underflow.
- Skills are floored at 0 in `SkillProcess::advance`.
- Match quality is clamped to [0, 1] after the raw calculation.
- NaN and ±Inf are forbidden in all finalized metrics and rating states; validated by `SanityRatingSystem` in invariant tests.
- The Newton-Raphson volatility iteration in Glicko-2 is bracketed with upper/lower bounds and a convergence tolerance of 1e-6.
