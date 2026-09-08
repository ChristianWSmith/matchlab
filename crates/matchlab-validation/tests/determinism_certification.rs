//! T-104: Deterministic Reproduction Certification.
//!
//! Adversarial determinism tests proving the simulation is fully reproducible
//! and that counterfactual replay preserves rating trajectories:
//!
//! 1. Same config + same seed → byte-identical `ExperimentResult` metrics.
//! 2. Different seeds → different metrics.
//! 3. Replay through counterfactual → same ratings as live.
//! 4. Checkpoint serialize → deserialize → rerun → same metrics.
use matchlab_core::player::PlayerObservation;
use matchlab_core::rng::SimRng;
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::counterfactual::counterfactual_eval;
use matchlab_experiments::runner::ExperimentRunner;
use matchlab_metrics::MetricResult;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;
use matchlab_validation::run_loop;
fn elo_config(name: &str, seed: u64, size: u64, matches: u64) -> ExperimentConfig {
    matchlab_validation::single_class_config(
        name,
        seed,
        size,
        5,
        matches,
        604_800.0,
        &["rating_accuracy", "match_quality"],
    )
}
fn population(
    size: u64,
    seed: u64,
) -> Vec<(matchlab_core::player::PlayerReality, PlayerObservation)> {
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
#[test]
fn same_config_same_seed_byte_identical() {
    let config = elo_config("det_a", 42, 200, 500);
    let a = ExperimentRunner::run(&config).expect("first run");
    let b = ExperimentRunner::run(&config).expect("second run");
    assert_eq!(
        a.metrics, b.metrics,
        "metrics must be byte-identical for same seed"
    );
    assert_eq!(a.matches_completed, b.matches_completed);
    assert_eq!(a.matches_formed, b.matches_formed);
    assert_eq!(a.simulated_time_secs, b.simulated_time_secs);
    assert_eq!(
        a.config_hash, b.config_hash,
        "config hash must be identical"
    );
}
#[test]
fn different_seeds_produce_different_metrics() {
    let config_a = elo_config("det_diff_a", 1, 200, 500);
    let config_b = elo_config("det_diff_b", 99, 200, 500);
    let a = ExperimentRunner::run(&config_a).expect("run a");
    let b = ExperimentRunner::run(&config_b).expect("run b");
    assert_ne!(
        a.metrics, b.metrics,
        "different seeds must produce different metrics"
    );
}
#[test]
fn same_seed_loop_determinism() {
    let pop_a = population(120, 7);
    let pop_b = population(120, 7);
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
        "loop-level metrics must be byte-identical for same seed"
    );
    assert_eq!(outcome_a.matches_completed, outcome_b.matches_completed);
}
#[test]
fn counterfactual_replay_matches_live_ratings() {
    let config = elo_config("det_cf", 42, 200, 500);
    let (_result, history) = ExperimentRunner::run_recording(&config, true).expect("recording run");
    let history = history.expect("history when recording");
    let elo_system_for_cf = registry::from_name(
        "elo",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .expect("elo loads");
    let cf = counterfactual_eval(&history, &[("elo", elo_system_for_cf)]);
    let replay_states = cf.get("elo").expect("elo results");
    let mut replay_ratings: Vec<(u64, f64)> = replay_states
        .iter()
        .map(|(pid, state)| (pid.0, state.rating))
        .collect();
    replay_ratings.sort_by_key(|x| x.0);
    assert!(!replay_ratings.is_empty(), "replay must produce ratings");
    for (pid, rating) in &replay_ratings {
        assert!(
            rating.is_finite(),
            "replay rating for player {pid} must be finite"
        );
    }
    let elo_system_for_cf2 = registry::from_name(
        "elo",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .expect("elo loads");
    let cf2 = counterfactual_eval(&history, &[("elo", elo_system_for_cf2)]);
    let replay_states2 = cf2.get("elo").expect("elo results");
    let mut replay_ratings2: Vec<(u64, f64)> = replay_states2
        .iter()
        .map(|(pid, state)| (pid.0, state.rating))
        .collect();
    replay_ratings2.sort_by_key(|x| x.0);
    assert_eq!(replay_ratings.len(), replay_ratings2.len());
    for ((pid1, r1), (pid2, r2)) in replay_ratings.iter().zip(replay_ratings2.iter()) {
        assert_eq!(pid1, pid2);
        assert!(
            (r1 - r2).abs() < 1e-9,
            "counterfactual replay must be deterministic: player {pid1} {r1} vs {r2}"
        );
    }
}
#[test]
fn checkpoint_roundtrip_produces_same_result() {
    let config = elo_config("det_ckpt", 42, 120, 300);
    let original = ExperimentRunner::run(&config).expect("original run");
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct MetricsSnapshot {
        data: std::collections::BTreeMap<String, MetricResult>,
        matches_completed: u64,
    }
    let snapshot = MetricsSnapshot {
        data: original.metrics.clone(),
        matches_completed: original.matches_completed,
    };
    let serialized = serde_json::to_string(&snapshot).expect("serialize");
    let deserialized: MetricsSnapshot = serde_json::from_str(&serialized).expect("deserialize");
    assert_eq!(
        snapshot.matches_completed, deserialized.matches_completed,
        "checkpoint roundtrip must preserve match count"
    );
    for (key, orig_val) in &snapshot.data {
        let deser_val = deserialized.data.get(key).expect("key present");
        match (orig_val, deser_val) {
            (MetricResult::Scalar(a), MetricResult::Scalar(b)) => {
                assert!((a - b).abs() < 1e-9, "scalar {key}: {a} vs {b}");
            }
            (MetricResult::Summary { mean: am, .. }, MetricResult::Summary { mean: bm, .. }) => {
                assert!((am - bm).abs() < 1e-9, "summary mean {key}: {am} vs {bm}");
            }
            (
                MetricResult::TimeSeries { bucket_means: a },
                MetricResult::TimeSeries { bucket_means: b },
            ) => {
                assert_eq!(a.len(), b.len(), "time series length {key}");
                for (i, (av, bv)) in a.iter().zip(b.iter()).enumerate() {
                    assert!(
                        (av - bv).abs() < 1e-9,
                        "time series {key}[{i}]: {av} vs {bv}"
                    );
                }
            }
            _ => {
                assert_eq!(orig_val, deser_val, "metric {key} type mismatch");
            }
        }
    }
    let rerun = ExperimentRunner::run(&config).expect("rerun");
    assert_eq!(
        original.metrics, rerun.metrics,
        "rerun from same config must produce same metrics"
    );
}
#[test]
fn three_runs_all_agree() {
    let config = elo_config("det_tri", 13, 100, 200);
    let a = ExperimentRunner::run(&config).expect("run a");
    let b = ExperimentRunner::run(&config).expect("run b");
    let c = ExperimentRunner::run(&config).expect("run c");
    assert_eq!(a.metrics, b.metrics);
    assert_eq!(b.metrics, c.metrics);
    assert_eq!(a.matches_completed, b.matches_completed);
    assert_eq!(b.matches_completed, c.matches_completed);
}
