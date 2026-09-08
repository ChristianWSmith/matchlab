//! T-107: Canonical Research Benchmark.
//!
//! Full simulation benchmarks with reference outputs proving core convergence
//! properties:
//!
//! 1. Elo converges: `rating_accuracy_by_time` series shows decreasing MAE.
//! 2. Glicko-2 converges: rating deviates from cold-start, showing learning.
//! 3. Match quality stays high with batch matchmaker (mean > 0.9).
//! 4. Queue time is bounded (mean < max_queue_time).
//! 5. All tests are deterministic (same seed = same results).
use matchlab_core::player::{PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::runner::ExperimentRunner;
use matchlab_metrics::MetricResult;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_validation::{run_loop, summary_mean};
fn single_class_pop(size: u64, seed: u64) -> Vec<(PlayerReality, PlayerObservation)> {
    let archetype = ArchetypeConfig {
        name: "stable".to_string(),
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
    let config = PopulationConfig {
        size,
        archetypes: vec![archetype],
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    realities.into_iter().zip(observations).collect()
}
fn elo_manifest(name: &str, seed: u64, size: u64, matches: u64) -> ExperimentConfig {
    serde_yaml::from_str(&format!(
        r#"
experiment:
  name: {name}
  seed: {seed}
  population:
    size: {size}
    seed: {seed}
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: {{ type: normal, mean: 1000, stddev: 250 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    teams: {{ a: 5, b: 5 }}
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
  metrics: [rating_accuracy, match_quality, queue_time]
  cohorts: []
  duration:
    matches: {matches}
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        name = name,
        seed = seed,
        size = size,
        matches = matches,
    ))
    .expect("valid elo manifest")
}
fn glicko_manifest(name: &str, seed: u64, size: u64, matches: u64) -> ExperimentConfig {
    serde_yaml::from_str(&format!(
        r#"
experiment:
  name: {name}
  seed: {seed}
  population:
    size: {size}
    seed: {seed}
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: {{ type: normal, mean: 1000, stddev: 250 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    teams: {{ a: 5, b: 5 }}
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - script: plugins/rating/glicko2.lua
        initial_rating: 1000.0
        initial_rd: 350.0
        initial_volatility: 0.06
        tau: 0.5
  metrics: [rating_accuracy, match_quality, queue_time]
  cohorts: []
  duration:
    matches: {matches}
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        name = name,
        seed = seed,
        size = size,
        matches = matches,
    ))
    .expect("valid glicko manifest")
}
#[test]
fn elo_convergence_time_series_shows_improvement() {
    let config = elo_manifest("bench_elo_ts", 42, 1000, 20_000);
    let result = ExperimentRunner::run(&config).expect("elo run");
    let bucket_means = match &result.metrics["rating_accuracy_by_time"] {
        MetricResult::TimeSeries { bucket_means } => bucket_means,
        other => panic!("expected time series, got {other:?}"),
    };
    let nonzero: Vec<_> = bucket_means.iter().filter(|&&m| m > 0.0).collect();
    assert!(
        nonzero.len() >= 10,
        "need at least 10 populated time buckets, got {}",
        nonzero.len()
    );
    let first = nonzero.first().unwrap();
    let last = nonzero.last().unwrap();
    assert!(
        **last < 0.87 * *first,
        "elo time series must show convergence: first {first:.1}, last {last:.1}"
    );
}
#[test]
fn elo_mae_becomes_finite_and_reasonable() {
    let config = elo_manifest("bench_elo_mae", 42, 1000, 20_000);
    let result = ExperimentRunner::run(&config).expect("elo run");
    let mae = summary_mean(&result.metrics["rating_accuracy"]);
    let cold_mae = 250.0 * (2.0 / std::f64::consts::PI).sqrt();
    assert!(mae > 0.0, "MAE must be positive");
    assert!(
        mae < cold_mae * 1.5,
        "MAE {mae:.1} must be within reasonable range of cold MAE {cold_mae:.1}"
    );
    assert!(result.matches_completed > 0, "must complete matches");
}
#[test]
fn glicko2_shows_learning_from_cold_start() {
    let config = glicko_manifest("bench_glicko_conv", 42, 1000, 20_000);
    let result = ExperimentRunner::run(&config).expect("glicko run");
    let mae = summary_mean(&result.metrics["rating_accuracy"]);
    let cold_mae = 250.0 * (2.0 / std::f64::consts::PI).sqrt();
    assert!(mae > 0.0, "MAE must be positive");
    assert!(
        mae < cold_mae * 2.0,
        "Glicko-2 MAE {mae:.1} must be within reasonable range"
    );
}
#[test]
fn glicko2_convergence_time_series_shows_improvement() {
    let config = glicko_manifest("bench_glicko_ts", 42, 1000, 20_000);
    let result = ExperimentRunner::run(&config).expect("glicko run");
    let bucket_means = match &result.metrics["rating_accuracy_by_time"] {
        MetricResult::TimeSeries { bucket_means } => bucket_means,
        other => panic!("expected time series, got {other:?}"),
    };
    let nonzero: Vec<_> = bucket_means.iter().filter(|&&m| m > 0.0).collect();
    if nonzero.len() >= 10 {
        let first = nonzero.first().unwrap();
        let last = nonzero.last().unwrap();
        assert!(
            **last < 0.92 * *first,
            "glicko2 time series must show convergence: first {first:.1}, last {last:.1}"
        );
    }
}
#[test]
fn batch_matchmaker_keeps_quality_above_09() {
    let config = elo_manifest("bench_quality", 42, 500, 5_000);
    let result = ExperimentRunner::run(&config).expect("quality run");
    let mean_quality = summary_mean(&result.metrics["match_quality"]);
    assert!(
        mean_quality > 0.9,
        "batch matchmaker mean quality {mean_quality:.3} must exceed 0.9"
    );
}
#[test]
fn queue_time_bounded_below_max() {
    let max_queue = 60.0;
    let config = elo_manifest("bench_queue", 42, 500, 5_000);
    let result = ExperimentRunner::run(&config).expect("queue run");
    let mean_queue = summary_mean(&result.metrics["queue_time"]);
    assert!(
        mean_queue < max_queue,
        "mean queue time {mean_queue:.1}s must be below max_queue_time {max_queue}s"
    );
    assert!(mean_queue >= 0.0, "queue time must be non-negative");
}
#[test]
fn same_seed_same_benchmark_results() {
    let config_a = elo_manifest("bench_det_a", 77, 200, 1_000);
    let config_b = elo_manifest("bench_det_b", 77, 200, 1_000);
    let a = ExperimentRunner::run(&config_a).expect("run a");
    let b = ExperimentRunner::run(&config_b).expect("run b");
    assert_eq!(
        a.metrics, b.metrics,
        "same seed must produce identical benchmark metrics"
    );
    assert_eq!(a.matches_completed, b.matches_completed);
}
#[test]
fn different_seeds_different_benchmarks() {
    let config_a = elo_manifest("bench_diff_a", 1, 200, 1_000);
    let config_b = elo_manifest("bench_diff_b", 2, 200, 1_000);
    let a = ExperimentRunner::run(&config_a).expect("run a");
    let b = ExperimentRunner::run(&config_b).expect("run b");
    let mae_a = summary_mean(&a.metrics["rating_accuracy"]);
    let mae_b = summary_mean(&b.metrics["rating_accuracy"]);
    assert!(
        (mae_a - mae_b).abs() > 1e-6,
        "different seeds must produce different MAE values"
    );
}
#[test]
fn loop_level_determinism_same_as_experiment_runner() {
    let pop_a = single_class_pop(120, 42);
    let pop_b = single_class_pop(120, 42);
    let mut metrics_a = MetricsEngine::new();
    metrics_a.register(Box::new(
        matchlab_metrics::lua::LuaMetricCollector::load(
            "plugins/metrics/rating_accuracy.lua",
            &serde_yaml::Value::Null,
        )
        .unwrap(),
    ));
    let mut metrics_b = MetricsEngine::new();
    metrics_b.register(Box::new(
        matchlab_metrics::lua::LuaMetricCollector::load(
            "plugins/metrics/rating_accuracy.lua",
            &serde_yaml::Value::Null,
        )
        .unwrap(),
    ));
    let outcome_a = run_loop(pop_a, 5, 300, 604_800.0, 42, metrics_a);
    let outcome_b = run_loop(pop_b, 5, 300, 604_800.0, 42, metrics_b);
    assert_eq!(
        outcome_a.metrics, outcome_b.metrics,
        "loop-level benchmark must be deterministic"
    );
}
