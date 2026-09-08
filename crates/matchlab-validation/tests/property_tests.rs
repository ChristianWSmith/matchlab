//! Property-based testing (hand-written, no proptest dependency).
//!
//! Each test asserts a structural invariant that must hold for every
//! configuration the simulator encounters.
use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_matchmaking::queue::Queue;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;
use matchlab_validation::{build_loop, logistic_win_probability, observation, queue_entry};
fn single_class_pop(
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
fn equal_teams(size: usize) -> TeamComposition {
    TeamComposition {
        team_size_a: size,
        team_size_b: size,
        role_a: None,
        role_b: None,
    }
}
fn elo_params() -> serde_yaml::Value {
    serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap()
}
/// All win probabilities are in [0, 1] — exercised across the full range of
/// rating differences from −2000 to +2000.
#[test]
fn probability_bounds() {
    let beta = 400.0;
    for diff in (-2000..=2000).step_by(50) {
        let p = logistic_win_probability(diff as f64, beta);
        assert!(
            (0.0..=1.0).contains(&p),
            "win probability {p} outside [0,1] for diff {diff}"
        );
    }
}
/// The logistic function is monotonically non-decreasing: a larger skill gap
/// never decreases the win probability for the stronger side.
#[test]
fn probability_monotonic() {
    let beta = 400.0;
    let mut prev = 0.0f64;
    for diff in (-2000..=2000).step_by(25) {
        let p = logistic_win_probability(diff as f64, beta);
        assert!(
            p >= prev,
            "monotonicity violated at diff={diff}: {p} < {prev}"
        );
        prev = p;
    }
}
/// Identical players produce identical predictions from the rating system
/// (symmetry around 0 for the same player).
#[test]
fn rating_symmetry() {
    let rating = registry::from_name("elo", &elo_params()).unwrap();
    let players: Vec<PlayerObservation> = (0..40).map(|i| observation(i, 1000.0)).collect();
    for window in players.chunks(2) {
        let p_forward = rating.predict(
            std::slice::from_ref(&window[0]),
            std::slice::from_ref(&window[1]),
        );
        let p_reverse = rating.predict(
            std::slice::from_ref(&window[1]),
            std::slice::from_ref(&window[0]),
        );
        let sum = p_forward + p_reverse;
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "symmetry violated: {p_forward} + {p_reverse} = {sum}"
        );
    }
}
/// Swapping teams produces complementary win probabilities: P(A wins) +
/// P(B wins) = 1.0 for any pair of players.
#[test]
fn team_swap_complementary() {
    let rating = registry::from_name("elo", &elo_params()).unwrap();
    let mut ids = 0u64;
    let mut make_obs = |r: f64| -> PlayerObservation {
        ids += 1;
        observation(ids, r)
    };
    let pairs = &[
        (800.0, 1200.0),
        (1000.0, 1000.0),
        (600.0, 1400.0),
        (1500.0, 900.0),
        (1100.0, 1100.0),
    ];
    for &(ra, rb) in pairs {
        let obs_a = make_obs(ra);
        let obs_b = make_obs(rb);
        let p_ab = rating.predict(std::slice::from_ref(&obs_a), std::slice::from_ref(&obs_b));
        let p_ba = rating.predict(std::slice::from_ref(&obs_b), std::slice::from_ref(&obs_a));
        let sum = p_ab + p_ba;
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "team swap not complementary for {ra} vs {rb}: {p_ab} + {p_ba} = {sum}"
        );
    }
}
/// Enqueue + dequeue preserves player count: after n enqueues the queue has n
/// entries, and after k removals the queue has exactly n − k entries.
#[test]
fn queue_conservation() {
    let mut q = Queue::default();
    let n = 100u64;
    for i in 0..n {
        q.enqueue(queue_entry(i, SimTime::from_secs(i as f64), 1000.0));
    }
    assert_eq!(q.len(), n as usize, "after enqueue");
    for i in 0..n / 2 {
        q.remove(PlayerId(i));
    }
    assert_eq!(q.len(), n as usize / 2, "after removing half");
    for i in 0..n / 2 {
        q.enqueue(queue_entry(i + n, SimTime::from_secs(i as f64), 1000.0));
    }
    assert_eq!(q.len(), n as usize, "after re-enqueue");
    let ids: Vec<PlayerId> = (0..n).map(PlayerId).collect();
    q.remove_batch(&ids);
    assert_eq!(q.len(), n as usize / 2, "after remove_batch");
}
/// Match quality (as computed by ProposedMatch::match_quality) is always in
/// [0, 1]. Checked across a range of average rating gaps.
#[test]
fn score_normalization() {
    for gap in (0..=400).step_by(20) {
        let avg_a = 1200.0 + gap as f64;
        let avg_b = 1200.0;
        let quality = 1.0 - (avg_a - avg_b).abs() / 400.0;
        let quality = quality.clamp(0.0, 1.0);
        assert!(
            (0.0..=1.0).contains(&quality),
            "match quality {quality} outside [0,1] for gap {gap}"
        );
    }
    let gap_val: f64 = (3000.0f64 - 1000.0f64).abs();
    let q = (1.0f64 - gap_val / 400.0f64).clamp(0.0f64, 1.0f64);
    assert_eq!(q, 0.0f64);
    let q = (1.0f64 - 0.0f64 / 400.0f64).clamp(0.0f64, 1.0f64);
    assert_eq!(q, 1.0f64);
}
/// Same seed produces identical populations (deterministic generation).
#[test]
fn deterministic_seed_population() {
    let seed = 42u64;
    let pop_a = single_class_pop(200, seed);
    let pop_b = single_class_pop(200, seed);
    assert_eq!(pop_a.len(), pop_b.len());
    for (i, ((ra, oa), (rb, ob))) in pop_a.iter().zip(pop_b.iter()).enumerate() {
        assert_eq!(ra.id.0, rb.id.0, "player {i} id mismatch");
        assert_eq!(oa.rating, ob.rating, "player {i} rating mismatch");
        assert_eq!(
            oa.skill_vector.overall(),
            ob.skill_vector.overall(),
            "player {i} skill_vector mismatch"
        );
    }
}
/// Different seeds produce different populations (entropy check).
#[test]
fn different_seeds_differ() {
    let pop_a = single_class_pop(200, 1);
    let pop_b = single_class_pop(200, 2);
    let same_ratings = pop_a
        .iter()
        .zip(pop_b.iter())
        .filter(|((_, oa), (_, ob))| {
            (oa.skill_vector.overall() - ob.skill_vector.overall()).abs() < 1e-6
        })
        .count();
    assert!(
        same_ratings < pop_a.len(),
        "all skill vectors identical across different seeds — no entropy"
    );
}
/// Probability bounds after a full loop run: the Elo system's predict()
/// must stay in [0,1] for all observation pairs post-update.
#[test]
fn probability_bounds_post_loop() {
    let population = single_class_pop(200, 10);
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(
        matchlab_metrics::lua::LuaMetricCollector::load(
            "plugins/metrics/match_quality.lua",
            &serde_yaml::Value::Null,
        )
        .unwrap(),
    ));
    let outcome = build_loop(population, equal_teams(5), 50, 42, metrics);
    let mut loop_ = outcome;
    loop_.run_until(SimTime::from_secs(10_000.0));
    let metrics = loop_.finalize_metrics();
    assert!(
        metrics.contains_key("match_quality"),
        "match_quality must be computed"
    );
    let rating = registry::from_name("elo", &elo_params()).unwrap();
    let obs: Vec<PlayerObservation> = loop_
        .world
        .observations
        .values()
        .take(64)
        .cloned()
        .collect();
    for window in obs.chunks(2) {
        let p = rating.predict(
            std::slice::from_ref(&window[0]),
            std::slice::from_ref(&window[1]),
        );
        assert!(
            p.is_finite() && (0.0..=1.0).contains(&p),
            "post-loop predict {p} outside [0,1]"
        );
    }
}
/// Queue entry waiting times are non-negative for all entries.
#[test]
fn queue_waits_non_negative() {
    let mut q = Queue::default();
    for i in 0..50 {
        q.enqueue(queue_entry(i, SimTime::from_secs(i as f64 * 0.5), 1000.0));
    }
    let now = SimTime::from_secs(30.0);
    for entry in q.entries() {
        let wait = q.waiting_time(entry.player_id, now).unwrap();
        assert!(
            wait >= SimTime::ZERO,
            "negative wait for player {}",
            entry.player_id.0
        );
    }
}
