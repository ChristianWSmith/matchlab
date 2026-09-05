# 010 — Experiment manifest overhaul + dogfood examples

## Summary

Bring every manifest under `experiments/` to the final Lua-native schema,
replace the obsolete hook-style manifest, and add dogfood examples that prove
the refactor's value: a manifest using a **novel** rating script and a **novel**
custom metric that exist only as Lua.

## Context

The layer tickets updated the affected manifest sections incrementally. This
ticket does the final pass over all manifests, removes the obsolete
`lua_hooks_test.yaml` (hook model no longer exists), and adds examples whose
entire point is "a user can drop a new system in as a Lua file and reference it
by path" — the core motivation of the refactor.

## Scope

**In:**
- Final schema pass over all manifests.
- Replace `lua_hooks_test.yaml`.
- New dogfood examples (novel rating script + custom metric).
- CLI verification of every manifest.

**Out:**
- No code changes (unless a script bug is found; fix as part of this ticket).

## Design

### Final manifests

- `base/standard.yaml` — base config: `rating.systems[0] = { script:
  plugins/rating/elo.lua, k_factor, initial_rating, beta }`; `game: { team_size,
  script: plugins/game/logistic.lua, beta, noise }`; `matchmaking: { script:
  plugins/matchmaking/batch.lua, batch_interval, max_queue_time }`; metrics
  names unchanged.
- `v0_1_basic.yaml` — inherits nothing; set the same script references inline
  (rating elo, game logistic, matchmaking batch, metrics names).
- `glicko_comparison.yaml` — `rating.systems[0] = { script:
  plugins/rating/glicko2.lua, initial_rating, initial_rd, initial_volatility,
  tau, epsilon }`.
- `matchmaker_comparison.yaml` — `matchmaking: { script:
  plugins/matchmaking/expanding_window.lua, tiers, max_window,
  max_queue_time }`.
- `detection_test.yaml` — `detection: { enabled: true, script:
  plugins/detection/smurf.lua, min_games_before_action: 3 }` (+ optional
  `sigma_threshold`).
- `full_featured.yaml` — `game.script: plugins/game/fatigue.lua`;
  `detection.script: plugins/detection/smurf.lua`; `ranking: { script:
  plugins/ranking/brackets.lua, brackets: [...] }`; `adversarial.agents[0..1]`
  → `{ player, script: plugins/adversarial/afk.lua, go_afk_probability }` and
  `{ player, script: plugins/adversarial/deranker.lua, target_rating }`;
  `satisfaction: { enabled: true, script: plugins/utility/satisfaction.lua,
  ...weights params }`; metrics list unchanged.

### Obsolete manifest

- Delete `experiments/lua_hooks_test.yaml` (hook model removed). If a
  script-selection smoke test is still wanted, add `experiments/lua_systems_test.yaml`
  that exercises a non-default script per layer in one run (e.g. glicko2 +
  expanding_window + fatigue + smurf) to prove mixed-script selection works.

### Dogfood examples (new)

- `experiments/novel_rating.yaml` — uses a **novel rating script** that exists
  only in Lua, e.g. `plugins/rating/decay_elo.lua`: Elo update plus a configurable
  rating decay toward `initial_rating` when a player has been idle. This is a
  system with no Rust equivalent, demonstrating per-player state via `context`.
  Include a `--` README-style header comment in the script explaining the idea.
- `experiments/novel_metric.yaml` (or fold into `novel_rating.yaml`) — uses a
  **novel metric** `plugins/metrics/avg_rating_gap.lua` (e.g. mean |rating −
  rating_mean| across participants per match), proving a user can add a metric
  by dropping one Lua file and listing its name.

These become the documentation examples: "add a system by writing a `.lua` file
and referencing it by path."

## Steps

1. Pass over every manifest with the final schema; run each through the CLI.
2. Delete `lua_hooks_test.yaml`; add `lua_systems_test.yaml` if desired.
3. Write `plugins/rating/decay_elo.lua` and `plugins/metrics/avg_rating_gap.lua`;
   add the example manifests.
4. Update `AGENTS.md` (experiments section: current manifests).
5. Verify all manifests run end-to-end.

## Acceptance Criteria

- [ ] Every manifest in `experiments/` deserializes under the final schema and
      runs via `cargo run -- run experiments/<name>.yaml` (exit 0).
- [ ] `grep -r 'lua:' experiments/` is empty; no manifest references a deleted
      script.
- [ ] `lua_hooks_test.yaml` is gone (replaced/removed).
- [ ] `novel_rating.yaml` demonstrates a rating system with no Rust equivalent
      (idle decay) and runs; `avg_rating_gap` appears in its results keyed by
      the script-declared `name`.
- [ ] `full_featured.yaml` runs with all eight layers Lua-native.
- [ ] Comparison manifests (`glicko_comparison`, `matchmaker_comparison`)
      produce outputs distinguishable per-system (utility/`experiment_id`).

## Testing

- CLI smoke: run every manifest; assert exit 0 and a non-empty metrics JSON.
- Dogfood: assert `novel_rating.yaml`'s `rating_accuracy_by_time` still trends
  down (decay_elo still learns); assert the custom metric value is sane (mean
  gap in a plausible range given the population).
- Comparison: two runs of `glicko_comparison` with the same seed produce
  identical JSON (determinism holds through scripts).

## Risks / Notes

- Manifest inheritance (`base:`) interacts with script paths: paths are resolved
  relative to the workspace root, not the manifest's directory — keep all
  scripts under `plugins/` and rely on `resolve_script_path` (ticket 001).
- Do not regress the v0.1 acceptance numbers here; if a ported script drifts,
  file the fix under ticket 012's acceptance pass.