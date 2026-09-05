# 002 — Port rating systems to Lua

## Summary

Make `RatingSystem` fully Lua-implementable and **delete the Rust rating
implementations**. `LuaRatingSystem` (a thin trait adapter over `matchlab-lua`)
becomes the only way to build a rating system; the four classic systems
(Elo, FlatPoints, Glicko-2, TrueSkill) are ported to Lua scripts under
`plugins/rating/`. This is the canonical layer ticket — it establishes the
contract pattern that tickets 003–008 follow.

## Context

Currently `matchlab-rating` has Rust `EloRatingSystem`, `FlatPointsRatingSystem`,
`Glicko2RatingSystem`, `TrueSkillRatingSystem`, each optionally wrapped with
hook-style `LuaHooks`, and a registry with `lua:`-prefixed variants. Per the
refactor decision, these Rust algorithms are deleted and replaced by scripts.

The `RatingSystem` trait signature is **unchanged**; the loop and
`counterfactual_eval` keep working on `Box<dyn RatingSystem>`.

## Scope

**In:**
- `LuaRatingSystem` adapter in `matchlab-rating::lua`.
- New script registry: `from_script`, plus `name -> script` lookup so manifests
  stay concise.
- Lua ports of Elo, FlatPoints, Glicko-2, TrueSkill.
- `information_budget` declared by the script.
- Config + runner wiring; loop/experiments test updates; manifest rating
  sections; ported tests (incl. the Glicko-2 golden example).

**Out:**
- No changes to `RatingState`, `ObservationType`, `filter.rs` (budget
  sanitization), or the `RatingSystem` trait itself.

## Design

### Lua contract (rating system script)

Every rating script defines:

```lua
-- global read by the adapter at load time
information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    -- config: YAML params (k_factor, initial_rating, beta, ...)
    -- context: opaque script-owned data (may be any Lua value; default {})
    -- returns (state, context)
    -- state = { rating, rating_deviation, volatility, games_played }
end

function predict(team_a, team_b, config, context)
    -- team_a/team_b: arrays of observation tables (see matchlab-lua convert)
    -- returns expected score of team_a (number)
end

function update(match_result, observations, config, context)
    -- match_result: table from matchlab-lua convert (winner, team_a, team_b, ...)
    -- observations: array of observation tables for participants
    -- returns (updates, context)
    -- updates = { { player_id, rating, rating_deviation, volatility, games_played }, ... }
end
```

`context` is threaded: the adapter passes the stored `Context` in and stores the
returned value. Systems that need per-player state (e.g. TrueSkill variances,
decay timers) keep it in `context` keyed by player id.

### `matchlab-rating::lua` — `LuaRatingSystem`

```rust
pub struct LuaRatingSystem {
    vm: LuaVm,
    context: Mutex<Context>,   // serde_yaml::Value
    budget: Vec<ObservationType>,
    name: String,
}

impl LuaRatingSystem {
    pub fn load(script: &str, params: &serde_yaml::Value) -> Result<Self, String>;
    // - resolve_script_path
    // - validate_script(path, &["initialize", "predict", "update"])
    // - read `information_budget` global -> Vec<ObservationType>
    //   (string -> enum map; unknown string -> load error)
}
```

`RatingSystem` impl:
- `information_budget()` → stored `budget`.
- `initialize(id)` → `vm.call_required("initialize", (id.0, config, ctx))`
  → `(state_table, ctx)`; read `{rating, rating_deviation, volatility,
  games_played}` (defaults for missing: 350.0 / 0.06 / 0); store new `ctx`.
- `predict(a, b)` → `vm.call_required("predict", (a_tbl, b_tbl, config, ctx))`
  → f64; store returned `ctx` (predict may also update context).
- `update(mr, obs)` → build `mr_tbl` + `obs_tbl` via `matchlab_lua::convert`;
  `call_required("update", ...)` → `(updates_tbl, ctx)`; map each row to
  `HashMap<PlayerId, RatingState>`; store `ctx`.

### Registry (`plugins.rs`, rewritten)

```rust
pub mod registry {
    /// Resolve a system by script path.
    pub fn from_script(path: &str, params: &serde_yaml::Value)
        -> Result<Box<dyn RatingSystem>, String>;

    /// Built-in name -> script map, for concise manifests and docs.
    pub fn known_systems() -> Vec<(&'static str, &'static str)>; // e.g. ("elo", "plugins/rating/elo.lua")

    /// Resolve `name` via `known_systems`, else error.
    pub fn from_name(name: &str, params: &serde_yaml::Value)
        -> Result<Box<dyn RatingSystem>, String>;
}
```

The `lua:` variants and the old `all_systems()` (list of Rust names) are gone.
`from_name` exists only for manifest brevity and maps to a script path.

### Scripts (`plugins/rating/`)

Port the math from the current Rust implementations (delete them in the same
ticket), preserving semantics exactly:

- `elo.lua` — `divisor = beta * ln(10)` (log10 scale consistent with the
  logistic game model); `expected_score(a,b) = 1/(1+10^((b-a)/divisor))`; team
  averages from `team_a`/`team_b` ratings; `update` applies
  `k_factor * (actual - expected)` per participant, incrementing `games_played`.
- `flat.lua` — fixed `win_points` / `loss_points` around `initial_rating`.
- `glicko2.lua` — full 6-step Glicko-2 (scale to μ/φ/σ → g/E per opponent →
  v, Δ → Newton-Raphson volatility iteration → φ*, φ', μ' → scale back), one
  opponent tuple per opposing-team player. Verified by the golden test below.
- `trueskill.lua` — N(μ, σ²) per player; team performance = sum of members;
  truncated-Gaussian conditioning with `v`/`w` factors and draw margin from
  `draw_probability`; `initial_variance` stored as `rating_deviation` = σ.

Each script declares `information_budget = { "WinLoss" }` and reads its params
from `config` (e.g. `config.k_factor`, `config.initial_rating`, `config.beta`).

### Deletions

- `crates/matchlab-rating/src/elo.rs`, `flat.rs`, `glicko.rs`, `trueskill.rs`,
  `hooks.rs`, `loader.rs`.
- `plugins.rs` old registry body (rewritten, not deleted).
- Keep: `system.rs`, `filter.rs`.

### Config + runner

- `crates/matchlab-experiments/src/config.rs` — `RatingSystemSpec`:
  ```rust
  pub struct RatingSystemSpec {
      #[serde(default)] pub name: Option<String>,  // optional label
      pub script: String,
      #[serde(flatten)] pub params: HashMap<String, serde_yaml::Value>,
  }
  ```
- `runner.rs::build_rating_system` → `registry::from_name(&spec.name, &params)`
  if `name` is set, else `registry::from_script(&spec.script, &params)`. Keep the
  "at least one system" check.
- Runner unit tests: `MINI` manifest rating section becomes
  `- script: plugins/rating/elo.lua` + params; the `unknown_rating_system_is_rejected`
  test now mutates the script path to a nonexistent file.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` tests: `default_state` and the
  pipeline/determinism tests construct `EloRatingSystem::new(...)` → build via a
  test helper that loads `plugins/rating/elo.lua` (using `matchlab-lua` path
  resolution). Add `matchlab-lua` as a dev-dependency of `matchlab-loop`.
- `crates/matchlab-experiments/src/counterfactual.rs` tests: construct Elo/Flat
  via `registry::from_script` (function itself is unchanged).
- `crates/matchlab-rating` lib.rs exports.
- Manifests: rating sections in `v0_1_basic.yaml`, `base/standard.yaml`,
  `full_featured.yaml`, `glicko_comparison.yaml`, `detection_test.yaml`,
  `matchmaker_comparison.yaml`, `lua_hooks_test.yaml` → `script:` form.
- Delete `plugins/rating/dynamic_elo.lua` and `plugins/rating/adaptive_glicko.lua`
  (hook-style; superseded).

## Steps

1. Implement `lua.rs` (`LuaRatingSystem`) and the rewritten registry.
2. Write the four scripts under `plugins/rating/`, porting the Rust math.
3. Delete the Rust implementation files and old hooks/loader modules; update
   `lib.rs`.
4. Update `config.rs` + `runner.rs`; update runner and counterfactual tests.
5. Update `machine.rs` tests (Lua elo helper).
6. Update manifest rating sections; remove hook-style rating scripts.
7. Port tests (below) and write new registry/validation tests.
8. Update `AGENTS.md` (rating crate section).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` all pass.
- [ ] No reference to `EloRatingSystem`, `Glicko2RatingSystem`,
      `TrueSkillRatingSystem`, `FlatPointsRatingSystem`, or `LuaHooks` remains
      anywhere in the workspace (grep-clean).
- [ ] **Glicko-2 golden example** passes against `glicko2.lua`: the worked
      example from Glickman's paper reproduces r′=1464.06, RD′=151.52,
      σ′=0.05999 within tolerance.
- [ ] `elo.lua`: equal ratings → `predict` ≈ 0.5; a win moves the winner's
      rating up by `k_factor * (1 - expected)` and the loser's down symmetrically.
- [ ] `flat.lua`: winner +`win_points`, loser −`loss_points` (or vice versa per
      current semantics).
- [ ] `trueskill.lua`: winner's mean rises, loser's falls; `rating_deviation`
      decreases with games played.
- [ ] `information_budget` read from the script drives `filter_match_result`
      sanitization in the loop (WinLoss-only results stripped as today).
- [ ] `registry::from_script` with a script missing `update` returns `Err`;
      a script containing `math.random` is rejected by validation.
- [ ] A mini experiment run through the Lua elo script completes with sensible
      metrics (rating_accuracy mean well below the cold-start 250+ and trending
      down in `rating_accuracy_by_time`).

## Testing

- Golden test: Glicko-2 worked example against `glicko2.lua`.
- Elo: predict symmetry, update direction + magnitude, convergence over a
  scripted series.
- TrueSkill: single-match update sanity (winner up, loser down, RD shrinks).
- Registry: `from_name("elo", params)` resolves and behaves; unknown name →
  Err; missing script file → Err; invalid script → Err; `math.random` script →
  Err.
- Adapter: context threading (script stores a counter in context; two calls see
  the increment); budget mapping round-trip.
- Counterfactual tests (replaced constructions) still pass with Lua systems.
- Runner determinism test now exercises the Lua path end-to-end (same seed →
  identical metrics).

## Risks / Notes

- Glicko-2/TrueSkill ports are the correctness-critical pieces. Port field by
  field from the current Rust code and keep the golden test as the gate.
- Floating-point: Lua numbers are f64; expect the golden test to need a small
  tolerance (1e-2 on rating, 1e-2 on RD, 1e-4 on volatility is safe).
- `matchlab-loop` needs `matchlab-lua` as a dev-dependency for its tests.