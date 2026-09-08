//! information-budget audit: spy scripts prove the budget contract is
//! enforced end-to-end. Test-only collectors live in `plugins/_test/` and error
//! loudly if the simulation hands them data outside their layer's budget.
use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team, TeamComposition};
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
use matchlab_core::rng::StreamSeeds;
use matchlab_core::time::SimTime;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_metrics::{LuaMetricCollector, MetricResult, MetricsEngine};
use matchlab_rating::plugins::registry;
use matchlab_rating::system::RatingSystem;
use matchlab_validation::{interleaved_two_class_population, observation};
use std::collections::HashMap;
fn elo_params() -> serde_yaml::Value {
    serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n")
        .expect("valid elo params")
}
fn params(yaml: &str) -> serde_yaml::Value {
    serde_yaml::from_str(yaml).expect("valid params")
}
fn population() -> Vec<(PlayerReality, PlayerObservation)> {
    interleaved_two_class_population(8, 1000.0, 1000.0, 1000.0, 3)
}
fn build_loop(
    rating: Box<dyn RatingSystem>,
    matchmaker: Box<dyn Matchmaker>,
    metrics: MetricsEngine,
    max_matches: u64,
    seed: u64,
) -> MatchLoop {
    let outcome = LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &params("beta: 400.0\nnoise: 0.0"),
    )
    .expect("outcome loads");
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
        stream_seeds: StreamSeeds::from_seed(seed),
        record_history: false,
    };
    MatchLoop::new(
        population(),
        rating,
        Box::new(outcome),
        matchmaker,
        metrics,
        config,
    )
}
fn completed(loop_: &mut MatchLoop) -> u64 {
    loop_.run_until(SimTime::from_secs(3600.0));
    loop_.state.lock().unwrap().matches_completed
}
/// The loop must hand a WinLoss-only system the sanitized result: scores and
/// duration zeroed, performances emptied, and no ground-truth skill keys in the
/// observation snapshot. `spy_rating` mirrors elo.lua; a completed run proves
/// `filter_match_result` + `into_match_result` are wired into `handle_match_end`.
#[test]
fn loop_level_winloss_sanitizes_rating_result() {
    let metrics = MetricsEngine::new();
    let rating = registry::from_script("plugins/_test/spy_rating.lua", &elo_params())
        .expect("spy rating loads");
    let matchmaker = LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null)
        .expect("batch loads");
    let mut loop_ = build_loop(rating, Box::new(matchmaker), metrics, 6, 9);
    assert!(
        completed(&mut loop_) > 0,
        "spy-rating loop must complete matches"
    );
}
/// The queue snapshot every matchmaker sees must be observations-only.
/// `spy_matchmaker` errors if any entry carries skill data; a completed run
/// proves the matchmaking convert layer leaks nothing.
#[test]
fn loop_level_queue_snapshot_is_observations_only() {
    let metrics = MetricsEngine::new();
    let rating = registry::from_name("elo", &elo_params()).expect("elo loads");
    let matchmaker =
        LuaMatchmaker::load("plugins/_test/spy_matchmaker.lua", &serde_yaml::Value::Null)
            .expect("spy matchmaker loads");
    let mut loop_ = build_loop(rating, Box::new(matchmaker), metrics, 6, 9);
    assert!(
        completed(&mut loop_) > 0,
        "spy-matchmaker loop must complete matches"
    );
}
/// Positive control: metrics are the legitimate ground-truth reader, so their
/// snapshot must carry `true_skill`/`skill_overall`/`skill_vector`.
/// `spy_collector` counts every participant it asserts; a non-zero value
/// proves every record carried reality data.
#[test]
fn metric_snapshot_carries_ground_truth() {
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(
        LuaMetricCollector::load("plugins/_test/spy_collector.lua", &serde_yaml::Value::Null)
            .expect("spy collector loads"),
    ));
    let rating = registry::from_name("elo", &elo_params()).expect("elo loads");
    let matchmaker = LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null)
        .expect("batch loads");
    let mut loop_ = build_loop(rating, Box::new(matchmaker), metrics, 6, 9);
    loop_.run_until(SimTime::from_secs(3600.0));
    let finalized = loop_.finalize_metrics();
    let count = match &finalized["spy_collector"] {
        MetricResult::Scalar(v) => *v,
        other => panic!("expected scalar, got {other:?}"),
    };
    assert!(count > 0.0, "spy_collector asserted no participants");
}
/// Negative control: the spies must be live. Bypassing `filter_match_result`
/// and calling `update` directly on an unfiltered `MatchResult` must trip the
/// leak assertion — proving the loop-level passes above are meaningful.
#[test]
#[should_panic(expected = "budget leak")]
fn unfiltered_result_trips_the_rating_spy() {
    let spy = registry::from_script("plugins/_test/spy_rating.lua", &elo_params())
        .expect("spy rating loads");
    let mr = MatchResult {
        match_id: MatchId(1),
        winner: Team::A,
        team_a: vec![PlayerId(1)],
        team_b: vec![PlayerId(2)],
        team_a_score: 13.0,
        team_b_score: 5.0,
        player_performances: vec![PlayerPerformance {
            player_id: PlayerId(1),
            stats: {
                let mut s = std::collections::HashMap::new();
                s.insert("kills".to_string(), 10.0);
                s.insert("deaths".to_string(), 2.0);
                s.insert("assists".to_string(), 4.0);
                s.insert("impact".to_string(), 0.8);
                s
            },
            variance: 0.2,
        }],
        duration: SimTime::from_secs(1800.0),
        disconnected: false,
        forfeited: false,
        variance: 0.05,
        unexpected_events: Vec::new(),
    };
    let mut obs_map = HashMap::new();
    obs_map.insert(PlayerId(1), observation(1, 1000.0));
    obs_map.insert(PlayerId(2), observation(2, 1000.0));
    spy.update(&mr, &obs_map);
}
