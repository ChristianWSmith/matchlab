//! Full-Stack Failure Testing — error handling.
//!
//! Verifies that malformed inputs produce useful, non-panicking error messages
//! at every layer of the stack: YAML deserialization, script loading, and
//! validation.
use matchlab_core::rng::SimRng;
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::runner::ExperimentRunner;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;
#[test]
fn missing_required_field_name_produces_useful_error() {
    let yaml = r#"
experiment:
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "missing 'name' field must fail");
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("name") || msg.contains("missing"),
        "error message should reference 'name': {msg}"
    );
}
#[test]
fn missing_required_field_seed_produces_useful_error() {
    let yaml = r#"
experiment:
  name: no_seed
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "missing 'seed' field must fail");
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("seed") || msg.contains("missing"),
        "error message should reference 'seed': {msg}"
    );
}
#[test]
fn missing_required_field_population_produces_useful_error() {
    let yaml = r#"
experiment:
  name: no_pop
  seed: 42
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "missing 'population' field must fail");
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("population") || msg.contains("missing"),
        "error message should reference 'population': {msg}"
    );
}
#[test]
fn missing_required_field_duration_produces_useful_error() {
    let yaml = r#"
experiment:
  name: no_duration
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "missing 'duration' field must fail");
}
#[test]
fn missing_required_field_output_produces_useful_error() {
    let yaml = r#"
experiment:
  name: no_output
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "missing 'output' field must fail");
}
#[test]
fn invalid_yaml_syntax_produces_useful_error() {
    let yaml = "experiment: { name: broken, seed: }";
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "invalid YAML must fail to parse");
    let msg = err.unwrap_err().to_string();
    assert!(!msg.is_empty(), "error message must not be empty");
}
#[test]
fn completely_garbage_yaml_produces_error() {
    let yaml = "this is not yaml: at all: [broken: {";
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "garbage YAML must fail");
}
#[test]
fn wrong_type_for_seed_produces_error() {
    let yaml = r#"
experiment:
  name: bad_seed
  seed: "not_a_number"
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "string seed must fail");
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("seed") || msg.contains("u64") || msg.contains("type"),
        "error message should be specific: {msg}"
    );
}
#[test]
fn wrong_type_for_team_size_produces_error() {
    let yaml = r#"
experiment:
  name: bad_team
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    teams: { a: "five", b: 5 }
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "string team size must fail");
}
#[test]
fn nonexistent_outcome_script_produces_error() {
    let params = serde_yaml::from_str("beta: 400.0\nnoise: 0.0").unwrap();
    let err = LuaOutcomeModel::load("plugins/game/nonexistent_script.lua", &params);
    assert!(err.is_err(), "nonexistent script must fail to load");
    let msg = err.err().unwrap();
    assert!(
        msg.contains("nonexistent")
            || msg.contains("script")
            || msg.contains("not found")
            || msg.contains("file"),
        "error should reference the missing script: {msg}"
    );
}
#[test]
fn nonexistent_matchmaker_script_produces_error() {
    let params = serde_yaml::from_str("").unwrap();
    let err = LuaMatchmaker::load("plugins/matchmaking/nonexistent.lua", &params);
    assert!(err.is_err(), "nonexistent matchmaker must fail");
    let msg = err.err().unwrap();
    assert!(
        msg.contains("nonexistent")
            || msg.contains("script")
            || msg.contains("not found")
            || msg.contains("file"),
        "error should reference the missing script: {msg}"
    );
}
#[test]
fn nonexistent_rating_script_produces_error() {
    let params = serde_yaml::from_str("k_factor: 32.0").unwrap();
    let err = registry::from_script("plugins/rating/nonexistent.lua", &params);
    assert!(err.is_err(), "nonexistent rating script must fail");
    let msg = err.err().unwrap();
    assert!(
        msg.contains("nonexistent")
            || msg.contains("script")
            || msg.contains("not found")
            || msg.contains("file"),
        "error should reference the missing script: {msg}"
    );
}
#[test]
fn nonexistent_metric_produces_error() {
    let yaml = r#"
experiment:
  name: bad_metric
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics:
    - nonexistent_metric_xyz
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let err = ExperimentRunner::run(&config);
    assert!(err.is_err(), "unknown metric must fail at run time");
    let msg = err.unwrap_err();
    assert!(
        msg.contains("metric") || msg.contains("nonexistent") || msg.contains("unknown"),
        "error should reference the unknown metric: {msg}"
    );
}
#[test]
fn empty_population_produces_zero_matches() {
    let config = ExperimentConfig {
        experiment: matchlab_experiments::config::ExperimentSpec {
            name: "empty_pop".to_string(),
            description: None,
            seed: 42,
            population: matchlab_experiments::config::PopulationSpec {
                size: 0,
                seed: 42,
                archetypes: vec![matchlab_experiments::config::ArchetypeSpec {
                    name: "empty".to_string(),
                    proportion: 1.0,
                    skill_distribution: matchlab_experiments::config::DistributionSpec::Normal {
                        mean: 1000.0,
                        stddev: 250.0,
                    },
                    skill_volatility: 0.0,
                    improvement_rate: 0.0,
                    play_frequency: 0.8,
                    session_length: 1800.0,
                    quit_probability: 0.0,
                    initial_rating: Some(1000.0),
                    role: None,
                }],
            },
            game: matchlab_experiments::config::GameSpec {
                teams: matchlab_experiments::config::TeamSpecs::default(),
                script: "plugins/game/logistic.lua".to_string(),
                skill_update_interval_secs: None,
                params: [
                    ("beta".to_string(), serde_yaml::Value::Number(400.0.into())),
                    ("noise".to_string(), serde_yaml::Value::Number(0.0.into())),
                ]
                .into_iter()
                .collect(),
            },
            matchmaking: matchlab_experiments::config::MatchmakingSpec {
                script: "plugins/matchmaking/batch.lua".to_string(),
                max_queue_time: 60.0,
                params: Default::default(),
            },
            rating: matchlab_experiments::config::RatingSpec {
                systems: vec![matchlab_experiments::config::RatingSystemSpec {
                    name: Some("elo".to_string()),
                    script: None,
                    params: [
                        (
                            "k_factor".to_string(),
                            serde_yaml::Value::Number(32.0.into()),
                        ),
                        (
                            "initial_rating".to_string(),
                            serde_yaml::Value::Number(1000.0.into()),
                        ),
                        ("beta".to_string(), serde_yaml::Value::Number(400.0.into())),
                    ]
                    .into_iter()
                    .collect(),
                }],
            },
            detection: None,
            ranking: None,
            metrics: vec![matchlab_experiments::config::MetricEntry::Name(
                "match_quality".to_string(),
            )],
            objectives: None,
            adversarial: None,
            satisfaction: None,
            cohorts: vec![],
            duration: matchlab_experiments::config::DurationSpec {
                matches: 100,
                max_time: 60.0,
            },
            output: matchlab_experiments::config::OutputSpec {
                directory: "results/".to_string(),
                formats: vec!["json".to_string()],
                plots: false,
                report: false,
            },
            replication: None,
        },
    };
    let result = ExperimentRunner::run(&config).expect("empty population should not panic");
    assert_eq!(
        result.matches_completed, 0,
        "empty population must produce zero completed matches"
    );
}
#[test]
fn population_generation_respects_size() {
    let archetype = ArchetypeConfig {
        name: "test".to_string(),
        proportion: 1.0,
        skill_distribution: DistributionConfig::Normal {
            mean: 1000.0,
            stddev: 250.0,
        },
        skill_volatility: 0.0,
        improvement_rate: 0.0,
        play_frequency: 0.8,
        session_length: 1800.0,
        quit_probability: 0.0,
        initial_rating: Some(1000.0),
        role: None,
        skill_dimensions: None,
        correlation: None,
        dynamics: None,
    };
    for &size in &[0u64, 1, 5, 10, 100] {
        let config = PopulationConfig {
            size,
            archetypes: vec![archetype.clone()],
        };
        let mut rng = SimRng::from_seed(42);
        let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
        assert_eq!(
            realities.len(),
            size as usize,
            "reality count mismatch for size {size}"
        );
        assert_eq!(
            observations.len(),
            size as usize,
            "observation count mismatch for size {size}"
        );
    }
}
#[test]
fn zero_team_size_in_manifest_produces_error_or_handles_gracefully() {
    let yaml = r#"
experiment:
  name: zero_team
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    teams: { a: 0, b: 5 }
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.experiment.game.teams.a.size(), 0);
    let result = ExperimentRunner::run(&config);
    if let Ok(r) = result {
        assert_eq!(
            r.matches_completed, 0,
            "zero team size must produce zero matches"
        );
    }
}
#[test]
fn both_zero_team_sizes_produces_no_matches() {
    let yaml = r#"
experiment:
  name: both_zero
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    teams: { a: 0, b: 0 }
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let result = ExperimentRunner::run(&config);
    if let Ok(r) = result {
        assert_eq!(
            r.matches_completed, 0,
            "both-zero team sizes must produce zero matches"
        );
    }
}
#[test]
fn role_with_only_one_side_configured() {
    let yaml = r#"
experiment:
  name: asymmetric_role
  seed: 42
  population:
    size: 20
    seed: 1
    archetypes:
      - name: killer
        proportion: 0.5
        skill_distribution: { type: normal, mean: 1250, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.9
        session_length: 2400.0
        quit_probability: 0.0
        role: killer
      - name: survivor
        proportion: 0.5
        skill_distribution: { type: normal, mean: 1000, stddev: 100 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.7
        session_length: 1800.0
        quit_probability: 0.0
        role: survivor
  game:
    teams: { a: { size: 1, role: killer }, b: 4 }
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 100
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        config.experiment.game.teams.a.role(),
        Some("killer".to_string())
    );
    assert_eq!(config.experiment.game.teams.b.role(), None);
    let result = ExperimentRunner::run(&config);
    let _ = result;
}
#[test]
fn role_on_both_sides_with_only_one_role_populated() {
    let yaml = r#"
experiment:
  name: one_role_only
  seed: 42
  population:
    size: 20
    seed: 1
    archetypes:
      - name: killer
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1250, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.9
        session_length: 2400.0
        quit_probability: 0.0
        role: killer
  game:
    teams: { a: { size: 1, role: killer }, b: { size: 4, role: survivor } }
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 100
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let result = ExperimentRunner::run(&config);
    if let Ok(r) = result {
        assert_eq!(
            r.matches_completed, 0,
            "only-killer population with survivor-required team must produce zero matches"
        );
    }
}
#[test]
fn empty_rating_systems_produces_error() {
    let yaml = r#"
experiment:
  name: no_rating
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems: []
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let err = ExperimentRunner::run(&config);
    assert!(err.is_err(), "empty rating systems must fail");
    let msg = err.unwrap_err();
    assert!(
        msg.contains("rating") || msg.contains("system") || msg.contains("at least one"),
        "error should mention rating systems: {msg}"
    );
}
#[test]
fn unknown_rating_system_name_produces_error() {
    let yaml = r#"
experiment:
  name: bad_rating
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: nonexistent_system_xyz
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let err = ExperimentRunner::run(&config);
    assert!(err.is_err(), "unknown rating system must fail");
    let msg = err.unwrap_err();
    assert!(
        msg.contains("nonexistent") || msg.contains("unknown") || msg.contains("rating"),
        "error should reference the unknown system: {msg}"
    );
}
#[test]
fn unknown_distribution_type_produces_error() {
    let yaml = r#"
experiment:
  name: bad_dist
  seed: 42
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: cauchy, median: 1000, scale: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
    assert!(err.is_err(), "unknown distribution type must fail");
}
#[test]
fn zero_matches_duration_produces_no_matches() {
    let yaml = r#"
experiment:
  name: zero_matches
  seed: 42
  population:
    size: 20
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 0
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let result = ExperimentRunner::run(&config).expect("zero matches should not error");
    assert_eq!(
        result.matches_completed, 0,
        "zero matches duration must produce zero completed"
    );
}
#[test]
fn zero_max_time_produces_no_matches() {
    let yaml = r#"
experiment:
  name: zero_time
  seed: 42
  population:
    size: 20
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 1000000
    max_time: 0.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    let result = ExperimentRunner::run(&config).expect("zero time should not error");
    assert_eq!(
        result.matches_completed, 0,
        "zero max_time must produce zero completed"
    );
}
#[test]
fn all_error_messages_are_non_empty() {
    let bad_configs = vec![
        "experiment:\n  seed: 1\n  population:\n    size: 1\n    seed: 1\n    archetypes: []\n  game:\n    script: plugins/game/logistic.lua\n    beta: 400.0\n  matchmaking:\n    script: plugins/matchmaking/batch.lua\n    max_queue_time: 60.0\n  rating:\n    systems: []\n  metrics: []\n  cohorts: []\n  duration:\n    matches: 1\n    max_time: 1.0\n  output:\n    directory: results/\n    formats: [json]\n    plots: false\n    report: false",
    ];
    for yaml in bad_configs {
        let err = serde_yaml::from_str::<ExperimentConfig>(yaml);
        if let Err(e) = err {
            assert!(
                !e.to_string().is_empty(),
                "error message must not be empty for yaml:\n{yaml}"
            );
        }
    }
}
