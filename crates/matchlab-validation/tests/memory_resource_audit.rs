//! Memory & Resource Leak Audit (T-132).
//!
//! Basic resource conservation tests that run the simulation loop and verify
//! that all resources are properly released and accounted for. These are not
//! memory-profiler replacements — they catch logic bugs where players leak,
//! matches accumulate, or queues are never drained.
use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_validation::build_loop;
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
/// Run 1000 matches and verify the loop completes without panic.
#[test]
fn resource_loop_completes_1000_matches() {
    let pop = single_class_pop(500, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 1000, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let completed = loop_.state.lock().unwrap().matches_completed;
    assert!(
        completed >= 1000,
        "expected >= 1000 matches completed, got {completed}"
    );
}
/// All players are accounted for after the loop — population count is preserved.
#[test]
fn resource_population_count_preserved() {
    let pop = single_class_pop(500, 42);
    let expected_count = pop.len();
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 100, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let state = loop_.state.lock().unwrap();
    assert_eq!(
        state.population.len(),
        expected_count,
        "population count changed: expected {expected_count}, got {}",
        state.population.len()
    );
}
/// Active matches is empty after loop completion.
#[test]
fn resource_active_matches_empty() {
    let pop = single_class_pop(200, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 50, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let state = loop_.state.lock().unwrap();
    assert!(
        state.active_matches.is_empty(),
        "active_matches not empty after loop: {} still active",
        state.active_matches.len()
    );
}
/// Queue is empty after loop completion.
#[test]
fn resource_queue_empty() {
    let pop = single_class_pop(200, 42);
    let metrics = MetricsEngine::new();
    let teams = TeamComposition {
        team_size_a: 5,
        team_size_b: 5,
        role_a: None,
        role_b: None,
    };
    let mut loop_ = build_loop(pop, teams, 50, 42, metrics);
    loop_.run_until(SimTime::from_secs(604_800.0));
    let state = loop_.state.lock().unwrap();
    assert!(
        state.queue.is_empty(),
        "queue not empty after loop: {} entries remaining",
        state.queue.len()
    );
}
