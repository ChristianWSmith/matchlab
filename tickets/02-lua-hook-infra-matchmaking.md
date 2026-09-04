# Ticket 02: Lua Hook Infrastructure — matchlab-matchmaking

## Context
Add Lua hooks to the matchmaking crate, following the pattern established in Ticket 01 (rating).

## Scope
- Add `mlua` dependency to `matchlab-matchmaking/Cargo.toml`
- Create `crates/matchlab-matchmaking/src/hooks.rs` — `LuaHooks` struct with matchmaking-specific call methods
- Create `crates/matchlab-matchmaking/src/loader.rs` — re-export or share loader pattern
- Update `crates/matchlab-matchmaking/src/batch.rs` — integrate `on_match_quality` and `on_accept_match` hooks
- Update `crates/matchlab-matchmaking/src/matchmaker.rs` — add `with_hooks()` to `BatchMatchmaker`
- Create `plugins/matchmaking/` directory with `adaptive_quality.lua` example

## LuaHooks API
```rust
pub struct LuaHooks {
    lua: Lua,
    script_path: String,
}

impl LuaHooks {
    pub fn load(path: &str) -> Result<Self, String>;
    pub fn call_match_quality(&self, team_a_avg: f64, team_b_avg: f64, queue_times: &[f64]) -> Option<f64>;
    pub fn call_accept_match(&self, team_a: &[u64], team_b: &[u64], quality: f64, now_secs: f64) -> Option<bool>;
    pub fn call_queue_priority(&self, rating: f64, wait_secs: f64, games_played: u64) -> Option<f64>;
    pub fn call_max_skill_diff(&self, longest_wait_secs: f64) -> Option<f64>;
}
```

## Integration Points
- `BatchMatchmaker::find_matches()`: use Lua `on_match_quality` if defined, else default formula
- After forming a `ProposedMatch`: check `on_accept_match` — reject if returns false
- Queue sorting: use `on_queue_priority` if defined for custom ordering

## Acceptance Criteria
- [ ] `cargo build -p matchlab-matchmaking` succeeds
- [ ] `cargo test -p matchlab-matchmaking` passes (existing tests unchanged)
- [ ] New tests: Lua `on_match_quality` overrides default formula
- [ ] New tests: Lua `on_accept_match` rejects low-quality matches
- [ ] `plugins/matchmaking/adaptive_quality.lua` loads and produces correct quality scores
- [ ] YAML config with `name: lua:batch` resolves to hooked matchmaker

## Testing
- Unit test: `call_match_quality` with Lua function → custom score
- Unit test: `call_match_quality` without Lua function → None (uses default)
- Unit test: `call_accept_match` returns true/false from Lua
- Integration test: `BatchMatchmaker` with hooks forms different matches than without
- Integration test: rejected matches stay in queue

## Dependencies
- Ticket 01 (pattern reference)
- `mlua` in workspace deps
- `matchlab-core`, `matchlab-matchmaking` crates

## Example Script
```lua
-- plugins/matchmaking/adaptive_quality.lua
function on_match_quality(team_a_avg, team_b_avg, queue_times)
    local diff = math.abs(team_a_avg - team_b_avg)
    local max_wait = 0
    for _, t in ipairs(queue_times) do
        if t > max_wait then max_wait = t end
    end
    local tolerance = 200.0 + max_wait * 5.0
    return 1.0 - math.min(diff / tolerance, 1.0)
end

function on_accept_match(team_a, team_b, quality, now)
    return quality > 0.85
end
```
