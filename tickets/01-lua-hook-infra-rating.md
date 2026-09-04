# Ticket 01: Lua Hook Infrastructure — matchlab-rating

## Context
Introduce the Lua scripting layer for rating systems. This is the first crate to get Lua hooks, establishing the pattern all other crates will follow.

## Scope
- Add `mlua` dependency to `matchlab-rating/Cargo.toml` and workspace `Cargo.toml`
- Create `crates/matchlab-rating/src/hooks.rs` — `LuaHooks` struct with loader and call methods
- Create `crates/matchlab-rating/src/loader.rs` — `ScriptLoader` for batch validation
- Update `crates/matchlab-rating/src/plugins.rs` — add `lua:elo` registry entry
- Update `crates/matchlab-rating/src/elo.rs` — add `with_hooks()` constructor, integrate `on_k_factor` and `on_rating_bounds` hooks into `update()`
- Create `plugins/rating/` directory with `dynamic_elo.lua` example

## LuaHooks API
```rust
pub struct LuaHooks {
    lua: Lua,
    script_path: String,
}

impl LuaHooks {
    pub fn load(path: &str) -> Result<Self, String>;
    pub fn call_k_factor(&self, player_id: u64, rating: f64, games_played: u64, recent_win_rate: f64) -> Option<f64>;
    pub fn call_rating_bounds(&self) -> Option<(f64, f64)>;
    pub fn call_initial_rating(&self, archetype_name: &str) -> Option<f64>;
}
```

## Elo Integration
- `EloRatingSystem::with_hooks(config, hooks)` constructor
- In `update()`: `let k = hooks.call_k_factor(...).unwrap_or(self.config.k_factor)`
- In `initialize()`: check `hooks.call_rating_bounds()` for clamping

## Registry
```rust
"lua:elo" => {
    let path = config.get("script")?.as_str()?;
    let hooks = LuaHooks::load(path)?;
    Some(Box::new(EloRatingSystem::with_hooks(config, hooks)))
}
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-rating` succeeds
- [ ] `cargo test -p matchlab-rating` passes (existing tests unchanged)
- [ ] New tests: Lua hook overrides K factor when defined
- [ ] New tests: Lua hook returns None when function not defined (falls back to Rust default)
- [ ] New tests: `ScriptLoader` rejects malformed Lua files with descriptive error
- [ ] `plugins/rating/dynamic_elo.lua` loads and produces correct K factors
- [ ] YAML config with `name: lua:elo` resolves to hooked Elo system

## Testing
- Unit test: `LuaHooks::load` with valid script → Ok
- Unit test: `LuaHooks::load` with syntax error → Err with message
- Unit test: `call_k_factor` with script defining function → Some(value)
- Unit test: `call_k_factor` with script NOT defining function → None
- Unit test: `call_rating_bounds` returns (floor, ceiling) from Lua table
- Integration test: `EloRatingSystem::with_hooks` uses Lua K factor in update
- Integration test: `plugins.rs::from_name("lua:elo", config)` returns hooked system

## Dependencies
- `mlua = { version = "0.10", features = ["lua54", "vendored"] }` in workspace deps
- Existing `matchlab-core`, `matchlab-rating` crates

## Example Script
```lua
-- plugins/rating/dynamic_elo.lua
function on_k_factor(player_id, rating, games_played, recent_win_rate)
    if games_played < 10 then
        return 64.0
    elseif recent_win_rate > 0.7 then
        return 48.0
    elseif games_played > 100 then
        return 16.0
    end
    return 32.0
end

function on_rating_bounds()
    return { floor = 100.0, ceiling = 3000.0 }
end
```
