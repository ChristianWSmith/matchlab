# 005 — Port detection systems to Lua

## Summary

Make `DetectionSystem` fully Lua-implementable and **delete the Rust smurf
detector and intervention policy**. `LuaDetectionSystem` becomes the only way to
build a detector; the smurf detector (including the escalation ladder) is ported
to `plugins/detection/smurf.lua`.

## Context

`matchlab-detection` currently has `SmurfDetector` and `InterventionPolicy`
(Rust), with no Lua support at all. The trait is unchanged; the loop calls
`observe` / `evaluate` / `recommend_action` on a `Box<dyn DetectionSystem>`.

The `InterventionAction` enum is part of the trait's return type and must stay
in Rust; only the *policy logic* (threshold ladder, escalation) moves to Lua.
Detection receives observations only — never `PlayerReality` — and smurf status
is inferred from behavior (high performance vs low rating / few games), never a
boolean flag.

## Scope

**In:**
- `LuaDetectionSystem` adapter in `matchlab-detection::lua`.
- Action-string ↔ `InterventionAction` mapping.
- Lua port of the smurf detector + intervention ladder
  (`plugins/detection/smurf.lua`).
- Config + runner wiring; manifest detection sections; tests.

**Out:**
- `InterventionAction` enum stays in Rust (in `intervention.rs`, trimmed to the
  enum only).
- `DetectionResult` shape unchanged.

## Design

### Lua contract (detection script)

```lua
-- optional global read at load: nothing required, but the script owns state in
-- context (keyed by player_id) across calls.

function observe(match_result, observations, config, context)
    -- match_result: table from matchlab-lua convert (winner, teams, performances)
    -- observations: array of participant observation tables
    -- returns context  (accumulate per-player evidence here)
end

function evaluate(player_id, observations, config, context)
    -- observations: array of observation tables (adapter provides the full
    --               participant set for the player's last match, or the queue
    --               snapshot; the script reads from its context state)
    -- returns { probability_of_anomaly, confidence, evidence = { ... } }
end

function recommend_action(result, config, context)
    -- result: { player_id, probability_of_anomaly, confidence, evidence = {...} }
    -- returns action string, one of:
    --   "None" | "AccelerateRating" | "IncreaseKFactor" | "FlagForReview" |
    --   "RestrictQueue" | "TempBan" | "Probation" | "Ban"
end
```

`context` is threaded as in tickets 002–004. The smurf script keeps per-player
`{ recent_performance, expected_performance, consecutive_anomalous,
  prior_interventions, games_played }` in `context[player_id]`, so
`recommend_action` can escalate on repeated detections.

### `matchlab-detection::lua` — `LuaDetectionSystem`

```rust
pub struct LuaDetectionSystem { vm: LuaVm, context: Mutex<Context> }

impl LuaDetectionSystem {
    pub fn load(script: &str, params: &serde_yaml::Value) -> Result<Self, String>;
    // validate_script(path, &["observe", "evaluate", "recommend_action"])
}
```

`DetectionSystem` impl:
- `observe(mr, world)` → build `mr_tbl` + participant observations via
  `matchlab_lua::convert`; `vm.call_required("observe", ...)` → `ctx`; store.
- `evaluate(player_id, world)` → build the observation table for the player
  (and participant set if available); call `evaluate(player_id, obs_tbl,
  config, ctx)` → result table; read `probability_of_anomaly`, `confidence`,
  `evidence`; store returned `ctx`.
- `recommend_action(result)` → call `recommend_action(result_tbl, config, ctx)`
  → action string; map to `InterventionAction`; store returned `ctx`.

Action mapping (string → enum) covers all `InterventionAction` variants;
unknown string → load error (validated by mapping the script's possible
outputs, or at runtime returning an error that surfaces as `Err` from
`recommend_action` — prefer load-time: scan the script for the documented set).

### Script: `plugins/detection/smurf.lua`

Port the current `SmurfDetector` + `InterventionPolicy::default_ladder`
semantics:

- Config: `sigma_threshold` (3.0), `min_anomalous_games` (5),
  `min_games_before_action`, and a `ladder` table
  `{ { probability, action }, ... }` (the default ladder: 0.3 None, 0.5
  AccelerateRating, 0.7 FlagForReview, 0.8 RestrictQueue, 0.9 TempBan, 0.95
  Probation, 0.99 Ban), `escalation_window_ticks`, `escalation_factor` (0.9).
- `observe`: per participant, expected performance scales with visible rating,
  actual from `impact` + `kills/10`; track a bounded history in context; compute
  per-game deviation vs max spread; count `consecutive_anomalous`.
- `evaluate`: smurf if `consecutive_anomalous >= min_anomalous_games`; ramp
  probability 0.7 + 0.25 * extra (cap 0.99); confidence = streak/min.
- `recommend_action`: walk the ladder (escalated thresholds via
  `escalation_factor^prior_interventions`), respect `min_games_before_action`;
  update `prior_interventions` in context when a non-None action fires.

### Deletions

- `crates/matchlab-detection/src/smurf.rs` — deleted.
- `crates/matchlab-detection/src/intervention.rs` — keep only the
  `InterventionAction` enum; delete `InterventionPolicy` /
  `PlayerInterventionState` / `apply` / `default_ladder`.
- Keep: `detector.rs` (trait + `DetectionResult`). Update `lib.rs`.

### Config + runner

- `config.rs` — `DetectionSpec` becomes:
  ```rust
  pub struct DetectionSpec {
      pub enabled: bool,
      pub script: String,                 // plugins/detection/smurf.lua
      #[serde(flatten)] pub params: serde_yaml::Mapping,
  }
  ```
  Remove `SmurfDetectionSpec` (sigma/min_games/ladder become params).
- `runner.rs::build_detection_system` → if `enabled`, `LuaDetectionSystem::load(
  &spec.script, &spec.params)`.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` test `detection_check_flags_anomalous_player`
  uses a tiny inline Rust detector implementing the trait — keep it (it is a
  test double, not an inherent system). No production consumer change.
- Manifests: detection sections in `detection_test.yaml` and
  `full_featured.yaml` → `script: plugins/detection/smurf.lua` + params
  (`min_games_before_action`, etc.).
- Delete `plugins/detection/smurf_thresholds.lua` (hook-style; superseded).

## Steps

1. Implement `lua.rs` (`LuaDetectionSystem`) + action mapping.
2. Write `plugins/detection/smurf.lua`.
3. Delete `smurf.rs`; trim `intervention.rs` to the enum; update `lib.rs`.
4. Update `config.rs` + `runner.rs`; update runner tests if any reference
   detection config.
5. Update manifest detection sections; remove the hook-style script.
6. Write tests (below).
7. Update `AGENTS.md` (detection crate section).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] No reference to `SmurfDetector` or `InterventionPolicy` remains
      (grep-clean); `InterventionAction` still exported and used by the loop.
- [ ] `smurf.lua` detects a synthetic smurf: a high-skill player with a low
      visible rating and a string of high-impact performances reaches
      `probability_of_anomaly >= 0.7` after `min_anomalous_games` anomalous
      games.
- [ ] `recommend_action` maps to the correct `InterventionAction` per ladder
      tier; `min_games_before_action` suppresses action early; repeated
      detections escalate (thresholds shrink by `escalation_factor`).
- [ ] Detection state persists across calls via context: two `evaluate` calls
      separated by `observe` calls accumulate evidence (context threading).
- [ ] Unknown action string returned by a script → load error (or `Err` from
      `recommend_action`), never a silent `None`.
- [ ] `detection_test.yaml` run completes and the `smurf` metric reports
      detection events for the smurf archetype cohort.

## Testing

- Detector behavior: craft observation/performance pairs (high impact, low
  rating) → anomaly ramp; clean players stay at low probability.
- Ladder: threshold crossing selects the right action; escalation on repeated
  detections; `min_games_before_action` gate.
- Action mapping: every string in the documented set round-trips to the enum
  and back.
- Adapter: context threading (evidence accumulates across `observe` +
  `evaluate`); missing `recommend_action` → load error.
- Loop: existing `detection_check_flags_anomalous_player` (test-double detector)
  still passes.

## Risks / Notes

- The current Rust detector reads per-match performance (`impact`, `kills`);
  ensure `matchlab-lua::convert` includes the full `player_performances` list so
  the script gets the same signals.
- The escalation policy is the same logic in a different language — port
  carefully and assert the ladder behavior directly (not just end-to-end) so a
  subtle threshold difference is caught.