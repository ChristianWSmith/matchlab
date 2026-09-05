# 009 — Config/runner consolidation + script-content hashing

## Summary

After tickets 002–008 have landed, sweep the configuration schema and the
experiment runner to their final, unified state, remove all leftover debris from
the old model, and strengthen reproducibility so that **changing a script's
contents changes the experiment's `config_hash`**.

## Context

Tickets 002–008 each migrated one layer (config struct, runner builder, tests,
manifests). By the end of 008 every layer references scripts, but the config
module and runner may still carry vestiges: unused fields, dead builder arms,
stale imports, and the old `lua:` naming remnants. `hash_config` currently hashes
only the serialized config; since algorithms now live in script files, the
experiment identity must also capture script contents.

## Scope

**In:**
- Final `config.rs` schema (sweep for leftover fields/variants).
- Final `runner.rs` builders (unified script-loading helpers; dead arms removed).
- `seed.rs` `hash_config` includes referenced script contents (path + bytes).
- Workspace-wide grep for stale references to deleted modules/types.
- `AGENTS.md` cross-check.

**Out:**
- No new algorithm functionality (this is consolidation).

## Design

### `config.rs` final shape

Verify (and enforce) that every system reference is script-based and that no
legacy field survives:

| Spec | Fields after 002–008 | Removed |
|------|----------------------|---------|
| `RatingSystemSpec` | `name: Option<String>` (label), `script`, `params` | — |
| `GameSpec` | `team_size`, `script`, `params` | `outcome_model`, `variant`, `beta`, `noise` |
| `MatchmakingSpec` | `script`, `params` | `algorithm` |
| `DetectionSpec` | `enabled`, `script`, `params` | `smurf: SmurfDetectionSpec` |
| `AdversarialAgentSpec` | `player: Option<u64>`, `script`, `params` | `agent_type` |
| `SatisfactionSpec` | `enabled`, `script`, `params` | `weights` |
| `RankingSpec` | `script`, `params` | — |

Remove `SmurfDetectionSpec`, `SatisfactionWeightsSpec`, and any unused
`serde` derives/fields the sweep turns up.

### `runner.rs` builders

- Introduce one helper shape per layer, e.g.
  `load_rating(name_or_script, params)`, `load_outcome(script, params)`,
  `load_matchmaker(script, params)`, `load_detection(...)`,
  `load_metric(name)`, `load_agent(...)`, `load_satisfaction(...)`,
  `load_ranker(...)`, each resolving to `matchlab-<crate>::lua::*::load`.
- Delete any remaining `matchlab_game::*`, `matchlab_matchmaking::*`,
  `matchlab_rating::registry::from_name("elo")`-style arms.
- Metrics resolution helper: `metric_script(name) -> path` =
  `plugins/metrics/<name>.lua`; error message kept as
  `unknown metric collector: <name>`.

### `seed.rs` — script-aware `hash_config`

Change `hash_config` so it hashes, in order:
1. The serialized config (as today).
2. For every script path referenced anywhere in the config (walk the YAML tree
   for string values starting with `plugins/` or ending in `.lua`): the resolved
   absolute path + the file's bytes.

```rust
pub fn hash_config(config: &ExperimentConfig) -> String {
    // fold: serialized config
    //   + for each referenced script path (resolved): path.as_bytes() + content.as_bytes()
}
```

`resolve_script_path` comes from `matchlab-lua`. Missing script → hash its path
with empty content (the runner will already have errored at load, but `hash_config`
should not panic). This means an uncommitted edit to `elo.lua` changes the
experiment id + hash even when the manifest is unchanged — closing the
reproducibility gap noted in the refactor design.

### Stale-reference sweep

`rg` for the following across the workspace (must be empty):
`EloRatingSystem|Glicko2RatingSystem|TrueSkillRatingSystem|FlatPointsRatingSystem`,
`LogisticOutcomeModel|VarianceOutcomeModel|CompositionOutcomeModel|PerformanceOutcomeModel|FatigueOutcomeModel|MomentumOutcomeModel`,
`BatchMatchmaker|ExpandingWindowMatchmaker|StrictMatchmaker|HubSpokeMatchmaker`,
`SmurfDetector|InterventionPolicy`, the 12 Rust collector type names,
`AfkAgent|DerankerAgent|WinTraderAgent|BoosterAgent|RatingFarmerAgent`,
`SatisfactionWeights|BracketRankMapper`, and `lua:` in any YAML manifest.

## Steps

1. Sweep `config.rs`; remove dead specs/fields/variants; fix any deserialization
   tests that referenced them.
2. Sweep `runner.rs`; unify builders; remove dead arms/imports.
3. Implement script-aware `hash_config` in `seed.rs`.
4. Run the stale-reference grep; resolve every hit.
5. Run the full verification suite; update `AGENTS.md` (experiments crate
   section) to describe the final schema + hashing.

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] Stale-reference grep for all deleted Rust types and `lua:` YAML values is
      empty.
- [ ] Every config struct references systems by `script` (or metric name
      resolved to `plugins/metrics/<name>.lua`); no legacy fields remain in
      `config.rs`.
- [ ] Editing a script body (e.g. change `k_factor` default in `elo.lua`) and
      re-running the same manifest produces a **different** `config_hash` and
      `experiment_id`; leaving the script untouched keeps them identical.
- [ ] Determinism test (same config + seed) still passes and is byte-identical
      (hashing doesn't perturb execution).
- [ ] Runner error contract preserved: missing script → `Err`, unknown metric
      name → `Err("unknown metric collector: ...")`.

## Testing

- New unit test in `seed.rs` (or runner tests): two identical configs → equal
  hash; two configs whose referenced script bodies differ (write temp scripts)
  → different hash.
- Config deserialization tests updated for the final schema (remove any legacy
  field usage).
- Full `cargo test --workspace` (all layer tickets' tests still green after the
  sweep).

## Risks / Notes

- Walking the YAML tree for `.lua` strings is heuristic; scope it to known
  script-reference fields (`script`, plus metric-name resolution) rather than
  hashing arbitrary `.lua`-looking params. Simpler and sufficient: collect
  `spec.script` values from every section + metric names.
- Keep `hash_config` deterministic (sorted collection order) so two runs in
  different processes hash identically.