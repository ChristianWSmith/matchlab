# Independent Reproduction Studies

How to conduct independent reproduction studies of matchlab experiments.

---

## Methodology

### 1. Obtain the exact reproduction artifacts

A reproduction requires three things from the original study:

| Artifact | Where to find it | Why it matters |
|----------|-----------------|----------------|
| `ExperimentResult` JSON | `<output_dir>/<name>.json` | Contains metrics, config hash, git commit |
| `StudyResult` JSON | `<output_dir>/<study_id>.json` | Contains arm assignments, replicate seeds |
| `study_stats.json` | `<output_dir>/study_stats.json` | Contains aggregated CIs, effect sizes |

The `config_hash` and `git_commit` fields in any result JSON identify the exact
code and configuration used.

### 2. Reconstruct the configuration

Load the configuration that produced the result:

```bash
# From a single experiment result
cat results/elo_logistic_5000.json | jq .config_hash

# Check out the exact commit
git checkout <git_commit>

# Verify the config hash matches
cargo run -- run experiments/v0_1_basic.yaml --dry-run 2>&1 | grep config_hash
```

### 3. Replicate with the same seed

The reproduction uses the **same seed** as the original:

```bash
# Run the same experiment with the same seed
cargo run -- run experiments/v0_1_basic.yaml

# Compare results
diff results/original/elo_logistic.json results/reproduction/elo_logistic.json
```

For studies with replication, the seed is derived deterministically:

```
per_replicate_seed = derive(base_seed, replicate_index)
per_arm_seed       = derive(replicate_seed, arm_index)   # independent
per_arm_seed       = replicate_seed                       # CRN
```

### 4. Compare metrics

The comparison is quantitative:

- **Scalar metrics** must match to floating-point precision (1 ULP tolerance for
  byte-identical runs; 1e-6 relative for independent builds).
- **Time series** must have the same number of buckets and matching values.
- **Distribution** metrics must have matching sample counts and summary statistics.

```bash
# Quick comparison
cargo run -- compare results/original.json results/reproduction.json
```

### 5. Document the reproduction

Fill in the template below and commit it alongside the reproduction results.

---

## Reproduction Study Report Template

```markdown
# Reproduction Study: <title>

**Original study:** <reference to original paper/result>
**Date of reproduction:** <YYYY-MM-DD>
**Reproducer:** <name or handle>

## Original Study Details

- **Experiment config:** <path or hash>
- **Git commit:** <full SHA>
- **Config hash:** <hex string>
- **Engine version:** matchlab <version>
- **Seed:** <seed value>

## Environment

- **OS:** <e.g. Ubuntu 22.04>
- **Rust version:** <rustc --version>
- **Platform:** <e.g. x86_64-unknown-linux-gnu>

## Configuration

<Full experiment manifest or study manifest>

## Results

### Scalar Metrics

| Metric | Original | Reproduction | Δ | Status |
|--------|----------|--------------|---|--------|
| rating_accuracy | <value> | <value> | <delta> | PASS/FAIL |
| match_quality | <value> | <value> | <delta> | PASS/FAIL |
| queue_time | <value> | <value> | <delta> | PASS/FAIL |

### Effect Sizes (for studies)

| Comparison | Metric | Original Δ | Reproduction Δ | Cohen's d | Status |
|-----------|--------|-----------|----------------|-----------|--------|
| arm_a vs arm_b | rating_accuracy | <value> | <value> | <value> | PASS/FAIL |

## Discrepancies

<Describe any differences and their likely cause>

## Conclusion

<One paragraph: does the reproduction confirm the original?>

## Artifacts

- Reproduction results: `results/reproduction/<name>.json`
- Diff: `results/reproduction/diff.json`
```

---

## Reproduction Checklist

Use this checklist for every reproduction study:

### Pre-reproduction

- [ ] Obtained the original `ExperimentResult` or `StudyResult` JSON
- [ ] Identified the exact git commit from `git_commit` field
- [ ] Checked out that commit (`git checkout <sha>`)
- [ ] Verified `config_hash` matches by running `--dry-run`
- [ ] Noted the platform, Rust version, and OS
- [ ] Confirmed the seed is available (in the result or manifest)

### Execution

- [ ] Ran the experiment with the same seed
- [ ] Ran with the same number of replicates (for studies)
- [ ] Used the same `--replicates` override if applicable
- [ ] Verified the run completed without errors
- [ ] Collected all output artifacts

### Comparison

- [ ] Compared scalar metrics (within tolerance)
- [ ] Compared time series (same bucket count, matching values)
- [ ] Compared distribution summaries (mean, median, percentiles)
- [ ] Compared effect sizes and confidence intervals (for studies)
- [ ] Compared Cohen's d and relative differences
- [ ] Verified `study_stats.json` structural equivalence

### Documentation

- [ ] Filled in the reproduction report template
- [ ] Documented any discrepancies with likely causes
- [ ] Noted any environment differences
- [ ] Recorded the comparison method (byte-identical vs quantitative)

---

## Handling Discrepancies

### Expected variations (not bugs)

| Variation | Cause | Action |
|-----------|-------|--------|
| Timestamp field differs | Wall-clock time | Ignore — this field legitimately differs |
| Float re-parses 1 ULP off | JSON round-trip | Accept if within machine epsilon |
| `study_stats.json` differs structurally | Timestamp only | Accept if metrics agree |
| Different number of matches formed | Race condition | Investigate — should be deterministic |

### Unexpected variations (bugs)

| Variation | Likely cause | Action |
|-----------|-------------|--------|
| Scalar metrics differ by > 1e-6 relative | Non-determinism or code change | Investigate RNG seeding or config mismatch |
| Different number of replicates completed | Crash or early termination | Check error logs |
| Effect size CI excludes original value | Different random stream | Verify seed derivation |
| `config_hash` mismatch | Code or script change | Check out correct commit |

### Discrepancy resolution workflow

1. **Verify the config hash** — most discrepancies come from using the wrong commit.
2. **Check the seed** — confirm the seed was passed correctly.
3. **Compare environments** — OS-level differences (e.g. allocator) can cause
   floating-point divergence on edge cases, but never > 1 ULP for well-conditioned
   math.
4. **Check for script changes** — the config hash includes Lua script contents;
   if the hash matches, the scripts are identical.
5. **Report the discrepancy** — open an issue with the original result, reproduction
   result, and environment details.

---

## Example: Elo vs Glicko-2 Reproduction

This section walks through reproducing the 2-arm CRN study from
`experiments/studies/elo_vs_glicko.yaml`.

### Original study details

- **Config:** `experiments/studies/elo_vs_glicko.yaml`
- **Base config:** `experiments/base/standard.yaml`
- **Seed:** 42 (default)
- **Replicates:** 50 (CI) or 500 (production)
- **Strategy:** CRN (common random numbers)

### Reproduction steps

```bash
# 1. Check out the commit
git checkout <git_commit_from_result>

# 2. Build
cargo build --release

# 3. Run with the same replicate count
cargo run --release -- study experiments/studies/elo_vs_glicko.yaml --replicates 50

# 4. Compare
cargo run --compare results/original/elo_vs_glicko_study.json \
                     results/reproduction/elo_vs_glicko_study.json
```

### Expected results

From the acceptance run (N=50):

| Metric | Elo (mean) | Glicko-2 (mean) | Δ | 95% CI | Cohen's d |
|--------|-----------|-----------------|---|--------|-----------|
| rating_accuracy | 166.2 | 374.5 | +208.32 | [207.43, 209.28] | 84.12 |

A reproduction within these bounds (accounting for replicate count) confirms
the original finding.

---

## Counterfactual Replay and Reproduction

The counterfactual replay system (`ReplayEngine`) enables a powerful form of
reproduction: replaying the exact same match history through a different rating
system without re-running the simulation.

### How it works

1. **Record:** A recording run captures every match, participant observation, and
   ground-truth snapshot into `GameHistory`.
2. **Replay:** `ReplayEngine::replay` feeds the recorded matches through a new
   rating system, computing what *would have happened* under different algorithms.
3. **Compare:** The replay produces metrics that are directly comparable to the
   original live run.

### Reproduction via counterfactual

```bash
# 1. Run the live arm and record history
cargo run -- study experiments/studies/elo_vs_glicko_replay.yaml --replicates 10

# 2. The glicko2 arm replays the elo arm's history
#    No second run needed — the replay is deterministic from the recording
```

### What counterfactual replay proves

- **Algorithm comparison without simulation variance:** Both systems see the
  exact same matches, eliminating matchmaker and outcome noise.
- **Start-state contract:** The replay seeds initial `RatingState` from the first
  recorded observation, matching the live loop's cold-start behavior.
- **Budget sanitation:** Each replay sanitizes the `MatchResult` through
  `filter_match_result`, so budget-constrained systems never see leaked data.

### Limitations

Counterfactual replay can only replay metrics that depend solely on match
history and ratings. The following metrics are **not replayable**:

- `queue_time` — requires matchmaking state
- `match_quality` — requires queue and matchmaker state
- `ndcg` — requires population ranking
- `population_health` — requires population state
- `match_inequality` — requires population distribution

These are silently dropped during replay with a log message.

---

## Tips for Reproducers

1. **Always use `--release`** for performance-sensitive reproductions. Debug builds
   may have different floating-point behavior (overflow checks, debug assertions).

2. **Pin your Rust version** if reproducing an old study. Edition 2024 behavior
   may change across toolchain versions.

3. **Check the CI** — the `ci.yml` workflow runs the same tests on every commit.
   If CI passed, the code is in a reproducible state.

4. **Use `--replicates` to control runtime.** For quick checks, use 10 replicates.
   For acceptance, use 50+ for CIs or 500+ for publication-quality results.

5. **Save everything.** The result JSON, the config, the git commit, and the
   environment details. Future you will thank present you.
