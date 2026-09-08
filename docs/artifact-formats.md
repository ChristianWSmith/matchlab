# Research Artifact Formats

Schema reference and interpretation guide for matchlab output artifacts.

---

## ExperimentResult JSON Schema

The `ExperimentResult` is the primary output of a single experiment run.

```json
{
  "experiment_id": "string — unique identifier ({name}-{config_hash})",
  "name": "string — human-readable experiment name",
  "config_hash": "string — SHA256 hex digest of the resolved config + all Lua scripts",
  "git_commit": "string — full SHA of the git commit used to run the experiment",
  "timestamp": "string — ISO-8601 UTC timestamp of when the run completed",
  "matches_completed": "integer — total matches that reached MatchEnd",
  "matches_formed": "integer — total matches that were formed (may exceed completed if interrupted)",
  "simulated_time_secs": "float — total simulated time in seconds",
  "metrics": "object — map of metric name to MetricResult",
  "utility_score": "float|null — weighted utility score (null if no objective weights configured)"
}
```

### Required fields

All fields are required. `utility_score` is `null` when no `objectives` are
configured in the manifest.

### Key invariants

- `config_hash` includes the contents of every referenced Lua script. Changing a
  script changes the hash, even if the manifest is identical.
- `matches_completed ≤ matches_formed`. They are equal only when the experiment
  runs to completion without interruption.
- `metrics` is a `BTreeMap`, so JSON serialization key order is deterministic
  across processes and platforms.
- `timestamp` is the only field that legitimately differs between runs of the
  same config + seed (it reflects wall-clock time).

### Example

```json
{
  "experiment_id": "elo_logistic_batch-3a7f2c",
  "name": "elo_logistic_batch",
  "config_hash": "3a7f2c8e91d4b5a6f0e1c2d3b4a5f6e7d8c9b0a1",
  "git_commit": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
  "timestamp": "2026-01-15T12:00:00Z",
  "matches_completed": 10000,
  "matches_formed": 10000,
  "simulated_time_secs": 604800.0,
  "metrics": {
    "rating_accuracy": { "type": "scalar", "value": 162.3 },
    "match_quality": { "type": "summary", "mean": 0.98, "median": 0.99, ... },
    "queue_time": { "type": "scalar", "value": 5.01 }
  },
  "utility_score": 0.742
}
```

---

## StudyResult JSON Schema

The `StudyResult` is the output of a multi-arm replicated study.

```json
{
  "study_id": "string — unique identifier ({name}-{hash8}-{strategy}-r{count})",
  "name": "string — human-readable study name",
  "config_hash": "string — hash of the base (resolved) config",
  "git_commit": "string — full SHA of the git commit",
  "strategy": "string — replication strategy: 'independent', 'crn', or 'counterfactual'",
  "replication_count": "integer — number of replicates per arm",
  "arms": "object — map of arm name to ArmResult"
}
```

### ArmResult

```json
{
  "arm_name": "string — arm identifier",
  "config": "object — the experiment config used for this arm",
  "replicates": [
    {
      "replicate_index": "integer — 0-based replicate number",
      "seed": "integer — seed used for this replicate",
      "result": "ExperimentResult — the full result of this replicate run"
    }
  ]
}
```

### Study ID format

```
{study_name}-{config_hash_first_8_chars}-{strategy}-r{replication_count}
```

Example: `elo_vs_glicko-3a7f2c8e-crn-r50`

### Strategy-specific behavior

| Strategy | Seed derivation | Arm comparison |
|----------|----------------|----------------|
| `independent` | `derive(replicate, arm_index)` per arm | Each arm runs independently |
| `crn` | `derive(replicate, arm_index)` shared across arms | Same random stream for all arms |
| `counterfactual` | Live arm uses replicate seed; replay arms derive | First arm runs live, others replay |

### Example

```json
{
  "study_id": "elo_vs_glicko-3a7f2c8e-crn-r50",
  "name": "elo_vs_glicko",
  "config_hash": "3a7f2c8e91d4b5a6f0e1c2d3b4a5f6e7d8c9b0a1",
  "git_commit": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
  "strategy": "crn",
  "replication_count": 50,
  "arms": {
    "elo": {
      "config": { "...": "..." },
      "replicates": [
        {
          "replicate_index": 0,
          "seed": 42,
          "result": { "...": "ExperimentResult ..." }
        }
      ]
    },
    "glicko2": {
      "config": { "...": "..." },
      "replicates": [
        {
          "replicate_index": 0,
          "seed": 42,
          "result": { "...": "ExperimentResult ..." }
        }
      ]
    }
  }
}
```

---

## StatisticalResult JSON Schema

The `StatisticalResult` is produced by the analysis layer for pairwise
comparisons.

```json
{
  "estimand": {
    "kind": "string — 'absolute_difference' | 'relative_difference' | 'percent_improvement' | 'mean_paired_difference' | 'median_paired_difference'",
    "outcome": "string — metric name (e.g. 'rating_accuracy')",
    "treatment": "string — treatment arm name",
    "control": "string — control arm name"
  },
  "estimate": {
    "value": "float — point estimate of the estimand",
    "ci_lower": "float — lower bound of confidence interval",
    "ci_upper": "float — upper bound of confidence interval",
    "confidence": "float — confidence level (e.g. 0.95)"
  },
  "uncertainty": {
    "method": "string — 'bootstrap' | 'paired_bootstrap' | 'welch_t' | 'student_t' | 'wilson'",
    "iterations": "integer|null — bootstrap iterations (if applicable)",
    "seed": "integer|null — RNG seed for bootstrap (if applicable)"
  },
  "effect_size": {
    "mean_delta": "float — raw difference in means",
    "cohen_d": "float — standardized effect size",
    "d_z": "float|null — within-group standardized effect (paired only)",
    "relative_diff": "float — (treatment - control) / |control|",
    "percent_improvement": "float — relative_diff * 100"
  },
  "sample": {
    "n_treatment": "integer — number of replicates in treatment arm",
    "n_control": "integer — number of replicates in control arm",
    "mean_treatment": "float — mean of treatment replicates",
    "mean_control": "float — mean of control replicates",
    "sd_treatment": "float — standard deviation of treatment replicates",
    "sd_control": "float — standard deviation of control replicates"
  },
  "p_value": "float|null — two-sided p-value (if computed)",
  "family": "string — 'primary' | 'secondary' | 'exploratory'",
  "provenance": {
    "study_id": "string — the study this result belongs to",
    "config_hash": "string — config hash of the base config",
    "git_commit": "string — full SHA",
    "engine_version": "string — matchlab version",
    "design": "string — 'crn' | 'independent' | 'counterfactual' | 'paired' | 'blocked'",
    "aggregation": "string — 'per_replication_scalar'",
    "method": "string — CI method used",
    "confidence": "float — confidence level",
    "bootstrap_iterations": "integer|null",
    "seed": "integer|null",
    "n_replications": "integer — total replicates used"
  }
}
```

---

## MetricResult Types

`MetricResult` is an enum with four variants:

### Scalar

A single numeric value.

```json
{
  "type": "scalar",
  "value": 162.3
}
```

Used for: aggregate metrics computed over all matches (e.g. final MAE, mean
queue time).

### Distribution

Raw sample data with summary statistics.

```json
{
  "type": "distribution",
  "samples": [150.0, 162.3, 145.0, ...],
  "summary": {
    "n": 10000,
    "mean": 162.3,
    "median": 160.0,
    "p75": 175.0,
    "p90": 195.0,
    "p95": 210.0,
    "p99": 250.0,
    "stddev": 25.0
  }
}
```

Used for: per-match or per-player metrics where the full distribution matters
(e.g. individual match quality scores, per-player rating errors).

### Summary

Pre-computed summary statistics (no raw samples).

```json
{
  "type": "summary",
  "mean": 0.98,
  "median": 0.99,
  "p75": 1.0,
  "p90": 1.0,
  "p95": 1.0,
  "p99": 1.0,
  "stddev": 0.02
}
```

Used for: metrics where the distribution is summarized to save space.

### TimeSeries

Bucketed time-series data.

```json
{
  "type": "time_series",
  "bucket_means": [250.0, 200.0, 180.0, 165.0, 162.3]
}
```

Used for: convergence metrics (e.g. `rating_accuracy_by_time`). Bucket
boundaries are defined by the collector's `time_buckets()` function:

| Default buckets (seconds) | Meaning |
|---------------------------|---------|
| 60 | 1 minute |
| 300 | 5 minutes |
| 900 | 15 minutes |
| 3600 | 1 hour |
| 86400 | 1 day |
| 604800 | 1 week |

---

## Provenance Fields

Every artifact carries provenance fields that enable exact reproduction:

| Field | Location | Purpose |
|-------|----------|---------|
| `config_hash` | ExperimentResult, StudyResult, StatisticalResult | Identifies the exact config + scripts used |
| `git_commit` | ExperimentResult, StudyResult, StatisticalResult | Identifies the exact code version |
| `timestamp` | ExperimentResult | When the run completed (wall-clock) |
| `engine_version` | StatisticalResult.provenance | Matchlab version (from Cargo.toml) |
| `seed` | ExperimentResult (in config), StatisticalResult.provenance | RNG seed for the run |
| `experiment_id` | ExperimentResult | Unique run identifier |
| `study_id` | StudyResult, StatisticalResult.provenance | Unique study identifier |

### Config hash computation

The config hash is a SHA256 digest over:

1. The serialized experiment config (all fields, flattened).
2. The contents of every referenced Lua script (resolved from `plugins/`).
3. The base config contents (if using inheritance).

Changing *anything* about the experiment — a parameter, a script, or the base
config — changes the hash.

### Git commit

The `git_commit` is the full 40-character SHA of the HEAD commit at the time
the experiment was run. It is embedded at compile time and is always available:

```rust
// In Cargo.toml
[build-dependencies]
vergen = { version = "8", features = ["build", "cargo", "git", "gitcl"] }
```

---

## How to Interpret Artifacts

### Reading a single ExperimentResult

1. Check `config_hash` and `git_commit` — these identify the exact setup.
2. Check `matches_completed` — should match the manifest's `duration.matches`.
3. Look at the primary metric (e.g. `rating_accuracy` for rating system
   comparison).
4. Compare `utility_score` across experiments when objective weights are set.

### Reading a StudyResult

1. Check `strategy` — `crn` means common random numbers (lower variance),
   `independent` means separate runs (higher variance but simpler).
2. Check `replication_count` — more replicates = tighter CIs. 50+ for
   publication quality.
3. Compare arms by looking at `study_stats.json` (generated alongside the
   study result).

### Reading StatisticalResult

1. Check `estimate.value` — the point estimate of the difference.
2. Check `estimate.ci_lower` and `estimate.ci_upper` — the confidence interval.
   If the interval excludes 0, the difference is statistically significant at
   the given confidence level.
3. Check `effect_size.cohen_d` — practical significance. Rules of thumb:
   - 0.2 = small
   - 0.5 = medium
   - 0.8 = large
   - > 2.0 = very large (common in matchmaking research where algorithm
     differences are stark)
4. Check `p_value` — probability of observing this difference (or larger) under
   the null hypothesis. Values < 0.05 are conventionally significant.
5. Check `uncertainty.method` — `paired_bootstrap` is preferred for CRN studies;
   `welch_t` for independent samples with n ≥ 30.

### Reading MetricResult

| Type | When to use | What to look at |
|------|-------------|-----------------|
| Scalar | Final aggregate | The `value` field |
| Distribution | Per-sample analysis | `summary.mean`, `summary.stddev`, `summary.p95` |
| Summary | Pre-summarized data | Same fields as Distribution summary |
| TimeSeries | Convergence analysis | Trend of `bucket_means` (should decrease for accuracy) |

### Comparing across runs

When comparing two `ExperimentResult` JSONs:

1. **Same config hash** → same config + scripts, different seed or environment.
2. **Different config hash** → different experiment setup; compare carefully.
3. **Same git commit** → same code version.
4. **Different git commit** → code may have changed; check the changelog.

---

## Versioning Strategy

### Semantic versioning

matchlab follows SemVer:

- **Major** (1.0 → 2.0): Breaking changes to artifact formats, config schema,
  or Lua script contracts.
- **Minor** (1.0 → 1.1): New features, new metric types, new fields (backward
  compatible).
- **Patch** (1.0.0 → 1.0.1): Bug fixes, documentation improvements.

### Artifact format versioning

Each artifact type has an implicit version tied to the engine version:

| Field | Versioning rule |
|-------|----------------|
| `ExperimentResult` | New fields added in minor versions; fields never removed |
| `StudyResult` | New fields added in minor versions; fields never removed |
| `StatisticalResult` | New fields added in minor versions; fields never removed |
| `MetricResult` | New variants added in minor versions; variants never removed |
| `config_hash` | Changes when config format changes (scripts or parameters) |

### Backward compatibility

- **Consumers** must tolerate unknown fields (ignore them, don't error).
- **Producers** must not remove fields or change their types.
- **New fields** must have sensible defaults (null for optional, empty for
  collections).

### Forward compatibility

- **Consumers** should check `engine_version` and warn if the artifact was
  produced by a significantly older version.
- **New metric types** (new `MetricResult` variants) may not be supported by
  older consumers. Consumers should handle `Unknown` variants gracefully.

### Migration

When a breaking change is made:

1. Bump the major version.
2. Document the migration in `CHANGELOG.md`.
3. Provide a conversion script if the format change is mechanical (e.g. field
   rename).
4. Support reading old formats for at least one major version cycle.
