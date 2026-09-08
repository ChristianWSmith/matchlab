//! Release Candidate Soak Test (T-145).
//!
//! Extended simulation that runs 5000 matches through the full pipeline and
//! verifies metric sanity and player state invariants. The goal is to catch
//! issues that only appear at scale — divergence, NaN propagation, or
//! boundary-condition failures that are invisible in short runs.
use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_metrics::MetricResult;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_metrics::lua::LuaMetricCollector;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_validation::{build_loop, summary_mean};
fn lua_metric(name: &str) -> LuaMetricCollector {
    LuaMetricCollector::load(
        &format!("plugins/metrics/{name}.lua"),
        &serde_yaml::Value::Null,
    )
    .unwrap()
}
fn soak_metrics() -> MetricsEngine {
    let mut engine = MetricsEngine::new();
    engine.register(Box::new(lua_metric("rating_accuracy")));
    engine.register(Box::new(lua_metric("match_quality")));
    engine.register(Box::new(lua_metric("queue_time")));
    engine
}
fn soak_population(size: u64, seed: u64) -> Vec<(PlayerReality, PlayerObservation)> {
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
/// Run 5000 matches and verify no NaN or inf in any metric.
#[test]
fn soak_no_nan_or_inf_in_metrics() {
    let pop = soak_population(1000, 42);
    let metrics = soak_metrics();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 5000, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let completed = loop_.state.lock().unwrap().matches_completed;
    assert!(completed >= 5000, "only {completed} matches completed");
    let results = loop_.finalize_metrics();
    for (name, result) in &results {
        match result {
            MetricResult::Scalar(v) => {
                assert!(v.is_finite(), "metric {name} has non-finite value: {v}");
            }
            MetricResult::Summary { mean, median, .. } => {
                assert!(mean.is_finite(), "metric {name} mean is non-finite: {mean}");
                assert!(
                    median.is_finite(),
                    "metric {name} median is non-finite: {median}"
                );
            }
            MetricResult::Distribution(values) => {
                for (i, v) in values.iter().enumerate() {
                    assert!(
                        v.is_finite(),
                        "metric {name} distribution[{i}] is non-finite: {v}"
                    );
                }
            }
            MetricResult::TimeSeries { bucket_means } => {
                for (i, v) in bucket_means.iter().enumerate() {
                    assert!(
                        v.is_finite(),
                        "metric {name} timeseries[{i}] is non-finite: {v}"
                    );
                }
            }
            _ => {}
        }
    }
}
/// Verify rating_accuracy is finite and positive after 5000 matches.
#[test]
fn soak_rating_accuracy_finite_positive() {
    let pop = soak_population(1000, 42);
    let metrics = soak_metrics();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 5000, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let results = loop_.finalize_metrics();
    let accuracy = results
        .get("rating_accuracy")
        .expect("rating_accuracy metric present");
    let mean = summary_mean(accuracy);
    assert!(mean.is_finite(), "rating_accuracy is not finite: {mean}");
    assert!(mean > 0.0, "rating_accuracy is not positive: {mean}");
}
/// Verify match_quality is in [0, 1] after 5000 matches.
#[test]
fn soak_match_quality_in_unit_interval() {
    let pop = soak_population(1000, 42);
    let metrics = soak_metrics();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 5000, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let results = loop_.finalize_metrics();
    let quality = results
        .get("match_quality")
        .expect("match_quality metric present");
    match quality {
        MetricResult::Summary { mean, .. } => {
            assert!(
                *mean >= 0.0 && *mean <= 1.0,
                "match_quality mean out of [0,1]: {mean}"
            );
        }
        MetricResult::Scalar(v) => {
            assert!(
                *v >= 0.0 && *v <= 1.0,
                "match_quality scalar out of [0,1]: {v}"
            );
        }
        other => panic!("unexpected match_quality format: {other:?}"),
    }
}
/// Verify queue_time is non-negative after 5000 matches.
#[test]
fn soak_queue_time_non_negative() {
    let pop = soak_population(1000, 42);
    let metrics = soak_metrics();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 5000, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let results = loop_.finalize_metrics();
    let qt = results
        .get("queue_time")
        .expect("queue_time metric present");
    match qt {
        MetricResult::Summary { mean, p99, .. } => {
            assert!(*mean >= 0.0, "queue_time mean is negative: {mean}");
            assert!(*p99 >= 0.0, "queue_time p99 is negative: {p99}");
        }
        MetricResult::Scalar(v) => {
            assert!(*v >= 0.0, "queue_time scalar is negative: {v}");
        }
        other => panic!("unexpected queue_time format: {other:?}"),
    }
}
/// All player ratings are finite after 5000 matches.
#[test]
fn soak_all_ratings_finite() {
    let pop = soak_population(1000, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 5000, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let state = loop_.state.lock().unwrap();
    let mut non_finite = Vec::new();
    for (pid, (_, obs)) in &state.population {
        if !obs.rating.is_finite() {
            non_finite.push(format!("{}: rating={}", pid.0, obs.rating));
        }
        if !obs.rating_deviation.is_finite() {
            non_finite.push(format!("{}: RD={}", pid.0, obs.rating_deviation));
        }
    }
    assert!(
        non_finite.is_empty(),
        "non-finite ratings found: {:?}",
        non_finite
    );
}
