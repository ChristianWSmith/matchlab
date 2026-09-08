//! per-stream RNG: each stochastic subsystem (game, matchmaking, behavior)
//! draws from its own derived `SimRng`, so experiments can fix one stream while
//! varying another. The probes here prove the streams are disjoint and that the
//! loop consumes them exactly as designed.
use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerObservation, PlayerReality};
use matchlab_core::rng::StreamSeeds;
use matchlab_core::time::SimTime;
use matchlab_experiments::seed::SeedManager;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_metrics::{LuaMetricCollector, MetricResult, MetricsEngine};
use matchlab_rating::plugins::registry;
use matchlab_validation::interleaved_two_class_population;
use std::collections::HashMap;
fn elo_params() -> serde_yaml::Value {
    serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n")
        .expect("valid elo params")
}
fn params(yaml: &str) -> serde_yaml::Value {
    serde_yaml::from_str(yaml).expect("valid params")
}
fn metrics() -> MetricsEngine {
    let mut m = MetricsEngine::new();
    for name in ["match_quality", "queue_time", "rating_accuracy"] {
        m.register(Box::new(
            LuaMetricCollector::load(
                &format!("plugins/metrics/{name}.lua"),
                &serde_yaml::Value::Null,
            )
            .expect("metric loads"),
        ));
    }
    m
}
fn population() -> Vec<(PlayerReality, PlayerObservation)> {
    interleaved_two_class_population(80, 1200.0, 1000.0, 1000.0, 0)
}
fn run_loop(
    pop: Vec<(PlayerReality, PlayerObservation)>,
    streams: StreamSeeds,
    noise: f64,
    max_matches: u64,
) -> (u64, HashMap<String, MetricResult>) {
    let rating = registry::from_name("elo", &elo_params()).expect("elo loads");
    let outcome = LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &params(&format!("beta: 400.0\nnoise: {noise}")),
    )
    .expect("outcome loads");
    let matchmaker = LuaMatchmaker::load("plugins/matchmaking/batch.lua", &params("")).unwrap();
    let config = LoopConfig {
        teams: TeamComposition {
            team_size_a: 1,
            team_size_b: 1,
            role_a: None,
            role_b: None,
        },
        batch_interval_ticks: 10,
        rejoin_delay: SimTime::from_secs(30.0),
        max_matches,
        skill_update_interval: None,
        stream_seeds: streams,
        record_history: false,
    };
    let mut loop_ = MatchLoop::new(
        pop,
        rating,
        Box::new(outcome),
        Box::new(matchmaker),
        metrics(),
        config,
    );
    loop_.run_until(SimTime::from_secs(3600.0));
    let completed = loop_.state.lock().unwrap().matches_completed;
    let finalized = loop_.finalize_metrics();
    (completed, finalized)
}
#[test]
fn same_seed_same_streams_byte_identical() {
    let streams = StreamSeeds::from_seed(42);
    let a = run_loop(population(), streams, 0.1, 100);
    let b = run_loop(population(), streams, 0.1, 100);
    assert_eq!(
        a.0, b.0,
        "same experiment seed must complete the same matches"
    );
    assert_eq!(a.1, b.1, "same streams must produce byte-identical metrics");
}
#[test]
fn unused_stream_changes_leave_metrics_byte_identical() {
    let base = StreamSeeds::from_seed(42);
    let behavior_shift = StreamSeeds {
        behavior: 999,
        ..base
    };
    let matchmaker_shift = StreamSeeds {
        matchmaker: 999,
        ..base
    };
    let pop = population();
    let reference = run_loop(pop.clone(), base, 0.1, 100);
    let behavior_run = run_loop(pop.clone(), behavior_shift, 0.1, 100);
    let matchmaker_run = run_loop(pop, matchmaker_shift, 0.1, 100);
    assert_eq!(behavior_run, reference);
    assert_eq!(matchmaker_run, reference);
}
#[test]
fn stream_seeds_align_with_seed_manager() {
    let streams = StreamSeeds::from_seed(42);
    let manager = SeedManager::from_experiment_seed(42);
    assert_eq!(streams.master, manager.master_seed);
    assert_eq!(streams.game, manager.game_seed);
    assert_eq!(streams.matchmaker, manager.matchmaker_seed);
    assert_eq!(streams.behavior, manager.behavior_seed);
    assert_eq!(manager.population_seed, matchlab_core::rng::derive(42, 1));
}
#[test]
fn game_seed_change_perturbs_outcomes_only() {
    let base = StreamSeeds::from_seed(42);
    let game_shift = StreamSeeds {
        game: matchlab_core::rng::derive(42, 2) ^ 1,
        ..base
    };
    assert_ne!(game_shift.game, base.game);
    let pop = population();
    let a = run_loop(pop.clone(), base, 0.2, 100);
    let b = run_loop(pop, game_shift, 0.2, 100);
    assert_ne!(
        a.1, b.1,
        "game seed must be live: outcome draws rerandomized"
    );
}
