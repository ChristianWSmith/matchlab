# 011 — Documentation overhaul (AGENTS.md + spec.md)

## Summary

Bring `AGENTS.md` and `docs/spec.md` in line with the Lua-native architecture.
The repo rule is that `AGENTS.md` is the source of truth for the actual repo
state and must be updated as the code changes (each ticket did its crate
sections); this ticket is the comprehensive rewrite of the architecture,
plugin-model, and current-state sections, plus the spec update.

## Context

`AGENTS.md` describes Rust implementations (`EloRatingSystem`, `Glicko2RatingSystem`,
hook-style Lua, etc.) and the hook-based plugin model. `docs/spec.md` §3.3
defines the hook tables and `lua:` registry syntax, §8–§11/§15–§16 describe Rust
implementations, §13.1 shows the old manifest schema, and §17 references the
v0.1 build order. After tickets 001–010 these are all stale.

## Scope

**In:**
- Full `AGENTS.md` rewrite (architecture, plugin model, current state, config
  format, conventions).
- `docs/spec.md` updates for the affected sections.
- Deleted-documentation sweep (references to removed Rust types, hook names,
  `lua:` syntax).

**Out:**
- No code changes. This is docs only.

## Design

### `AGENTS.md`

Rewrite or heavily amend these sections:

- **Architecture / dependency flow**: add `matchlab-lua` (core ← lua ←
  algorithm crates). Note the layer tickets' deletions.
- **Design principles**: replace the "Plugin model" description with the
  Lua-native model: pure functions + `Context` on the Rust model; determinism
  rules (`matchlab.rng_*`, `math.random` ban); truth separation unchanged
  (outcome model reads the `skill_overall` observation binding; metrics read
  reality).
- **Current State**: per-crate descriptions updated to the adapters
  (`matchlab-rating::lua::LuaRatingSystem`, etc.), script inventories under
  `plugins/`, and the manifest schema.
- **Config format section**: the script-based schema, metric-name resolution,
  optional `name:` label.
- **Conventions**: Lua script conventions (pure, no `math.random`, terse header
  comments, contract per layer).
- Remove: references to `EloRatingSystem`, `lua:elo`, hook tables,
  `InterventionPolicy`, Rust collector lists, `SatisfactionWeights`, etc.

### `docs/spec.md`

- **§3.3 Plugin model**: replace the hook tables + `lua:` registry with the
  Lua-system contract: `LuaVm`, `Context`, per-layer function contracts,
  `matchlab.rng_*` API, validation rules, and the script-path config style.
  Keep the design rules (pure scripts, no heavy math in Lua beyond what a
  user chooses, determinism, truth separation) and update them to the new
  model.
- **§3 workspace layout**: add `matchlab-lua`, update `plugins/` tree.
- **§8/§9/§10/§11/§15/§16**: the trait sections stay (they are the contracts);
  the Rust-implementation prose becomes "reference implementations live as Lua
  scripts under `plugins/<layer>/`". Mark Glicko-2/TrueSkill as implemented in
  Lua (the stub comments are stale).
- **§13.1 manifest schema**: rewrite the YAML example to the script-based
  schema.
- **§17**: update the minimal manifest snippet and the acceptance summary to
  reflect the Lua pipeline.
- Note in §2 (or a new §3.3 subsection) that "a system is a set of pure Lua
  functions over (params, context, inputs); state lives in the Rust-held
  context".

### Sweep

`rg` for stale documentation references (`lua:elo`, `on_k_factor` hook tables,
`EloRatingSystem`, `BracketRankMapper`, `InterventionPolicy`, "stub",
"todo!()" where no longer true) in `*.md`; fix every hit or note an intentional
keep.

## Steps

1. Rewrite `AGENTS.md`.
2. Update `docs/spec.md` sections listed above.
3. Run the stale-reference doc sweep.
4. Verify the docs' example YAML deserializes against the final schema (lint by
   loading it through `inherit::load` if a doc example is a full manifest).

## Acceptance Criteria

- [ ] `AGENTS.md` describes only the Lua-native architecture; no reference to a
      deleted Rust type or hook name remains.
- [ ] `docs/spec.md` §3.3 defines the Lua-system contract (functions, context,
      rng API, validation) and no longer documents `lua:` registry syntax or
      hook tables as the mechanism.
- [ ] §13.1 manifest example uses `script:` references and deserializes under
      the final config schema.
- [ ] Stale-reference grep over `*.md` is clean (all hits resolved or
      intentionally annotated).
- [ ] `AGENTS.md` "Current State" matches the actual repo (crate list incl.
      `matchlab-lua`, plugin script inventory, manifest set).

## Testing

- Doc lint: extract the YAML code block from the §13.1/§17 examples and load via
  `serde_yaml` against `ExperimentConfig` (a small integration test or manual
  `cargo run -- run` with the doc example saved to a temp file).
- Manual consistency pass: `AGENTS.md` crate descriptions match each crate's
  actual modules.

## Risks / Notes

- `spec.md` is the authoritative design document; prefer marking reference
  implementations as "shipped as Lua scripts" over deleting the algorithm
  descriptions, since the math (Glicko-2 steps, TrueSkill conditioning) is still
  the spec for the scripts.
- This ticket can overlap ticket 012's acceptance pass; do it before so the
  docs reflect the verified end state.