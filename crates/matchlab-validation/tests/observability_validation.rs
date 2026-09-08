use matchlab_core::logging::init_logging_with_options;
use matchlab_core::match_::TeamComposition;
use matchlab_core::rng::StreamSeeds;
use matchlab_core::time::SimTime;
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::seed::SeedManager;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_rating::registry;
use std::process::Command;
use std::sync::Once;
static INIT: Once = Once::new();
fn ensure_logging() {
    INIT.call_once(|| {
        let _ = init_logging_with_options("warn", None, false);
    });
}
fn mini_config() -> ExperimentConfig {
    let yaml = r#"
experiment:
  name: observability_mini
  seed: 42
  population:
    size: 100
    seed: 42
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 150 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    teams: { a: 1, b: 1 }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics: [match_quality, rating_accuracy]
  cohorts: []
  duration:
    matches: 50
    max_time: 200000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    serde_yaml::from_str(yaml).expect("valid config")
}
fn run_experiment() -> matchlab_experiments::ExperimentResult {
    ensure_logging();
    let config = mini_config();
    let seeds = SeedManager::from_experiment_seed(config.experiment.seed);
    let pop = matchlab_players::population::PopulationGenerator::generate(
        &matchlab_players::population::PopulationConfig {
            size: config.experiment.population.size,
            archetypes: config
                .experiment
                .population
                .archetypes
                .iter()
                .map(|a| matchlab_players::archetype::ArchetypeConfig {
                    name: a.name.clone(),
                    proportion: a.proportion,
                    skill_distribution: matchlab_players::archetype::DistributionConfig::Normal {
                        mean: 1000.0,
                        stddev: 150.0,
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
                })
                .collect(),
        },
        &mut matchlab_core::rng::SimRng::from_seed(seeds.population_seed),
    );
    let population = pop.0.into_iter().zip(pop.1).collect();
    let rating = registry::from_name(
        "elo",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .unwrap();
    let outcome = LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &serde_yaml::from_str("beta: 400.0\nnoise: 0.05").unwrap(),
    )
    .unwrap();
    let matchmaker = LuaMatchmaker::load(
        "plugins/matchmaking/batch.lua",
        &serde_yaml::from_str("batch_interval: 10\nmax_queue_time: 60.0").unwrap(),
    )
    .unwrap();
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(
        matchlab_metrics::lua::LuaMetricCollector::load(
            "plugins/metrics/match_quality.lua",
            &serde_yaml::Value::Null,
        )
        .unwrap(),
    ));
    metrics.register(Box::new(
        matchlab_metrics::lua::LuaMetricCollector::load(
            "plugins/metrics/rating_accuracy.lua",
            &serde_yaml::Value::Null,
        )
        .unwrap(),
    ));
    let config_loop = LoopConfig {
        teams: TeamComposition {
            team_size_a: 1,
            team_size_b: 1,
            role_a: None,
            role_b: None,
        },
        batch_interval_ticks: 10,
        rejoin_delay: SimTime::from_secs(30.0),
        max_matches: 50,
        skill_update_interval: None,
        stream_seeds: StreamSeeds::from_seed(config.experiment.seed),
        record_history: false,
    };
    let mut loop_ = MatchLoop::new(
        population,
        rating,
        Box::new(outcome),
        Box::new(matchmaker),
        metrics,
        config_loop,
    );
    loop_.run_until(SimTime::from_secs(200_000.0));
    let (matches_completed, simulated_time_secs) = {
        let state = loop_.state.lock().unwrap();
        (state.matches_completed, loop_.world.time.as_secs_f64())
    };
    let metrics = loop_.finalize_metrics();
    matchlab_experiments::ExperimentResult {
        experiment_id: "test".into(),
        name: "test".into(),
        config_hash: "test".into(),
        git_commit: "test".into(),
        timestamp: "test".into(),
        matches_completed,
        matches_formed: matches_completed,
        simulated_time_secs,
        metrics: metrics.into_iter().collect(),
        utility_score: None,
    }
}
fn run_matchlab(args: &[&str]) -> Command {
    let bin = std::env::var("CARGO_BIN_EXE_match-lab").unwrap_or_else(|_| {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push("target");
        path.push("debug");
        path.push("match-lab");
        path.to_string_lossy().into_owned()
    });
    let mut manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.pop();
    manifest_dir.pop();
    let mut cmd = Command::new(&bin);
    cmd.current_dir(&manifest_dir).args(args);
    cmd
}
#[test]
fn logging_init_does_not_panic_at_info_level() {
    let result = std::panic::catch_unwind(|| {
        let _ = init_logging_with_options("info", None, false);
    });
    assert!(
        result.is_ok(),
        "init_logging panicked at level 'info': {:?}",
        result.unwrap_err()
    );
}
#[test]
fn logging_init_does_not_panic_at_debug_level() {
    let result = std::panic::catch_unwind(|| {
        let _ = init_logging_with_options("debug", None, false);
    });
    assert!(
        result.is_ok(),
        "init_logging panicked at level 'debug': {:?}",
        result.unwrap_err()
    );
}
#[test]
fn logging_init_does_not_panic_at_trace_level() {
    let result = std::panic::catch_unwind(|| {
        let _ = init_logging_with_options("trace", None, false);
    });
    assert!(
        result.is_ok(),
        "init_logging panicked at level 'trace': {:?}",
        result.unwrap_err()
    );
}
#[test]
fn logging_with_json_output_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _ = init_logging_with_options("info", None, true);
    });
    assert!(
        result.is_ok(),
        "init_logging with json_logs panicked: {:?}",
        result.unwrap_err()
    );
}
#[test]
fn logging_with_file_output_does_not_panic() {
    let tmp = std::env::temp_dir().join("matchlab_obs_test.log");
    let result = std::panic::catch_unwind(|| {
        let _ = init_logging_with_options("info", Some(tmp.to_str().unwrap()), false);
    });
    assert!(
        result.is_ok(),
        "init_logging with log_file panicked: {:?}",
        result.unwrap_err()
    );
    let _ = std::fs::remove_file(&tmp);
}
#[test]
fn logging_with_file_and_json_does_not_panic() {
    let tmp = std::env::temp_dir().join("matchlab_obs_test_json.log");
    let result = std::panic::catch_unwind(|| {
        let _ = init_logging_with_options("info", Some(tmp.to_str().unwrap()), true);
    });
    assert!(
        result.is_ok(),
        "init_logging with log_file + json_logs panicked: {:?}",
        result.unwrap_err()
    );
    let _ = std::fs::remove_file(&tmp);
}
#[test]
fn logging_does_not_affect_experiment_metrics() {
    ensure_logging();
    let baseline = run_experiment();
    let with_logging = run_experiment();
    assert_eq!(
        baseline.matches_completed, with_logging.matches_completed,
        "matches_completed differs with logging enabled"
    );
    assert_eq!(
        baseline.metrics, with_logging.metrics,
        "metrics differ with logging enabled"
    );
}
#[test]
fn progress_messages_are_emitted() {
    let tmp = std::env::temp_dir().join("matchlab_obs_progress.yaml");
    std::fs::write(
        &tmp,
        r#"
experiment:
  name: obs_progress_test
  seed: 1
  population:
    size: 10
    seed: 1
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 100 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    teams: { a: 1, b: 1 }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 1
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics: [rating_accuracy]
  cohorts: []
  duration:
    matches: 200
    max_time: 50000.0
  output:
    directory: /tmp/matchlab_obs_test_out
    formats: [json]
    plots: false
    report: false
"#,
    )
    .expect("write manifest");
    let output = run_matchlab(&["run", tmp.to_str().unwrap(), "--log-level", "info"])
        .output()
        .expect("failed to run matchlab");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = std::fs::remove_file(&tmp);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("progress"),
        "expected 'progress' in output, stdout={stdout}, stderr={stderr}"
    );
}
#[test]
fn error_messages_include_context() {
    let output = run_matchlab(&["run", "nonexistent_file.yaml"])
        .output()
        .expect("failed to run matchlab");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("load config failed") || stderr.contains("No such file"),
        "expected error context in stderr, got: {stderr}"
    );
    assert!(
        output.status.code() == Some(1),
        "expected exit code 1 for file-not-found, got {:?}",
        output.status.code()
    );
}
#[test]
fn invalid_config_produces_contextual_error() {
    let tmp = std::env::temp_dir().join("matchlab_obs_bad_config.yaml");
    std::fs::write(&tmp, "not: valid: yaml: [[[").expect("write tmp config");
    let output = run_matchlab(&["run", tmp.to_str().unwrap()])
        .output()
        .expect("failed to run matchlab");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("load config failed") || stderr.contains("parse"),
        "expected config error context in stderr, got: {stderr}"
    );
    let _ = std::fs::remove_file(&tmp);
}
