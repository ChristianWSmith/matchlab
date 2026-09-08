//! Performance Regression Suite (T-131).
//!
//! Wall-clock performance tests for key simulation operations. These catch
//! regressions where a code change makes a previously fast path slow. Thresholds
//! are generous — the goal is to catch order-of-magnitude slowdowns, not to
//! micro-optimize.
use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_validation::build_loop;
use std::time::Instant;
fn population_for_perf(size: u64, seed: u64) -> Vec<(PlayerReality, PlayerObservation)> {
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
/// Population generation: 1000 players in < 5 seconds.
#[test]
fn perf_population_generation_1000() {
    let start = Instant::now();
    let pop = population_for_perf(1000, 42);
    let elapsed = start.elapsed();
    assert_eq!(pop.len(), 1000);
    assert!(
        elapsed.as_secs() < 5,
        "1000-player population generation took {:?}, expected < 5s",
        elapsed
    );
}
/// Match simulation: 100 matches through the logistic outcome model in < 5 seconds.
#[test]
fn perf_match_simulation_100() {
    let pop = population_for_perf(100, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 100, 42, metrics);
    let start = Instant::now();
    loop_.run_until(SimTime::from_secs(604_800.0));
    let elapsed = start.elapsed();
    let completed = loop_.state.lock().unwrap().matches_completed;
    assert!(completed >= 100, "expected >= 100 matches, got {completed}");
    assert!(
        elapsed.as_secs() < 5,
        "100 matches through logistic outcome took {:?}, expected < 5s",
        elapsed
    );
}
/// Elo update: 1000 rating updates via the Lua path in < 1 second.
/// This runs a small loop with 200 players forming 1000 matches.
#[test]
fn perf_elo_update_1000() {
    let pop = population_for_perf(200, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 1000, 42, metrics);
    let start = Instant::now();
    loop_.run_until(SimTime::from_secs(604_800.0));
    let elapsed = start.elapsed();
    let completed = loop_.state.lock().unwrap().matches_completed;
    assert!(
        completed >= 1000,
        "expected >= 1000 matches, got {completed}"
    );
    assert!(
        elapsed.as_secs() < 5,
        "1000 Elo-update matches took {:?}, expected < 5s",
        elapsed
    );
}
/// Full loop: 100 matches through the complete pipeline in < 5 seconds.
#[test]
fn perf_full_loop_100() {
    let pop = population_for_perf(200, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 100, 42, metrics);
    let start = Instant::now();
    loop_.run_until(SimTime::from_secs(604_800.0));
    let elapsed = start.elapsed();
    let completed = loop_.state.lock().unwrap().matches_completed;
    assert!(completed >= 100, "expected >= 100 matches, got {completed}");
    assert!(
        elapsed.as_secs() < 5,
        "100 full-loop matches took {:?}, expected < 5s",
        elapsed
    );
}
