# 001 — Create `matchlab-lua` crate

## Summary

Create a new workspace crate `crates/matchlab-lua` that is the single shared
foundation for all Lua-native systems. It replaces the four near-duplicate
per-crate `hooks.rs` modules (rating/game/matchmaking/metrics) with one VM
wrapper, and adds the context plumbing, deterministic RNG routing, script
validation, core-type conversions, and path resolution that every layer will
reuse.

## Context

Currently each algorithm crate carries its own `LuaHooks { lua: Mutex<Lua> }`
copy with hand-written per-hook call methods, and detection has no Lua support
at all. The refactor inverts the model: Lua implements the whole algorithm, so
every layer needs the same machinery (load script, inject `config`, call named
functions, thread a context blob, draw deterministic randomness). That machinery
belongs in one place.

`matchlab-lua` depends on `matchlab-core` only, so it sits directly above core
in the dependency graph. All algorithm crates (rating, game, matchmaking,
detection, metrics, adversarial, utility, ranking) will depend on it.

## Scope

**In:**
- `LuaVm` — load a script, inject a `config` table, call functions, read globals.
- `Context` — arbitrary script-defined data stored on the Rust model and
  threaded through every call.
- Deterministic RNG routing — `matchlab.rng_*` helpers drawing from the
  in-flight `&mut SimRng`.
- Script validation — required-function presence, `math.random` ban.
- Core-type conversions — `PlayerObservation`, `MatchResult`, `Team`,
  `SimTime`, ids, and the metric-only snapshot (which may carry `true_skill`).
- Path resolution — resolve `plugins/...` paths from crate test dirs and the
  workspace root.

**Out:**
- No per-layer contracts (those live in tickets 002–008).
- No conversion of crate-specific types (`RatingState`, `QueueEntry`,
  `ProposedMatch`, `DetectionResult`, `MetricResult`) — those belong to the
  adapters in each layer ticket.

## Design

### Crate wiring

- Add `matchlab-lua` to the workspace `[workspace] members` in the root
  `Cargo.toml`.
- `Cargo.toml`: depend on `matchlab-core` (path), `mlua` (workspace), `serde`
  (workspace, derive), `serde_yaml` (workspace).
- Module layout:

```
crates/matchlab-lua/
└── src/
    ├── lib.rs        # re-exports
    ├── vm.rs         # LuaVm
    ├── context.rs    # Context type + helpers
    ├── rng.rs        # thread-local SimRng routing + matchlab.rng_* providers
    ├── convert.rs    # core <-> Lua conversions
    ├── validate.rs   # script validation
    └── resolve.rs    # workspace-root path resolution
```

### `context.rs`

```rust
/// Arbitrary, script-defined data persisted by a Rust model across calls.
/// Defaults to an ordered empty mapping.
pub type Context = serde_yaml::Value;

pub fn empty() -> Context;                       // Mapping(Map::new())
pub fn to_lua(&self, lua: &Lua) -> Result<Value, String>;
pub fn from_lua(value: &Value) -> Result<Context, String>;
```

`serde_yaml::Value` is chosen because (a) it round-trips exactly with the YAML
params the script already receives and (b) it is fully serializable, so a
system's context could be inspected or persisted for debugging. It is an
ordered mapping, so key insertion order is stable across runs — determinism-safe.

### `vm.rs`

```rust
pub struct LuaVm { lua: Mutex<Lua>, script_path: String }

impl LuaVm {
    /// Read script, exec it, inject `config` (from YAML params) as a global
    /// table, and register the `matchlab.rng_*` providers.
    pub fn load(path: &str, params: &Context) -> Result<Self, String>;

    pub fn script_path(&self) -> &str;

    /// Call a function; `Ok(None)` when the function is not defined.
    pub fn call<T: FromLuaMulti>(&self, name: &str, args: impl IntoLuaMulti)
        -> Result<Option<T>, String>;

    /// Call a function and require it to exist.
    pub fn call_required<T: FromLuaMulti>(&self, name: &str, args: impl IntoLuaMulti)
        -> Result<T, String>;

    /// Read a global (e.g. `information_budget`, `name`, `time_buckets`).
    pub fn get_global<T: FromLua>(&self, name: &str) -> Result<Option<T>, String>;

    /// Run `f` with the given `&mut SimRng` made available to `matchlab.rng_*`.
    /// Sets the thread-local slot, calls `f`, clears the slot.
    pub fn with_rng<T>(&self, rng: &mut SimRng, f: impl FnOnce(&Self) -> T) -> T;
}
```

`LuaVm` keeps the existing pattern (`Mutex<Lua>`) so it is `Send + Sync` and can
live inside trait adapters.

### `rng.rs`

Lua scripts never call `math.random` (banned, see `validate.rs`). Instead they
call deterministic helpers registered as globals:

- `matchlab.rng_range(low, high) -> number`
- `matchlab.rng_bool(p) -> boolean`
- `matchlab.rng_normal(mean, stddev) -> number`
- `matchlab.rng_u64() -> integer`

Implementation: a `thread_local! RefCell<Option<*mut SimRng>>` slot. The
providers deref the pointer and draw from `SimRng`. `LuaVm::with_rng` sets the
slot around a Lua call. Invariants: single-threaded simulation; the slot is
always set/cleared in a pair; drawing happens only inside the guarded region.
This preserves the `SimRng` sequence exactly, so determinism is unchanged.

### `convert.rs`

Core `matchlab-core` types → Lua, and back. Observation tables always include
only observable fields plus the ground-truth-derived skill binding the game
model needs:

```
observation -> {
  player_id, rating, hidden_mmr, rating_deviation, volatility, games_played,
  win_rate, tilt_level, queue_joined_at_secs, party_id, is_online,
  skill_overall,        -- skill_vector.overall()   (outcome-model binding)
  skill_vector = { dim -> value },                  -- for composition models
}
```

- `observation_to_table(&Lua, &PlayerObservation) -> Result<Table, String>`
- `observations_to_table(&Lua, &[PlayerObservation]) -> Result<Value, String>`
- `match_result_to_table(&Lua, &MatchResult) -> Result<Table, String>` (winner,
  team_a, team_b, scores, duration_secs, performances list, variance,
  disconnected, forfeited)
- `performance_to_table` (kills, deaths, assists, objective_score, impact, variance)
- `metric_snapshot(&Lua, &MatchResult, &World) -> Result<Value, String>`
  — match result + participant observations + `true_skill` per participant.
  **Metrics only.** Used by ticket 006.

### `validate.rs`

```rust
pub struct ValidationReport { pub defined_functions: Vec<String> }

/// Parse + exec the script, then check that every name in `required` is a
/// global function. Reject the script when `math.random` appears in its source.
pub fn validate_script(path: &str, required: &[&str]) -> Result<ValidationReport, String>;
```

The `math.random` ban is a source-text scan for `math.random` — strict by
design; a script that merely mentions it in a comment is rejected. Document this
in the error message. Arity checking is left to each adapter's `call_required`
error path (Lua reports argument mismatches with the function name).

### `resolve.rs`

```rust
/// Resolve a `plugins/...` path to an absolute path.
/// 1. If `path` exists as given (relative to cwd), use it.
/// 2. Else walk up from CARGO_MANIFEST_DIR to the workspace root (the first
///    ancestor with a Cargo.toml declaring `[workspace]`) and join `path`.
pub fn resolve_script_path(path: &str) -> PathBuf;
```

The CLI runs from the workspace root (step 1 works); crate unit tests run from
crate directories (step 2 finds the root). Cache the workspace root in a
`OnceLock`.

## Steps

1. Create `crates/matchlab-lua/` with `Cargo.toml`; add to workspace members.
2. Implement `context.rs` and `resolve.rs`.
3. Implement `rng.rs` (thread-local slot + providers) and `vm.rs` (`LuaVm`).
4. Implement `convert.rs` for core types.
5. Implement `validate.rs`.
6. Wire `lib.rs` re-exports.
7. Write unit tests (below). Do **not** touch any algorithm crate yet.
8. Update `AGENTS.md` (workspace layout, new crate entry).

## Acceptance Criteria

- [ ] `cargo build --workspace` and `cargo check --workspace` pass with the new
      crate present and no other crate changed.
- [ ] `LuaVm::load` with a valid script + params injects a readable `config`
      table; missing file / syntax error / exec error produce `Err` with the
      script path in the message.
- [ ] Context round-trips: script receives a context table, mutates it, returns
      it; Rust stores the new value; the next call sees the mutation.
- [ ] `matchlab.rng_range` / `rng_bool` / `rng_normal` draw deterministically:
      two `with_rng` sessions over the same seeded `SimRng` produce identical
      sequences.
- [ ] `validate_script` rejects a script missing a required function, and
      rejects any script whose source contains `math.random`.
- [ ] `resolve_script_path("plugins/rating/elo.lua")` resolves from both the
      workspace root and a crate test dir (once a script exists there).
- [ ] `Observation`/`MatchResult` conversions round-trip all fields that the
      layer contracts in tickets 002–008 need.
- [ ] `cargo clippy --workspace` and `cargo fmt --check` clean.

## Testing

- `context.rs`: empty default; to_lua/from_lua round-trip (nested maps, numbers,
  strings, arrays, bools).
- `vm.rs`: load valid/invalid/missing scripts; `call` vs `call_required`
  semantics; `get_global`; `with_rng` guards the slot (helper provably absent
  outside the guard).
- `rng.rs`: determinism test (two identical runs); interleaved draws consume the
  same `SimRng` sequence as a pure-Rust reference.
- `validate.rs`: missing function → Err; `math.random` present → Err; clean
  script → Ok with `defined_functions`.
- `resolve.rs`: path resolution from a nested temp dir anchored to the
  workspace.
- `convert.rs`: field-level assertions on observation and match-result tables;
  `metric_snapshot` exposes `true_skill`.

## Risks / Notes

- The thread-local raw-pointer RNG routing is `unsafe`; keep it contained in
  `rng.rs` with a clear invariant comment and prefer the safe `with_rng` guard
  everywhere.
- `serde_yaml::Value` for `Context` means per-call conversion cost; keep context
  payloads small (a few floats per player) and avoid copying large histories in
  hot paths.
- Subsequent tickets may surface missing conversions; extend `convert.rs`
  rather than duplicating per-crate table building.