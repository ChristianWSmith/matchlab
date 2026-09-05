# 008 — Port satisfaction model + rank mapper to Lua

## Summary

Make the satisfaction model and rank mapper Lua-implementable and **delete their
Rust implementations**.

- `matchlab-utility`: extract a `SatisfactionModel` **trait** (Rust math deleted),
  keep `PlayerExperience` as loop-maintained data, add `LuaSatisfactionModel`.
  The loop's `Option<SatisfactionModel>` becomes `Option<Box<dyn SatisfactionModel>>`.
- `matchlab-ranking`: add `LuaRankMapper` reading brackets from config, delete
  `BracketRankMapper`; keep `Rank`, `RankBracket`, and the `RankMapper` trait.
- Scripts: `plugins/utility/satisfaction.lua`, `plugins/ranking/brackets.lua`.

## Context

Satisfaction is the only production consumer that uses a concrete struct rather
than a trait (`MachineState.satisfaction_model: Option<SatisfactionModel>`, with
direct method calls). To make it pluggable, extract a trait with the same three
methods and switch the loop to `Box<dyn ...>`. `PlayerExperience` stays Rust — it
is loop-maintained data, not an algorithm. The satisfaction *weights* move out of
the struct into the script's `config`.

Ranking is already trait-based; only the concrete `BracketRankMapper` is
deleted. The bracket table is pure config and moves into `config.brackets`.

## Scope

**In:**
- `SatisfactionModel` trait + `LuaSatisfactionModel` adapter.
- `LuaRankMapper` adapter.
- Loop type change for satisfaction.
- Config + runner wiring; loop test updates; manifest updates; tests.

**Out:**
- `PlayerExperience` unchanged.
- `Rank`, `RankBracket`, `Leaderboard` unchanged.
- No change to `AdversarialAgent`/`RankMapper`/other traits.

## Design

### Satisfaction

**Trait extraction** (`matchlab-utility::satisfaction`):

```rust
pub trait SatisfactionModel: Send + Sync {
    fn satisfaction(&self, exp: &PlayerExperience) -> f64;
    fn retention_probability(&self, satisfaction: f64) -> f64;
    fn rematch_probability(&self, satisfaction: f64) -> f64;
}
```

`PlayerExperience` (`new()`, `record_match(...)`, fields) stays as-is.
`SatisfactionWeights` is deleted (weights become script config); check all
consumers (runner, loop tests) and remove.

**Lua contract** (`plugins/utility/satisfaction.lua`):

```lua
function satisfaction(experience, config, context)
    -- experience: { recent_match_qualities, recent_queue_times,
    --               recent_outcomes, current_streak, rank_change,
    --               perceived_fairness, rematch_rate }
    -- returns number
end

function retention_probability(satisfaction, config, context)
    -- returns 1 / (1 + exp(-satisfaction))
end

function rematch_probability(satisfaction, config, context)
    -- returns 1 / (1 + exp(-0.5 * (satisfaction - 2)))
end
```

**`LuaSatisfactionModel`** (`matchlab-utility::lua`):

```rust
pub struct LuaSatisfactionModel { vm: LuaVm, context: Mutex<Context> }
// load(script, params) — validate_script(path, &["satisfaction",
//   "retention_probability", "rematch_probability"])
```

Implements the trait by calling the three functions; context threaded as usual.

**Loop changes:**
- `machine.rs`: field `satisfaction_model: Option<Box<dyn SatisfactionModel>>`;
  `with_extras` param type; `handle_match_end` calls
  `model.satisfaction(exp)` / `model.retention_probability(s)` through the
  trait object.
- `lib.rs`: `MatchLoop::with_extras` param type.
- Runner: `build_satisfaction_model` → `Some(Box::new(LuaSatisfactionModel::load(...)))`.

### Ranking

**Lua contract** (`plugins/ranking/brackets.lua`):

```lua
function rating_to_rank(rating, config, context)
    -- config.brackets = { { tier, division, min, max }, ... }
    -- first bracket where min <= rating < max; else the LAST bracket
    -- returns { tier, division }
end

function rank_to_rating_range(rank, config, context)
    -- rank = { tier, division }
    -- returns { min, max }  (0,0 for unknown ranks)
end
```

**`LuaRankMapper`** (`matchlab-ranking::lua`):

```rust
pub struct LuaRankMapper { vm: LuaVm, context: Mutex<Context> }
// load(script, params) — validate_script(path, &["rating_to_rank",
//   "rank_to_rating_range"])
```

Implements the `RankMapper` trait by calling the two functions; converts
`Rank` ↔ `{tier, division}` tables. `Rank`/`RankBracket` serde types and the
trait stay; `BracketRankMapper` is deleted.

### Config + runner

- `config.rs`:
  - `SatisfactionSpec` → `{ enabled: bool, script: String, params }` (weights
    become params).
  - `RankingSpec` → `{ script: String, params }` with `brackets` as a params
    entry (kept as a `Vec<RankBracketSpec>` in params, or flattened into the
    script's config — prefer `params["brackets"]` so the existing YAML bracket
    list survives with minimal churn).
- `runner.rs`: `build_satisfaction_model` and `build_ranker` use the new
  loaders; pass `brackets` params through to `LuaRankMapper::load`.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` tests:
  - `low_satisfaction_schedules_quit_instead_of_requeue` constructs
    `SatisfactionModel::new(SatisfactionWeights {...})` → build a
    `LuaSatisfactionModel` from `plugins/utility/satisfaction.lua` with the same
    weights in params.
  - `ranking_updates_visible_rank_on_match_end` constructs
    `BracketRankMapper::new(brackets)` → `LuaRankMapper` with the brackets in
    params.
- `crates/matchlab-experiments/src/runner.rs` tests: satisfaction config shape.
- Manifests: `full_featured.yaml` ranking + satisfaction sections → script form.
- `matchlab-utility` lib.rs, `matchlab-ranking` lib.rs exports.

## Steps

1. Extract `SatisfactionModel` trait; keep `PlayerExperience`; delete
   `SatisfactionWeights` + Rust math.
2. Implement `LuaSatisfactionModel`; write `plugins/utility/satisfaction.lua`
   (default weights as config).
3. Update `machine.rs` + `lib.rs` to `Option<Box<dyn SatisfactionModel>>`.
4. Implement `LuaRankMapper`; write `plugins/ranking/brackets.lua`; delete
   `BracketRankMapper`.
5. Update `config.rs` + `runner.rs`; update runner tests.
6. Update machine tests (satisfaction + ranking).
7. Update manifest sections.
8. Write tests (below).
9. Update `AGENTS.md` (utility + ranking crate sections).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] No reference to `SatisfactionWeights`, `BracketRankMapper`, or the
      concrete `SatisfactionModel::new` struct usage remains (grep-clean).
- [ ] `satisfaction.lua` with default weights reproduces the Rust behavior: a
      long losing streak pushes satisfaction below the retention threshold, so
      the machine test `low_satisfaction_schedules_quit...` still passes.
- [ ] `retention_probability` / `rematch_probability` match the Rust logistic
      forms at sample points (s = 0 → 0.5; rematch at s = 2 → 0.5).
- [ ] `brackets.lua` maps ratings to the same ranks the Rust mapper produced
      (first bracket with `min <= r < max`; clamp to last; unknown rank → 0,0).
- [ ] The loop compiles and runs with `Option<Box<dyn SatisfactionModel>>`; no
      other loop behavior changes.
- [ ] `full_featured.yaml` runs with Lua satisfaction + ranking enabled.

## Testing

- Satisfaction: default-weight spot checks; streak-penalty threshold crossing
  (reuses the machine test); logistic values at known points.
- Ranking: bracket boundary cases (below min → last bracket; inside each
  bracket; above max → last bracket); `rank_to_rating_range` round-trip;
  unknown rank → (0,0).
- Adapters: context threading; missing function → load error.
- Machine tests (satisfaction + ranking) pass against the Lua scripts.
- Runner: `full_featured`-shaped config with satisfaction + ranking scripts
  completes.

## Risks / Notes

- The loop change to `Box<dyn SatisfactionModel>` is the only production
  signature change in this ticket; keep `PlayerExperience` untouched so the
  satisfaction data flow (record → evaluate) is identical.
- Keep `Rank`/`RankBracket` serde in Rust so the YAML bracket list still
  deserializes; pass it into the script as `config.brackets`.