# Security & Robustness Review

This document describes the trust model, attack surface, and deployment recommendations for matchlab.

---

## Trust Model

matchlab is a **single-user research tool**, not a multi-tenant service. The user who runs the simulation is fully trusted. The security model reflects this: correctness and robustness matter more than isolation.

### What Is Trusted

- **The user.** All configuration, scripts, and data are supplied by the operator.
- **The Rust codebase.** The core framework, event loop, and all algorithm crates.
- **The Lua scripts under `plugins/`.** These are local files, not fetched from remote sources.

### What Is Not Trusted (by design)

- **Output paths.** The framework writes to user-specified directories. A misconfigured path could overwrite files. This is the user's responsibility.
- **YAML manifests.** Malformed or contradictory configs can produce unexpected behavior. Validation catches structural errors but not semantic nonsense.

---

## Attack Surface

### 1. Lua Execution Model

**Risk level: HIGH (by design)**

Lua scripts under `plugins/` are executed by the `mlua` crate (Lua 5.4, vendored). Scripts have **full Lua language access** — including file I/O, string manipulation, table operations, and math. There is **no sandbox**.

```
plugins/
├── rating/       # elo.lua, glicko2.lua, trueskill.lua, flat.lua, decay_elo.lua
├── game/         # logistic.lua, variance.lua, composition.lua, etc.
├── matchmaking/  # batch.lua, expanding_window.lua, strict.lua, etc.
├── metrics/      # rating_accuracy.lua, queue_time.lua, etc.
├── detection/    # smurf.lua
├── adversarial/  # afk.lua, deranker.lua, win_trader.lua, etc.
├── utility/      # satisfaction.lua
└── ranking/      # brackets.lua
```

**Capabilities of a Lua script:**
- Read and write files on the local filesystem.
- Execute OS commands via `os.execute` and `io.popen`.
- Access network sockets (if the Lua build includes them; mlua's vendored Lua 5.4 does not include `socket` by default, but `os.execute` can shell out).
- Read all data passed to it: player observations, match results, population snapshots.

**Mitigations:**
- Scripts are local files, not fetched from untrusted sources.
- The `validate_script` function checks for `math.random` usage (which would break determinism) but does not restrict other capabilities.
- In a deployment context, the plugins directory should be read-only and audited.

**Recommendation:** If deploying in a shared environment, run the process in a container with no network access, a read-only filesystem for `plugins/`, and a restricted user account.

### 2. Configuration Trust Model

**Risk level: LOW (user-supplied)**

Experiment manifests are YAML files parsed by `serde_yaml`. The config is validated structurally (unknown fields rejected, required fields enforced) but not semantically.

**Risks:**
- A malformed YAML file can cause a panic or unexpected behavior during parsing. This is mitigated by serde's deserialize-time validation and explicit error handling in `ExperimentRunner`.
- A config referencing a non-existent Lua script will fail at load time with a clear error.
- A config with contradictory parameters (e.g., `skill_update_interval_secs` with `improvement_rate = 0`) produces a valid but uninteresting simulation — not a crash.

**Mitigations:**
- `validate_script` is called at load time for all referenced scripts.
- Config inheritance (`base:`) is resolved before validation, preventing shadowed overrides.
- The `ExperimentConfig` type uses serde's strict deserialization (no `#[serde(default)]` on required fields).

**Recommendation:** Treat experiment manifests as code. Version them alongside Lua scripts. Review changes before running.

### 3. Filesystem Paths

**Risk level: LOW-MEDIUM**

Output directories, script paths, and base config references are all user-controlled strings. The framework resolves paths relative to the workspace root.

**Risks:**
- An output directory outside the workspace could overwrite unrelated files.
- A script path with `../` traversal could load a Lua file outside `plugins/`.
- Multiple concurrent runs writing to the same output directory could produce interleaved results.

**Mitigations:**
- `resolve_script_path` walks up to the workspace root and validates the resolved path. Scripts outside the workspace are not loaded by default.
- Output is written atomically per-file (JSON, Markdown).
- No symlink following or privilege escalation is possible — the process runs as the invoking user.

**Recommendation:** Use absolute paths for output directories. Ensure the output directory is empty or contains only results from prior runs. Do not run multiple concurrent experiments targeting the same output path.

### 4. Serialized Artifacts

**Risk level: LOW**

Experiment results, study results, and raw data exports are JSON files. They are produced by the framework and consumed by `matchlab compare`, `matchlab study`, and analysis tooling.

**Risks:**
- A crafted JSON file fed to `matchlab compare` could contain unexpected metric types, causing analysis panics. This is mitigated by `MetricResult`'s serde handling and `extract_scalar` returning `None` for non-scalar metrics.
- A result file with a manipulated `config_hash` or `git_commit` would produce misleading provenance. This is a metadata integrity concern, not a runtime risk.

**Mitigations:**
- `ExperimentResult` uses typed deserialization; unknown fields are ignored.
- The config hash is recomputed from the actual config + script contents, not trusted from the file.
- Git commit hash is captured at run time, not read from artifacts.

**Recommendation:** Treat exported results as reproducibility artifacts, not authoritative truth. Re-run experiments from manifests to verify.

### 5. Plugin Loading

**Risk level: LOW (local files)**

Lua scripts are loaded from the `plugins/` directory via `resolve_script_path`, which walks up to the workspace root and resolves relative paths. Scripts are validated for required functions (`math.random` check, required entry points) at load time.

**Risks:**
- A malicious or buggy script could hang (infinite loop), crash (memory exhaustion), or corrupt shared state.
- A script loaded from a path outside `plugins/` (via `script: /etc/passwd`) would still be executed if the path resolves — the framework does not restrict script locations.

**Mitigations:**
- The `validate_script` function checks for required functions and rejects scripts that use `math.random`.
- Lua scripts run inside `mlua`'s managed VM. A Lua panic or memory limit hit will abort the script and propagate an error to Rust.
- In practice, scripts are authored by the user or trusted collaborators.

**Recommendation:** Restrict the `plugins/` directory to trusted authors. Use `validate_script` in CI to catch regressions. Consider adding a path allowlist for production deployments.

### 6. No Network Access from Lua

**Risk level: N/A (not a concern)**

The vendored Lua 5.4 build does not include networking libraries. Lua scripts cannot make HTTP requests, open sockets, or communicate with external services. The only way to exfiltrate data would be via `os.execute` (shelling out), which requires the Lua build to support it.

**Recommendation:** If deploying in a sensitive environment, verify that `os.execute` is unavailable in the Lua build. The vendored mlua build does not expose it by default.

### 7. No Subprocess Execution from Lua

**Risk level: N/A (not a concern by default)**

The vendored Lua 5.4 does not include `os.execute` or `io.popen` in the standard library exposed by mlua. Scripts cannot spawn child processes.

**Recommendation:** Verify this assumption by checking the mlua feature flags. If `os.execute` is exposed, consider sandboxing or disabling it.

---

## Robustness Guarantees

### Determinism

- Every experiment is fully deterministic given its config + seed. The `SeedManager` derives separate RNG streams for population, games, arrivals, behavior, matchmaking, and master from a single seed.
- Lua scripts draw randomness exclusively via `matchlab.rng_*` helpers, which route through the seeded `SimRng`. Scripts that call `math.random` are rejected at load time.
- `HashMap` iteration order is randomized by Rust's `RandomState`. The loop seeds initial `PlayerJoin` events in ascending `PlayerId` order to eliminate this source of non-determinism.

### Invariant Enforcement

The `matchlab-validation` crate contains property tests that verify:
- No NaN or ±Inf in any finalized metric or rating state (via `SanityRatingSystem`).
- No player formed before joining the queue.
- Single-match membership (no player in two simultaneous matches).
- Population counts are conserved.
- Team sizes and roles match composition specifications.
- Same-seed runs produce byte-identical metrics and ratings.

### Information Budget Enforcement

The `plugins/_test/` directory contains spy scripts that verify the truth-separation principle end-to-end:
- `spy_rating.lua` refuses any `MatchResult` that still carries scores, duration, or ground-truth skill keys.
- `spy_matchmaker.lua` refuses any queue entry that carries `skill_vector`, `skill_overall`, or `hidden_mmr`.
- `spy_collector.lua` (positive control) asserts that reality fields ARE present for metrics.

A `#[should_panic]` negative control proves the spies are live.

---

## Deployment Recommendations

### Development (single user)

- Run directly: `cargo run -- run experiments/v0_1_basic.yaml`
- No special security measures needed.
- Scripts are authored and versioned locally.

### Shared research environment

1. **Read-only plugins directory.** Mount `plugins/` as read-only to prevent accidental modification.
2. **Isolated output directory.** Each run writes to a unique subdirectory under `/results`.
3. **No network access.** Run in a network-isolated environment (Docker `--network none`, air-gapped machine).
4. **Resource limits.** Set Lua memory limits via mlua's allocator configuration. Set process CPU/memory limits via cgroups or `ulimit`.
5. **Input validation.** Run `cargo test` and `validate_script` on all scripts before deploying new experiments.

### CI/CD pipeline

1. `cargo build --workspace` — catches compilation errors.
2. `cargo test --workspace` — runs all unit tests, invariant checks, metamorphic tests, and info-budget spies.
3. `cargo clippy --workspace` — catches common bugs and style issues.
4. `cargo fmt --check` — enforces formatting.
5. Run a smoke experiment: `cargo run -- run experiments/lua_systems_test.yaml` — verifies Lua integration end-to-end.

### Production (automated experiment campaigns)

1. **Container isolation.** Run each experiment in a fresh container with:
   - No network access.
   - Read-only root filesystem (except output directory).
   - Non-root user.
   - CPU/memory limits.
2. **Manifest review.** Treat experiment manifests as code — require review before merging.
3. **Script auditing.** All Lua scripts under `plugins/` should be reviewed and version-controlled.
4. **Output integrity.** Hash output artifacts and verify against expected config hashes.
5. **Provenance tracking.** `ExperimentResult` records `config_hash` and `git_commit` for exact reproduction.

---

## Known Limitations

1. **No sandbox for Lua.** Scripts have full Lua language access. This is intentional for flexibility but means scripts must be trusted.
2. **No input size limits.** A population of 10 billion players would exhaust memory. The framework does not enforce population size caps.
3. **No timeout for Lua scripts.** An infinite loop in a Lua script will hang the process. Consider process-level timeouts in automated deployments.
4. **No encryption of artifacts.** Exported JSON files contain raw simulation data. Do not store sensitive information in experiment configs or player names.
5. **Single-threaded simulation.** The event loop is sequential. Parallelism is only at the replication level (independent experiments can run in separate processes).
