# Ticket 19: Extend Config Schema + Runner + CLI

## Context
Update the experiment config schema, runner, and CLI to support all new features: detection, ranking, adversarial agents, satisfaction, Lua scripts, new matchmakers, new outcome models, and all metric collectors.

## Scope

### Config Schema (`crates/matchlab-experiments/src/config.rs`)
- Add `AdversarialSpec` type:
  ```rust
  pub struct AdversarialSpec {
      pub agents: Vec<AdversarialAgentSpec>,
  }
  pub struct AdversarialAgentSpec {
      pub agent_type: String, // "booster", "deranker", "win_trader", "afk", "rating_farmer"
      pub params: HashMap<String, serde_yaml::Value>,
  }
  ```
- Add `SatisfactionSpec` type:
  ```rust
  pub struct SatisfactionSpec {
      pub enabled: bool,
      pub weights: SatisfactionWeightsSpec,
  }
  ```
- Extend `MatchmakingSpec` with `search_strategy: Option<String>`
- Extend `GameSpec` with `outcome_model_variant: Option<String>` (variance/composition/performance/fatigue/momentum)
- Extend `OutputSpec` with `report: bool` (already exists, verify)
- Verify all existing types support new fields

### Runner (`crates/matchlab-experiments/src/runner.rs`)
- Add builder functions:
  - `build_detection_system(config) -> Option<Box<dyn DetectionSystem>>`
  - `build_ranker(config) -> Option<Box<dyn RankMapper>>`
  - `build_adversarial_agents(config, rng) -> HashMap<PlayerId, Box<dyn AdversarialAgent>>`
  - `build_satisfaction_model(config) -> Option<SatisfactionModel>`
  - `build_outcome_model(config) -> Box<dyn OutcomeModel>` (support variant selection)
  - `build_matchmaker(config) -> Box<dyn Matchmaker>` (support expanding_window, strict, hub_spoke + search strategy)
- Update `register_metrics(config)` to support all 13 collector names
- Update `MatchLoop::new()` call with all new optional params
- Support `lua:*` prefix in rating system names

### CLI (`src/main.rs`)
- Update output to include:
  - Detection summary (smurfs detected, intervention actions)
  - Ranking distribution table (if ranking enabled)
  - Adversarial impact summary (if adversarial agents enabled)
  - Satisfaction/retention statistics (if satisfaction enabled)

## YAML Config Examples
```yaml
rating:
  systems:
    - name: lua:elo
      script: plugins/rating/dynamic_elo.lua
      k_factor: 32.0
      initial_rating: 1000.0
      beta: 400.0

matchmaking:
  algorithm: expanding_window
  search_strategy: greedy
  max_queue_time: 60.0
  tiers:
    - [5.0, 25.0]
    - [10.0, 50.0]

game:
  team_size: 5
  outcome_model: logistic
  outcome_model_variant: fatigue
  beta: 400.0
  noise: 0.05
  fatigue_decay_rate: 0.001

detection:
  enabled: true
  smurf:
    sigma_threshold: 3.0
    min_anomalous_games: 5

ranking:
  brackets:
    - { rank: { tier: bronze, division: 1 }, min: 0, max: 800 }
    - { rank: { tier: silver, division: 1 }, min: 800, max: 1200 }

adversarial:
  agents:
    - type: smurf
      proportion: 0.02
      skill_distribution: { type: normal, mean: 1500, stddev: 100 }
      initial_rating: 700

satisfaction:
  enabled: true
  weights:
    match_quality: 1.0
    queue_time_penalty: -0.01
    win_bonus: 0.5

metrics:
  - match_quality
  - queue_time
  - rating_accuracy
  - match_inequality
  - ndcg
  - convergence
  - responsiveness
  - stability
  - streaks
  - population_health
  - smurf
```

## Acceptance Criteria
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] YAML with `lua:elo` loads and runs
- [ ] YAML with `algorithm: expanding_window` loads and runs
- [ ] YAML with `outcome_model_variant: fatigue` loads and runs
- [ ] YAML with `detection.enabled: true` loads and runs
- [ ] YAML with `ranking.brackets` loads and runs
- [ ] All 13 metric collector names are recognized
- [ ] Unknown metric collector name → descriptive error
- [ ] CLI output includes new sections when features are enabled

## Testing
- Unit test: config parsing with all new fields
- Unit test: `build_detection_system` returns Some when enabled
- Unit test: `build_detection_system` returns None when disabled
- Unit test: `build_outcome_model` with variant = "fatigue"
- Unit test: `register_metrics` with all 13 names → no error
- Unit test: `register_metrics` with unknown name → error
- Integration test: full experiment run with all features enabled
- Integration test: determinism check (same seed → identical output)

## Dependencies
- All previous tickets (crates must exist)
- `matchlab-experiments` (existing `config.rs`, `runner.rs`)
- `matchlab-analysis` (existing `report.rs`, `export.rs`)
