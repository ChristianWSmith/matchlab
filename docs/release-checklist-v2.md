# 1.0 Release Checklist

Comprehensive release checklist for matchlab 1.0.

---

## 1. Correctness

### Known-answer tests

- [ ] Elo rating update matches analytic formula for known input/output pairs
- [ ] Glicko-2 rating update matches Glickman (2012) worked example at 1e-6 relative
- [ ] TrueSkill update matches truncated-Gaussian conditioning at 1e-6 relative
- [ ] Logistic outcome model matches `1 / (1 + exp(-diff/400))` at machine precision
- [ ] Match quality formula matches `1 - (|avg_a - avg_b| / 400).clamp(0, 1)`
- [ ] Queue wait time is exactly `now - joined_at` in ticks
- [ ] Simulation time is nanosecond resolution and monotonic

### Property tests

- [ ] `win_probability` always returns a value in [0.0, 1.0]
- [ ] `match_quality` always returns a value in [0.0, 1.0]
- [ ] `SimTime` arithmetic never overflows for values < 584 years
- [ ] `PlayerId` and `MatchId` are unique within a simulation
- [ ] Rating accuracy MAE decreases over time on a stationary population
- [ ] Queue time is non-negative for all matches
- [ ] Population count is conserved (no phantom players)

### Limiting cases

- [ ] Zero-noise outcome model produces deterministic results
- [ ] Zero-improvement population has static skills (no drift)
- [ ] Single-player population forms zero matches
- [ ] Empty queue produces zero matches
- [ ] All-identical ratings produce match quality ≈ 1.0
- [ ] Max-skill-difference ratings produce match quality ≈ 0.0

### Invariants

- [ ] Truth separation: no rating/matchmaking/detection code accesses `PlayerReality` fields
- [ ] Budget separation: rating systems receive sanitized `MatchResult` per their `information_budget`
- [ ] Determinism: same seed + same config → byte-identical results
- [ ] Population conservation: `matches_completed ≤ matches_formed`
- [ ] Time conservation: `simulated_time_secs` reflects the actual sim clock

---

## 2. Code Quality

### Formatting and linting

- [ ] `cargo fmt --check` passes with no diffs
- [ ] `cargo clippy --workspace -- -D warnings` passes with zero warnings
- [ ] No `#[allow(...)]` except for documented, justified cases
- [ ] No dead code (`cargo clippy --workspace` reports no unused items)
- [ ] No unused imports
- [ ] No `unwrap()` in production code (only in tests and examples)

### Code organization

- [ ] No comments in code unless explicitly required (SAFETY, TODO with issue link)
- [ ] All public types have `#[doc]` attributes
- [ ] All public functions have `///` doc comments
- [ ] No circular dependencies between crates
- [ ] Dependency flow matches the architecture diagram (each layer depends only on layers below)

### Test coverage

- [ ] All `handle_*` functions have unit tests
- [ ] All rating systems have known-answer tests
- [ ] All outcome models have property tests
- [ ] All matchmakers have deterministic-formation tests
- [ ] Info-budget spies pass end-to-end (budget enforcement proven)
- [ ] Metamorphic tests pass (population scaling, noise invariance, window monotonicity)
- [ ] Same-seed determinism test passes (byte-identical metrics)
- [ ] Dynamic skill trajectories are exact (`S0 + k·t` for online players)

---

## 3. Reproducibility

### Determinism

- [ ] Same seed + same config → byte-identical `ExperimentResult` JSON
- [ ] Same seed + same config → byte-identical `StudyResult` JSON (net of timestamps)
- [ ] Same seed + same config → byte-identical `study_stats.json`
- [ ] Per-stream RNG isolation verified (varying one stream leaves others unchanged)

### Parallel execution

- [ ] CRN studies produce provably zero mean delta between identical arms
- [ ] Independent studies produce distinct results for different seeds
- [ ] Counterfactual replay reproduces the live arm's ratings bit-for-bit

### Checkpoint and resume

- [ ] `CheckpointManager` can serialize and restore `WorldSnapshot`
- [ ] Resumed simulation produces identical results from the same checkpoint
- [ ] Checkpoint serialization is deterministic for the same world state

---

## 4. Documentation

### Core documentation

- [ ] README.md explains what matchlab is, how to build it, and how to run the first experiment
- [ ] Quick start guide: build → run → read results in under 5 minutes
- [ ] Architecture overview explains the crate structure and dependency flow
- [ ] Truth separation principle documented with examples of correct/incorrect access
- [ ] Lua plugin system documented: contracts, RNG, context, information budget

### Configuration documentation

- [ ] Full YAML manifest schema documented (all fields, types, defaults)
- [ ] Config inheritance explained with examples
- [ ] Team composition (XvY) format documented
- [ ] All rating system parameters documented
- [ ] All outcome model parameters documented
- [ ] All matchmaker parameters documented
- [ ] Metric collector list with descriptions

### CLI documentation

- [ ] `matchlab run <manifest>` documented with all flags
- [ ] `matchlab study <study.yaml>` documented with `--replicates` and `--json`
- [ ] `matchlab compare <results...>` documented
- [ ] Exit codes documented (0 = success, 1 = failure, 2 = usage error)

### Methodology documentation

- [ ] Simulation methodology explained (population generation, event loop, metric collection)
- [ ] Rating system comparison methodology documented
- [ ] Statistical analysis methodology documented (CIs, effect sizes, multiple comparisons)
- [ ] Reproduction methodology documented (how to reproduce any published result)

### Plugin documentation

- [ ] How to write a rating system Lua script
- [ ] How to write an outcome model Lua script
- [ ] How to write a matchmaker Lua script
- [ ] How to write a metric collector Lua script
- [ ] How to write a detection system Lua script
- [ ] Plugin development cookbook with worked examples

---

## 5. Plugins

### Lua script suite

- [ ] `plugins/rating/elo.lua` — Elo rating system
- [ ] `plugins/rating/flat.lua` — Flat points baseline
- [ ] `plugins/rating/glicko2.lua` — Glicko-2 rating system
- [ ] `plugins/rating/trueskill.lua` — TrueSkill rating system
- [ ] `plugins/game/logistic.lua` — Logistic outcome model
- [ ] `plugins/game/variance.lua` — Variance outcome model
- [ ] `plugins/game/composition.lua` — Composition outcome model
- [ ] `plugins/game/performance.lua` — Performance outcome model
- [ ] `plugins/game/fatigue.lua` — Fatigue outcome model
- [ ] `plugins/game/momentum.lua` — Momentum outcome model
- [ ] `plugins/matchmaking/batch.lua` — Batch matchmaker
- [ ] `plugins/matchmaking/expanding_window.lua` — Expanding window matchmaker
- [ ] `plugins/matchmaking/strict.lua` — Strict matchmaker
- [ ] `plugins/matchmaking/hub_spoke.lua` — Hub-spoke matchmaker
- [ ] `plugins/matchmaking/random.lua` — Random matchmaker
- [ ] `plugins/metrics/match_quality.lua` — Match quality metric
- [ ] `plugins/metrics/queue_time.lua` — Queue time metric
- [ ] `plugins/metrics/rating_accuracy.lua` — Rating accuracy metric
- [ ] `plugins/metrics/convergence.lua` — Convergence metric
- [ ] `plugins/metrics/stability.lua` — Stability metric
- [ ] `plugins/metrics/streaks.lua` — Streak metric
- [ ] `plugins/metrics/population_health.lua` — Population health metric
- [ ] `plugins/metrics/smurf.lua` — Smurf metric
- [ ] `plugins/metrics/match_inequality.lua` — Match inequality metric
- [ ] `plugins/metrics/ndcg.lua` — NDCG metric
- [ ] `plugins/metrics/dimensionality_fidelity.lua` — Dimensionality fidelity metric
- [ ] `plugins/metrics/responsiveness.lua` — Responsiveness metric
- [ ] `plugins/detection/smurf.lua` — Smurf detection system
- [ ] `plugins/ranking/brackets.lua` — Rank bracket mapper
- [ ] `plugins/adversarial/afk.lua` — AFK adversarial agent
- [ ] `plugins/adversarial/deranker.lua` — Deranker adversarial agent
- [ ] `plugins/adversarial/win_trader.lua` — Win trader adversarial agent
- [ ] `plugins/adversarial/booster.lua` — Booster adversarial agent
- [ ] `plugins/adversarial/rating_farmer.lua` — Rating farmer adversarial agent
- [ ] `plugins/utility/satisfaction.lua` — Player satisfaction model

### Example experiments

- [ ] `experiments/v0_1_basic.yaml` — minimal baseline
- [ ] `experiments/base/standard.yaml` — inherited base config
- [ ] `experiments/full_featured.yaml` — all subsystems enabled
- [ ] `experiments/glicko_comparison.yaml` — Glicko-2 comparison
- [ ] `experiments/matchmaker_comparison.yaml` — matchmaker comparison
- [ ] `experiments/detection_test.yaml` — smurf detection
- [ ] `experiments/dbd_1v4.yaml` — asymmetric game mode
- [ ] `experiments/novel_rating.yaml` — custom Lua rating system
- [ ] `experiments/feedback_loop/*.yaml` — feedback loop factorial cells

### Cookbook

- [ ] How to add a new rating system (step-by-step)
- [ ] How to add a new outcome model (step-by-step)
- [ ] How to add a new matchmaker (step-by-step)
- [ ] How to add a new metric (step-by-step)
- [ ] How to run a controlled comparison (one variable differs)
- [ ] How to run a replicated study
- [ ] How to run a counterfactual replay study

---

## 6. Infrastructure

### CI/CD

- [ ] `.github/workflows/ci.yml` runs on every PR and push to main
- [ ] CI runs: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`
- [ ] CI runs on Linux, macOS, and Windows (or at least Linux + one other)
- [ ] CI build times are under 10 minutes for the full workspace
- [ ] CI caches dependencies (`cargo` cache action)

### Performance

- [ ] 10,000-player experiment completes in under 60 seconds (release build)
- [ ] 100,000-player experiment completes in under 10 minutes (release build)
- [ ] Memory usage is bounded (no unbounded allocations in the event loop)
- [ ] No performance regressions from the baseline (tracked via benchmarks if available)

### Cross-platform

- [ ] Builds on Linux (x86_64-unknown-linux-gnu)
- [ ] Builds on macOS (aarch64-apple-darwin)
- [ ] Builds on Windows (x86_64-pc-windows-msvc) or documents WSL2 as the supported path
- [ ] All tests pass on the primary platform
- [ ] Lua vendored build works on all target platforms

---

## 7. Release

### Changelog

- [ ] `CHANGELOG.md` covers all changes since the last release
- [ ] Breaking changes are clearly marked
- [ ] New features are listed with brief descriptions
- [ ] Bug fixes reference issue numbers
- [ ] Migration guide provided for breaking changes

### Version

- [ ] Version in `Cargo.toml` root is set to `1.0.0`
- [ ] Version in all crate `Cargo.toml` files is consistent
- [ ] Git tag created: `git tag v1.0.0`
- [ ] Release branch created (if using release branches)

### Artifacts

- [ ] Source tarball generated
- [ ] Binary releases built for all target platforms
- [ ] Binary releases tested on clean environments
- [ ] Checksums generated for all release artifacts
- [ ] Release artifacts uploaded to GitHub Releases

### License

- [ ] License file (`LICENSE` or `LICENSE-MIT` + `LICENSE-APACHE`) present
- [ ] All source files have license headers (if required by the license)
- [ ] All dependencies have compatible licenses
- [ ] `cargo deny` or `cargo audit` passes (no license violations, no security advisories)

---

## 8. Security Review

### Dependency audit

- [ ] `cargo audit` reports no known vulnerabilities
- [ ] `cargo deny` reports no advisories
- [ ] All dependencies are from crates.io (no git deps in release)
- [ ] Dependency versions are pinned (no `*` version specs)

### Input validation

- [ ] YAML manifests are validated at load time (unknown fields rejected or warned)
- [ ] Lua scripts are validated at load time (required functions checked, `math.random` banned)
- [ ] Numeric parameters are bounds-checked (no NaN, no infinity in configs)
- [ ] Population sizes are positive integers
- [ ] Team compositions have positive sizes

### Secret handling

- [ ] No secrets or API keys in the codebase
- [ ] No hardcoded paths or machine-specific values
- [ ] Seed values are not treated as secrets (they are reproducibility artifacts)

---

## 9. Observability

### Logging

- [ ] Structured logging available (via `tracing` or `log`)
- [ ] Log levels: `error` for failures, `warn` for degraded state, `info` for milestones, `debug` for diagnostics
- [ ] Match formation logged with match ID and team compositions
- [ ] Rating updates logged at debug level
- [ ] Experiment completion logged with summary metrics

### Metrics

- [ ] All 13 built-in metrics produce valid results
- [ ] Time-series metrics have the expected number of buckets
- [ ] Distribution metrics have non-empty samples (when data exists)
- [ ] Scalar metrics are finite (no NaN or infinity)

### Error handling

- [ ] All `Result` types are handled (no silent `unwrap()` in production)
- [ ] Error messages include context (which file, which player, which match)
- [ ] Panic messages are descriptive (not just "index out of bounds")
- [ ] Lua script errors propagate with file path and error message

### Progress reporting

- [ ] Long-running experiments print progress (matches completed / total)
- [ ] Study replication progress reported (replicate N / total)
- [ ] ETA estimation based on completed work

---

## Sign-off

| Area | Owner | Date | Status |
|------|-------|------|--------|
| Correctness | | | |
| Code Quality | | | |
| Reproducibility | | | |
| Documentation | | | |
| Plugins | | | |
| Infrastructure | | | |
| Release | | | |
| Security | | | |
| Observability | | | |

Release manager: ______________________ Date: _______________
