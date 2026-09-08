# Dependency Audit

Date: 2026-09-08

## Workspace Dependencies

| Dependency | Version | Purpose | Public API | Maintenance Status |
|---|---|---|---|---|
| `serde` | 1 | Serialization/deserialization framework for configs, metrics, and results. `derive` feature for procedural macros. | Yes — all config types, `MetricResult`, `ExperimentResult` derive `Serialize`/`Deserialize`. | Actively maintained. De facto standard. |
| `serde_json` | 1 | JSON encoding for experiment result export (`write_result_json`) and CLI output. | Yes — `ExperimentResult` is serialized to JSON for file export. | Actively maintained. Part of the serde ecosystem. |
| `serde_yaml` | 0.9 | YAML parsing for experiment manifests, config inheritance, and Lua script parameter passing. | Yes — `ExperimentConfig` and nested types are `Deserialize` from YAML. | Maintenance-only (0.9.x). Sufficient for read-heavy use; no known vulnerabilities. |
| `rand` | 0.8 | Deterministic RNG (`SmallRng`) for population generation, match simulation, and all stochastic subsystems. | Yes — `SimRng` wraps `rand::rngs::SmallRng`. | Actively maintained. 0.8 is the current stable line. |
| `rand_chacha` | 0.3 | ChaCha algorithm backing `SmallRng` for reproducible seeded randomness. | No — internal implementation detail. | Actively maintained. Part of the rand ecosystem. |
| `mlua` | 0.10 | Lua 5.4 VM embedding for the plugin system. All rating, outcome, matchmaking, detection, and metric algorithms are Lua scripts. | Yes — `LuaOutcomeModel`, `LuaRatingSystem`, `LuaMatchmaker`, etc. use `mlua::Lua` internally. Features: `lua54`, `vendored`. | Actively maintained. Vendored Lua avoids system dependency. |
| `rusqlite` | 0.31 | SQLite-backed `ExperimentStore` for persisting experiment results. Feature: `bundled` (compiles SQLite from source). | No — used only in `matchlab-experiments` store module. | Actively maintained. `bundled` feature ensures portability. |
| `tracing` | 0.1 | Structured logging facade. Used for progress reporting, experiment lifecycle events, and diagnostics. | No — instrumentation only. | Actively maintained. Part of the tokio ecosystem. |
| `tracing-subscriber` | 0.3 | Log formatting and filtering. Provides `EnvFilter` for `RUST_LOG` support and JSON output. Feature: `env-filter`. | No — wiring only in `logging.rs`. | Actively maintained. |
| `tracing-appender` | 0.2 | Non-blocking file appender for `--log-file` support (T-158). | No — wiring only in `logging.rs`. | Actively maintained. Part of the tokio ecosystem. |

## Summary

- **Total external dependencies:** 10
- **Workspace-managed:** All 10 (declared in `[workspace.dependencies]`)
- **Used in public API:** 5 (`serde`, `serde_json`, `serde_yaml`, `rand`, `mlua`)
- **Instrumentation only:** 3 (`tracing`, `tracing-subscriber`, `tracing-appender`)
- **Internal implementation detail:** 2 (`rand_chacha`, `rusqlite`)

## Assessment

The dependency tree is minimal and well-curated. Every dependency earns its place:

- **No transitive dependency bloat.** The workspace has exactly 10 direct external crates. None pulls in a large dependency tree.
- **No unmaintained crates.** All dependencies are actively maintained or in stable maintenance mode with no known issues.
- **Version pinning is conservative.** Semver-compatible ranges (`serde = "1"`, `rand = "0.8"`) allow patch updates without breaking changes.
- **The vendored Lua (`mlua` with `vendored` feature)** eliminates the system Lua dependency, improving portability across platforms.
- **The bundled SQLite (`rusqlite` with `bundled` feature)** similarly eliminates a system dependency.
- **No cryptographic or network dependencies.** This is a pure simulation framework — no `reqwest`, `hyper`, `tokio`, etc.

## Recommendations

1. **Monitor `serde_yaml` 0.9.** The crate is in maintenance mode. If a 0.10 or 1.0 release appears, evaluate migration. For now, 0.9 is stable and sufficient.
2. **No action needed on other dependencies.** All are current and well-maintained.
3. **Consider adding `tracing-appender`** to the workspace dependencies if file logging (T-158) is accepted.
