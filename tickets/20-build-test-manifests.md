# Ticket 20: Build, Test, Experiment Manifests, Determinism

## Context
Final integration pass. Ensure the entire workspace builds, all tests pass, new experiment manifests exercise all features, and determinism is verified.

## Scope

### Build + Test
- [ ] `cargo build --workspace` succeeds (all 14 crates)
- [ ] `cargo test --workspace` passes (all tests)
- [ ] `cargo clippy --workspace` — no warnings (or only allowed ones)
- [ ] `cargo fmt --check` — formatting is correct

### New Experiment Manifests
Create these YAML files in `experiments/`:

1. **`experiments/glicko_comparison.yaml`** — Elo vs Glicko-2 comparison
   - Same population, same matchmaking, different rating systems
   - Metrics: rating_accuracy, convergence, responsiveness, stability
   - Duration: max_matches = 50000

2. **`experiments/detection_test.yaml`** — Smurf detection test
   - Population includes smurf archetype (high skill, low initial_rating)
   - Detection enabled with default thresholds
   - Metrics: smurf, rating_accuracy, match_quality
   - Duration: max_matches = 20000

3. **`experiments/matchmaker_comparison.yaml`** — Batch vs ExpandingWindow vs Strict
   - Same population, same rating (Elo), different matchmakers
   - Metrics: match_quality, queue_time, match_inequality
   - Duration: max_matches = 30000

4. **`experiments/lua_hooks_test.yaml`** — Lua scripting test
   - Uses `lua:elo` with `plugins/rating/dynamic_elo.lua`
   - Uses `lua:batch` with `plugins/matchmaking/adaptive_quality.lua`
   - Metrics: rating_accuracy, match_quality, queue_time
   - Duration: max_matches = 10000

5. **`experiments/full_featured.yaml`** — All systems enabled
   - Detection, ranking, adversarial agents, satisfaction
   - Lua hooks for rating and matchmaking
   - All 13 metric collectors
   - Duration: max_matches = 50000

6. **`experiments/base/standard.yaml`** — Base config for inheritance
   - Standard population (10000 players, 7 archetypes)
   - Standard game model (logistic, beta=400, noise=0.05)
   - Standard matchmaking (batch, interval=10s)
   - Standard rating (Elo, k=32, initial=1000)
   - Standard duration (max_time=604800)
   - Output: directory=results/, formats=[json], plots=false, report=false

### Determinism Verification
- Run each experiment twice with same seed
- Compare output JSON files byte-for-byte (except timestamp)
- All must be identical

### Plugin Scripts
Create these example Lua scripts in `plugins/`:
- `plugins/rating/dynamic_elo.lua` — dynamic K factor
- `plugins/rating/adaptive_glicko.lua` — adaptive volatility
- `plugins/matchmaking/adaptive_quality.lua` — queue-time-aware quality
- `plugins/matchmaking/custom_formation.lua` — custom team assignment
- `plugins/game/fatigue_model.lua` — session-length-based skill decay
- `plugins/detection/smurf_thresholds.lua` — per-player sigma thresholds
- `plugins/metrics/custom_metric.lua` — custom metric collector hook

## Acceptance Criteria
- [ ] All 14 crates build without errors
- [ ] All tests pass (existing + new)
- [ ] All 6 experiment manifests run successfully
- [ ] `cargo run -- run experiments/glicko_comparison.yaml` produces valid JSON
- [ ] `cargo run -- run experiments/detection_test.yaml` produces valid JSON with smurf metrics
- [ ] `cargo run -- run experiments/matchmaker_comparison.yaml` produces valid JSON
- [ ] `cargo run -- run experiments/lua_hooks_test.yaml` produces valid JSON
- [ ] `cargo run -- run experiments/full_featured.yaml` produces valid JSON
- [ ] Determinism: each experiment run twice → identical output (except timestamp)
- [ ] All 7 Lua scripts load without errors
- [ ] `experiments/base/standard.yaml` is valid base config for inheritance

## Testing
- Run each experiment manifest and verify JSON output
- Run determinism check script:
  ```bash
  cargo run -- run experiments/v0_1_basic.yaml
  cp results/v0_1_basic.json /tmp/run1.json
  cargo run -- run experiments/v0_1_basic.yaml
  # Compare (excluding timestamp field)
  ```
- Verify Lua scripts parse and execute correctly

## Dependencies
- All previous tickets (1-19) must be complete
