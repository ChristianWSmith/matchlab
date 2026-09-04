# Ticket 03: Lua Hook Infrastructure — matchlab-game, matchlab-metrics, matchlab-detection

## Context
Add Lua hooks to the remaining three crates that need scripting: game outcome models, metric collectors, and detection systems.

## Scope

### matchlab-game
- Add `mlua` dependency
- Create `crates/matchlab-game/src/hooks.rs` — `LuaHooks` with game-specific calls
- Update `crates/matchlab-game/src/logistic.rs` — integrate `on_effective_skill`, `on_noise`, `on_post_process` hooks
- Create `plugins/game/` directory

**Hook API:**
```rust
pub fn call_effective_skill(&self, rating: f64, rd: f64, games_played: u64) -> Option<f64>;
pub fn call_noise(&self, match_duration_secs: f64, team_size: usize) -> Option<f64>;
pub fn call_post_process(&self, winner: &str, team_a_score: f64, team_b_score: f64) -> Option<(String, f64, f64)>;
```

### matchlab-metrics
- Add `mlua` dependency
- Create `crates/matchlab-metrics/src/hooks.rs` — `LuaHooks` with metric-specific calls
- Create `plugins/metrics/` directory

**Hook API:**
```rust
pub fn call_on_record(&self, winner: &str, team_a_avg: f64, team_b_avg: f64) -> Option<f64>;
pub fn call_bucket_config(&self) -> Option<Vec<f64>>;
```

### matchlab-detection
- Add `mlua` dependency
- Create `crates/matchlab-detection/src/hooks.rs` — `LuaHooks` with detection-specific calls
- Create `plugins/detection/` directory

**Hook API:**
```rust
pub fn call_anomaly_threshold(&self, player_id: u64, games_played: u64) -> Option<f64>;
pub fn call_confidence(&self, consecutive_anomalies: u64, evidence_count: usize) -> Option<f64>;
pub fn call_intervention(&self, probability: f64, prior_actions: usize) -> Option<String>;
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-game` succeeds
- [ ] `cargo build -p matchlab-metrics` succeeds
- [ ] `cargo build -p matchlab-detection` succeeds
- [ ] All existing tests pass in all three crates
- [ ] New tests for each hook point (defined → used, undefined → fallback)
- [ ] Example scripts in `plugins/game/`, `plugins/metrics/`, `plugins/detection/` load correctly
- [ ] Registry entries for `lua:logistic` (game) added

## Testing
- Unit tests for each `call_*` method (defined/undefined cases)
- Integration test: `LogisticOutcomeModel` with `on_effective_skill` hook
- Integration test: custom metric collector with `on_record` hook
- Integration test: `SmurfDetector` with `on_anomaly_threshold` hook

## Dependencies
- Tickets 01, 02 (pattern reference)
- `mlua` in workspace deps
