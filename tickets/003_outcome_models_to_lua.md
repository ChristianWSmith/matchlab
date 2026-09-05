# 003 — Port outcome models to Lua

## Summary

Make `OutcomeModel` fully Lua-implementable and **delete the Rust outcome
models**. `LuaOutcomeModel` becomes the only way to build an outcome model; the
six variants (logistic, variance, composition, performance, fatigue, momentum)
are ported to Lua scripts under `plugins/game/`.

## Context

`matchlab-game` currently has `LogisticOutcomeModel` (with `effective_skill`
reading the true-skill binding in the observation), plus variance/composition/
performance/fatigue/momentum variants, several with hook-style `LuaHooks`.
These are deleted; the loop keeps calling `OutcomeModel::simulate` on a
`Box<dyn OutcomeModel>` (trait unchanged).

Outcome models are the one algorithm tier that may read ground-truth-derived
skill: the observation table carries `skill_overall` / `skill_vector` (set from
`PlayerReality.skill` at population generation) so match winners are decided by
true skill — that is what makes rating convergence a real property. This must
survive the port.

## Scope

**In:**
- `LuaOutcomeModel` adapter in `matchlab-game::lua`.
- Lua ports of logistic, variance, composition, performance, fatigue, momentum.
- Deterministic randomness via `matchlab.rng_*` (from `&mut SimRng`).
- Config + runner wiring; loop test updates; manifest game sections; tests.

**Out:**
- No change to the `OutcomeModel` trait or `MatchResult` shape.

## Design

### Lua contract (outcome model script)

```lua
function win_probability(team_a, team_b, config, context)
    -- team_a/team_b: arrays of observation tables (incl. skill_overall,
    --                skill_vector for composition)
    -- returns P(team_a wins) in [0,1]
end

function simulate(match_id, team_a, team_b, config, context)
    -- randomness via matchlab.rng_range/rng_bool/rng_normal (fed from SimRng)
    -- returns (match_result, context)
    -- match_result = {
    --   winner = "A" | "B",
    --   team_a_score, team_b_score: numbers,
    --   duration_secs: number,
    --   performances = { { player_id, kills, deaths, assists,
    --                      objective_score, impact, variance }, ... },
    --   variance: number,
    -- }
end
```

`context` is threaded exactly as in ticket 002.

### `matchlab-game::lua` — `LuaOutcomeModel`

```rust
pub struct LuaOutcomeModel { vm: LuaVm, context: Mutex<Context> }

impl LuaOutcomeModel {
    pub fn load(script: &str, params: &serde_yaml::Value) -> Result<Self, String>;
    // validate_script(path, &["win_probability", "simulate"])
}
```

`OutcomeModel` impl:
- `win_probability(a, b)` → build tables via `matchlab_lua::convert`;
  `vm.call_required("win_probability", (a_tbl, b_tbl, config, ctx))` → f64.
- `simulate(match_id, a, b, rng)` → `vm.with_rng(rng, |vm| ...)` calls
  `simulate(match_id.0, a_tbl, b_tbl, config, ctx)` → `(result_tbl, ctx)`;
  map result back to `MatchResult` (winner enum, scores, `SimTime::from_secs`
  duration, performances, variance); store `ctx`.

### Scripts (`plugins/game/`)

Port from the current Rust implementations:

- `logistic.lua` — effective skill = `obs.skill_overall` (falling back to
  `obs.rating` only when `skill_overall` is absent); team averages; P = logistic
  of avg difference / beta; `simulate` adds noise (clamped 0.01..0.99), picks the
  winner, and builds scores/duration/performances from skill.
- `variance.lua` — logistic with `variance_multiplier`-scaled noise envelope.
- `composition.lua` — effective skill per player =
  `weighted_overall(skill_vector, config.dimension_weights)`; team totals add
  `synergy_bonus` per player.
- `performance.lua` — `recent_performances` mean (scaled by
  `performance_weight * beta`) shifts effective skill.
- `fatigue.lua` — decay each player's skill by `1 - decay_rate * games_played`
  before delegating to the base logistic math (inline in Lua).
- `momentum.lua` — scale each player's skill by
  `1 + momentum_factor * (win_rate - 0.5)` before the base logistic math.

Fatigue/momentum are wrappers over the base model in Rust; in Lua they compose
inline (compute the effective skill, then apply the logistic/winner logic). The
base behavior (winner selection, score/performance generation) is shared by
duplicating the small logistic helper within each script — keep the duplication
deliberate and tiny so scripts stay self-contained.

### Deletions

- `crates/matchlab-game/src/logistic.rs`, `variance.rs`, `composition.rs`,
  `performance.rs`, `fatigue.rs`, `momentum.rs`, `hooks.rs`.
- Keep: `outcome.rs` (trait). Update `lib.rs` exports.

### Config + runner

- `config.rs` — `GameSpec` becomes:
  ```rust
  pub struct GameSpec {
      pub team_size: usize,
      pub script: String,                 // plugins/game/logistic.lua, ...
      #[serde(flatten)] pub params: HashMap<String, serde_yaml::Value>,
  }
  ```
  Remove `outcome_model`, `variant`, `beta`, `noise` dedicated fields (they
  become params: `beta`, `noise`, `variance_multiplier`, `dimension_weights`,
  `synergy_bonus`, `performance_weight`, `fatigue_decay_rate`,
  `momentum_factor`).
- `runner.rs::build_outcome_model` → `LuaOutcomeModel::load(&spec.script,
  &params)`. Drop the `matchlab_game::*` import block. Remove the
  `fatigue_outcome_variant_runs` test's `variant` usage (keep a variant test by
  pointing the script at `fatigue.lua`).
- Runner `MINI` manifest game section → `script: plugins/game/logistic.lua` +
  beta/noise.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` tests: `LogisticOutcomeModel::new(...)`
  → load `plugins/game/logistic.lua` via a test helper.
- Manifests: game sections in `v0_1_basic.yaml`, `base/standard.yaml`,
  `full_featured.yaml` (which also sets `variant: fatigue` →
  `script: plugins/game/fatigue.lua`), plus any others.
- Delete `plugins/game/fatigue_model.lua` (hook-style; superseded).

## Steps

1. Implement `lua.rs` (`LuaOutcomeModel`).
2. Write the six scripts under `plugins/game/`, porting the Rust math.
3. Delete the Rust outcome model files and `hooks.rs`; update `lib.rs`.
4. Update `config.rs` + `runner.rs`; update runner tests.
5. Update `machine.rs` tests (Lua logistic helper).
6. Update manifest game sections.
7. Write ported tests (below).
8. Update `AGENTS.md` (game crate section).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] No reference to `LogisticOutcomeModel`, `VarianceOutcomeModel`,
      `CompositionOutcomeModel`, `PerformanceOutcomeModel`,
      `FatigueOutcomeModel`, `MomentumOutcomeModel`, or `LuaHooks` remains
      (grep-clean).
- [ ] `logistic.lua` decides winners from `skill_overall` (ground truth), not
      `rating`: a 1500-skill vs 500-skill pairing gives P(high) well above 0.5
      even when both visible ratings are 1000.
- [ ] `simulate` is deterministic given a seed: two calls over the same seeded
      `SimRng` produce identical `MatchResult`s.
- [ ] Noise clamp: `simulate` never produces an absolute P outside [0.01, 0.99]
      before the winner draw.
- [ ] `fatigue.lua` / `momentum.lua` tilt win probability in the expected
      direction (high games → fatigue lowers effective skill; high win_rate →
      momentum raises it).
- [ ] `composition.lua` uses `dimension_weights` from config; the full experiment
      `full_featured.yaml` runs to completion.
- [ ] The loop's `handle_match_formed` continues to record metrics at formation
      time with the Lua outcome model.

## Testing

- Win probability: symmetric equal teams ≈ 0.5; gap of 400 → ≈ 0.73 (logistic,
  beta 400); skill-over-rating dominance test.
- Determinism: same seed → identical results; different seed → (likely)
  different winner sequence.
- Fatigue/momentum: crafted observations (high games, high win_rate) shift
  `win_probability` relative to plain logistic.
- Composition: weights concentrate skill from specific dimensions.
- Adapter: context threading; missing `simulate` function → load error.
- Runner: `fatigue_outcome_variant_runs` updated to `script:
  plugins/game/fatigue.lua` and still completes 40 matches.

## Risks / Notes

- Keep `simulate`'s score/duration/performance generation behaviorally identical
  to the current Rust code so downstream metrics (queue time, smurf damage)
  aren't perturbed by the port.
- Duplicating the logistic body across variant scripts is intentional (each
  script is a self-contained example); keep the copies in sync by extracting a
  `matchlab` helper only if the duplication grows.