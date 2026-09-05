# 007 — Port adversarial agents to Lua

## Summary

Make `AdversarialAgent` fully Lua-implementable and **delete the Rust agents**.
`LuaAdversarialAgent` becomes the only way to build an agent; booster, deranker,
win_trader, afk, and rating_farmer are ported to Lua scripts under
`plugins/adversarial/`.

## Context

`matchlab-adversarial` currently has five Rust agents that implement
`tick(&mut self, player_id, world: &mut World)` and mutate reality fields
(`quit_probability`, `tilt_level`, `win_rate`, `party_id`) and read `world.rng`.
The trait is unchanged; the loop calls `agent.tick(*pid, world)` for each
participant at match end.

The whole `World` is never handed to Lua. Instead the adapter exposes a **behavior
table** — the slice of reality an agent may change — plus the player's
observation, and writes the returned behavior back to reality. Randomness is
routed from `world.rng` via `matchlab.rng_*`.

## Scope

**In:**
- `LuaAdversarialAgent` adapter in `matchlab-adversarial::lua`.
- Lua ports of afk, deranker, win_trader, booster, rating_farmer.
- Objective-string ↔ `AdversarialObjective` mapping.
- Config + runner wiring; manifest adversarial sections; tests.

**Out:**
- No change to the `AdversarialAgent` trait or `AdversarialObjective` enum
  (both stay Rust).

## Design

### Lua contract (agent script)

```lua
function tick(player_id, behavior, observation, config, context)
    -- behavior: the mutable reality slice, initially:
    --   { quit_probability, tilt_level, win_rate, party_id, is_online }
    -- observation: the player's observation table (matchlab-lua convert)
    -- randomness via matchlab.rng_* (fed from world.rng)
    -- returns (behavior, context)   -- mutate the returned behavior to act
end

function objective(config, context)
    -- returns { kind = "MaximizeRating" }
    -- kinds: "MaximizeRating" | "MinimizeGamesPlayed"
    --        | "MaximizeWinRate" | "MaintainLowRating" | "Derate"
    --        | "WinTrade"  (partner taken from config)
end
```

The adapter reads `behavior` from `world.players[player_id]`, calls `tick`,
writes the returned `behavior` back to the reality fields, and stores `context`.

### `matchlab-adversarial::lua` — `LuaAdversarialAgent`

```rust
pub struct LuaAdversarialAgent {
    vm: LuaVm,
    context: Mutex<Context>,
    objective: AdversarialObjective,
}

impl LuaAdversarialAgent {
    pub fn load(script: &str, params: &serde_yaml::Value,
                player: PlayerId) -> Result<Self, String>;
    // validate_script(path, &["tick", "objective"])
    // evaluate `objective` at load time and cache it
}
```

`AdversarialAgent` impl:
- `tick(player_id, world)` → build `behavior` from `world.players[pid]` +
  observation from `world.observations[pid]`; `vm.with_rng(&mut world.rng, |vm|
  ...)` calls `tick(pid.0, behavior_tbl, obs_tbl, config, ctx)` →
  `(behavior_tbl, ctx)`; write `quit_probability`, `tilt_level`, `win_rate`,
  `party_id`, `is_online` back; store `ctx`. (Booster/win_trader link parties by
  setting `party_id`.)
- `objective()` → cached `AdversarialObjective`.

### Scripts (`plugins/adversarial/`)

Port each Rust agent:

- `afk.lua` — `if matchlab.rng_bool(config.go_afk_probability) then behavior.quit_probability = 1.0 end`.
  Objective `MinimizeGamesPlayed`.
- `deranker.lua` — while `observation.rating > config.target_rating`,
  `behavior.quit_probability = 0.9` and `behavior.tilt_level = 1.0`.
  Objective `MaintainLowRating`.
- `win_trader.lua` — link party (`behavior.party_id = config.partner`); toggle
  `config.alternating`. Objective `WinTrade`.
- `booster.lua` — link the duo into a party (`behavior.party_id =
  config.boostee`); set `behavior.win_rate = 1.0` for the boostee (the adapter
  applies behavior per-player; the script may branch on `config.boost_target` vs
  `config.boostee`). Objective `MaximizeRating`.
- `rating_farmer.lua` — with `matchlab.rng_bool(config.quit_probability)`, set
  `behavior.quit_probability = 1.0` (queue-and-quit). Objective
  `MaximizeWinRate` with `target_games` from config.

### Deletions

- `crates/matchlab-adversarial/src/afk.rs`, `booster.rs`, `deranker.rs`,
  `win_trader.rs`, `rating_farmer.rs`.
- Keep: `agent.rs` (trait + `AdversarialObjective`). Update `lib.rs`.

### Config + runner

- `config.rs` — `AdversarialAgentSpec` becomes:
  ```rust
  pub struct AdversarialAgentSpec {
      pub player: Option<u64>,
      pub script: String,                 // plugins/adversarial/afk.lua, ...
      #[serde(flatten)] pub params: HashMap<String, serde_yaml::Value>,
  }
  ```
  Remove `agent_type` and the per-type param handling.
- `runner.rs::build_adversarial_agents` → for each spec, `LuaAdversarialAgent::load(
  &spec.script, &params, PlayerId(spec.player.unwrap_or(0)))`.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` test `adversarial_agent_ticks_modify_world`
  uses `AfkAgent::new(1.0)` → load `plugins/adversarial/afk.lua` with
  `go_afk_probability: 1.0`.
- Manifests: adversarial sections in `full_featured.yaml` → script form
  (`afk.lua`, `deranker.lua` + params).

## Steps

1. Implement `lua.rs` (`LuaAdversarialAgent`) + objective mapping.
2. Write the five scripts under `plugins/adversarial/`.
3. Delete the Rust agent files; update `lib.rs`.
4. Update `config.rs` + `runner.rs`; update runner tests.
5. Update `machine.rs` test (Lua afk helper).
6. Update manifest adversarial sections.
7. Write tests (below).
8. Update `AGENTS.md` (adversarial crate section).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] No reference to `AfkAgent`, `DerankerAgent`, `WinTraderAgent`,
      `BoosterAgent`, or `RatingFarmerAgent` remains (grep-clean).
- [ ] `afk.lua` with `go_afk_probability: 1.0` sets `quit_probability = 1.0` on
      tick (existing machine test passes against the Lua script).
- [ ] `deranker.lua` raises `quit_probability`/`tilt_level` only while the
      player's rating is above `target_rating`.
- [ ] `booster.lua` / `win_trader.lua` set the partner party link
      (`behavior.party_id`) so the loop's party-aware matchmaking sees it.
- [ ] `objective()` returns the correct `AdversarialObjective` per script
      (checked at load); `WinTrade` carries the partner from config.
- [ ] Behavior writes land back on `world.players` (reality), not observations.
- [ ] Determinism: agents draw only through `matchlab.rng_*`; same seed →
      identical behavior changes.

## Testing

- Per-agent unit tests via the adapter: build a `World` with a reality +
  observation, call `tick`, assert the reality mutation.
- Objective mapping: each script's `objective` maps to the right enum variant;
  unknown kind → load error.
- Randomness: `afk.lua` with p=0.0 never triggers; with p=1.0 always triggers;
  seeded p=0.5 run is deterministic across two identical worlds.
- Adapter: context threading; missing `tick` → load error.
- Machine test `adversarial_agent_ticks_modify_world` (updated) passes.

## Risks / Notes

- `tick` receives `&mut World`; the adapter must avoid holding the `Mutex<Lua>`
  while also borrowing `world` mutably in a conflicting way — scope the Lua call
  inside a block that finishes with `world` borrows before returning.
- Booster's original behavior ("boost the boostee's win_rate to 1.0") is a
  simplification of the spec's `todo!()`; keep the current implemented semantics
  in the port.