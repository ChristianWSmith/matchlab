//! Counterfactual evaluation (spec §13.6 + §13.8,): replay the identical
//! game history through rating systems to isolate rating-system effects from
//! matchmaking and game-model effects.
//!
//! Two layers:
//! - `counterfactual_eval` returns final `RatingState`s (the spec §13.6
//!   head-to-head, cheap and history-only).
//! - `ReplayEngine::replay` rebuilds observations and feeds the standard
//!   `MetricsEngine`, producing a full `ExperimentResult` with real
//!   `rating_accuracy` / `convergence` / `stability` / `streaks` outputs — the
//!   counterfactual study arms of spec §13.8.
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality, Region, SkillVector};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
pub use matchlab_loop::{GameHistory, RealitySnapshot};
use matchlab_metrics::{MetricResult, MetricsEngine};
use matchlab_rating::filter::filter_match_result;
use matchlab_rating::system::{RatingState, RatingSystem};
use std::collections::{BTreeMap, HashMap};
/// The metric collectors whose outputs are meaningful on a replayed trace.
/// `queue_time` / `match_quality` / `ndcg` / `population_health` depend on the
/// live queue and population dynamics that replay cannot reproduce, so replay
/// studies only ever see this subset .
pub const REPLAY_VALID_METRICS: [&str; 4] =
    ["rating_accuracy", "convergence", "stability", "streaks"];
/// Run multiple rating systems through identical history. Each system's full
/// `RatingState` (rating, RD, volatility, games played) is preserved so
/// Bayesian systems like Glicko-2 and TrueSkill update correctly across
/// matches. Each system only sees the data in its information budget
/// (WinLoss-only results are budget-sanitized before `update`).
pub fn counterfactual_eval(
    history: &GameHistory,
    systems: &[(&str, Box<dyn RatingSystem>)],
) -> HashMap<String, Vec<(PlayerId, RatingState)>> {
    let mut results = HashMap::new();
    for (name, system) in systems {
        let mut states: HashMap<PlayerId, RatingState> = HashMap::new();
        for (i, match_result) in history.matches.iter().enumerate() {
            let observations = &history.player_snapshots[i];
            for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
                if !states.contains_key(pid) {
                    states.insert(
                        *pid,
                        observations
                            .get(pid)
                            .map(first_state)
                            .unwrap_or_else(|| system.initialize(*pid)),
                    );
                }
            }
            let budget = system.information_budget();
            let filtered =
                filter_match_result(match_result, &budget).into_match_result(match_result.match_id);
            let updates = system.update(&filtered, observations);
            for (pid, state) in updates {
                states.insert(pid, state);
            }
        }
        results.insert(name.to_string(), states.into_iter().collect());
    }
    results
}
/// Replay a recorded `GameHistory` through one rating system and fold the
/// standard metric collectors over the replay's ratings (spec §13.8,).
///
/// The replay maintains per-participant `RatingState` across every match,
/// sanitizes each update to the system's information budget, and rebuilds the
/// per-player observations so `rating_accuracy` / `convergence` / `stability` /
/// `streaks` see the replay's ratings against the recorded ground truth. Only
/// the replay-valid collectors are registered — N/A metrics are silently
/// omitted. Fully deterministic (no RNG is consulted).
///
/// Start-state contract (regression: replay vs live drift): a player's first
/// replay state comes from the first recorded observation — the exact input the
/// live loop feeds its first `update` (the population-generated rating, which
/// equals the sampled skill unless an `initial_rating` config overrides it) —
/// never from the rating system's cold `initialize`, which the live loop never
/// materializes.
pub struct ReplayEngine;
impl ReplayEngine {
    pub fn replay(
        history: &GameHistory,
        system: &dyn RatingSystem,
        config: &crate::config::ExperimentConfig,
        requested_metrics: &[crate::config::MetricEntry],
    ) -> Result<crate::runner::ExperimentResult, String> {
        if history.is_empty() {
            return Err("counterfactual replay requires a non-empty history".to_string());
        }
        let mut engine = MetricsEngine::new();
        let valid: Vec<crate::config::MetricEntry> = requested_metrics
            .iter()
            .filter(|entry| {
                let name = match entry {
                    crate::config::MetricEntry::Name(n) => n.as_str(),
                    crate::config::MetricEntry::Script { .. } => return true,
                };
                REPLAY_VALID_METRICS.contains(&name)
            })
            .cloned()
            .collect();
        crate::runner::register_metrics(&mut engine, &valid)?;
        let mut world = World::new(SimRng::from_seed(0));
        let mut states: HashMap<PlayerId, RatingState> = HashMap::new();
        for (i, match_result) in history.matches.iter().enumerate() {
            let snapshot = &history.player_snapshots[i];
            let reality = history.reality_snapshots.get(i);
            for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
                states.entry(*pid).or_insert_with(|| {
                    snapshot
                        .get(pid)
                        .map(first_state)
                        .unwrap_or_else(|| system.initialize(*pid))
                });
                let st = &states[pid];
                let base = snapshot
                    .get(pid)
                    .cloned()
                    .unwrap_or_else(|| default_observation(*pid, st.rating));
                let role = base.role.clone();
                let party_id = base.party_id;
                world.observations.insert(*pid, with_state(base, st));
                if let Some(rs) = reality.and_then(|m| m.get(pid)) {
                    world.players.insert(
                        *pid,
                        PlayerReality {
                            id: *pid,
                            skill: SkillVector::one_dimensional(rs.true_skill),
                            skill_volatility: 0.0,
                            improvement_rate: rs.improvement_rate,
                            consistency: 0.0,
                            play_frequency: 0.0,
                            session_length: 0.0,
                            quit_probability: 0.0,
                            party_id,
                            region: Region::NA,
                            location: matchlab_core::player::GeoLocation::new(Region::NA, 0.0, 0.0),
                            account_age: 0,
                            games_played: rs.games_played,
                            fatigue: 0.0,
                            tilt: 0.0,
                            experience: 0,
                            is_online: true,
                            archetype: String::new(),
                            role,
                        },
                    );
                }
            }
            world.time = SimTime::from_secs(history.times_secs.get(i).copied().unwrap_or(0.0));
            engine.record_match(match_result, &world);
            let budget = system.information_budget();
            let filtered =
                filter_match_result(match_result, &budget).into_match_result(match_result.match_id);
            let updates = system.update(&filtered, snapshot);
            for (pid, state) in updates {
                states.insert(pid, state);
            }
        }
        engine.finalize();
        let metrics: BTreeMap<String, MetricResult> =
            engine.results().clone().into_iter().collect();
        let matches = history.matches.len() as u64;
        let config_hash = crate::seed::hash_config(config);
        Ok(crate::runner::ExperimentResult {
            experiment_id: format!("{}-{}", config.experiment.name, config_hash),
            name: config.experiment.name.clone(),
            config_hash,
            git_commit: crate::seed::git_commit_hash(),
            timestamp: crate::runner::iso8601_utc(),
            matches_completed: matches,
            matches_formed: matches,
            simulated_time_secs: history.times_secs.last().copied().unwrap_or(0.0),
            metrics,
            utility_score: None,
        })
    }
}
fn default_observation(id: PlayerId, rating: f64) -> PlayerObservation {
    PlayerObservation {
        id,
        rating,
        hidden_mmr: rating,
        visible_rank: matchlab_core::player::VisibleRank {
            tier: "unranked".to_string(),
            division: 1,
        },
        rating_deviation: 350.0,
        volatility: 0.06,
        games_played: 0,
        win_rate: 0.5,
        recent_performances: Vec::new(),
        queue_joined_at: None,
        is_online: true,
        party_id: None,
        session_history: std::collections::VecDeque::new(),
        quit_history: std::collections::VecDeque::new(),
        tilt_level: 0.0,
        game_mode: "ranked".to_string(),
        skill_vector: SkillVector::one_dimensional(rating),
        detection_flags: Vec::new(),
        role: None,
    }
}
/// Overlay the replay's evolving `RatingState` onto a recorded observation.
fn with_state(obs: PlayerObservation, st: &RatingState) -> PlayerObservation {
    PlayerObservation {
        rating: st.rating,
        rating_deviation: st.rating_deviation,
        volatility: st.volatility,
        games_played: st.games_played,
        ..obs
    }
}
/// Replay start state mirroring the population-generated observation the live
/// loop feeds its first `update`: the observed rating/RD/volatility, with a
/// zero games-played count (no prior matches have been recorded for a player
/// first seen at match `i`).
fn first_state(obs: &PlayerObservation) -> RatingState {
    RatingState {
        rating: obs.rating,
        rating_deviation: obs.rating_deviation,
        volatility: obs.volatility,
        games_played: 0,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, MatchResult, Team};
    use matchlab_core::player::{DetectionFlag, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use matchlab_core::world::World;
    use matchlab_rating::registry;
    use std::collections::VecDeque;
    fn lua_elo() -> Box<dyn RatingSystem> {
        let params =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0").unwrap();
        registry::from_script("plugins/rating/elo.lua", &params).expect("elo.lua loads")
    }
    fn lua_flat() -> Box<dyn RatingSystem> {
        let params =
            serde_yaml::from_str("win_points: 10.0\nloss_points: 10.0\ninitial_rating: 1000.0")
                .unwrap();
        registry::from_script("plugins/rating/flat.lua", &params).expect("flat.lua loads")
    }
    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::<DetectionFlag>::new(),
            role: None,
        }
    }
    fn mr(id: u64, a: PlayerId, b: PlayerId, winner: Team) -> MatchResult {
        MatchResult {
            match_id: MatchId(id),
            winner,
            team_a: vec![a],
            team_b: vec![b],
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }
    fn history() -> GameHistory {
        let mut world = World::new(SimRng::from_seed(1));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        let mut h = GameHistory::new();
        h.record(&mr(1, PlayerId(1), PlayerId(2), Team::A), &world);
        h.record(&mr(2, PlayerId(1), PlayerId(2), Team::B), &world);
        h.record(&mr(3, PlayerId(1), PlayerId(2), Team::A), &world);
        h
    }
    #[test]
    fn same_system_twice_is_identical() {
        let h = history();
        let sys_a = lua_elo();
        let sys_b = lua_elo();
        let a = counterfactual_eval(&h, &[("elo", sys_a)]);
        let b = counterfactual_eval(&h, &[("elo", sys_b)]);
        assert_eq!(a["elo"].len(), b["elo"].len());
        for (pid, state_a) in &a["elo"] {
            let state_b = &b["elo"].iter().find(|(p, _)| p == pid).unwrap().1;
            assert!((state_a.rating - state_b.rating).abs() < 1e-9);
            assert_eq!(state_a.games_played, state_b.games_played);
        }
    }
    #[test]
    fn different_systems_produce_different_results() {
        let h = history();
        let elo = lua_elo();
        let flat = lua_flat();
        let a = counterfactual_eval(&h, &[("elo", elo)]);
        let b = counterfactual_eval(&h, &[("flat", flat)]);
        let a_first = a["elo"][0].1.rating;
        let b_first = b["flat"][0].1.rating;
        assert!(
            (a_first - b_first).abs() > 1e-6,
            "Elo {a_first} vs Flat {b_first} should differ"
        );
    }
    #[test]
    fn winner_ends_higher_than_loser() {
        let h = history();
        let elo = lua_elo();
        let res = counterfactual_eval(&h, &[("elo", elo)]);
        let states: HashMap<PlayerId, _> = res["elo"].iter().cloned().collect();
        assert!(states[&PlayerId(1)].rating > states[&PlayerId(2)].rating);
    }
    #[test]
    fn game_history_records_and_roundtrips() {
        let mut world = World::new(SimRng::from_seed(2));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        let mut h = GameHistory::new();
        h.record(&mr(1, PlayerId(1), PlayerId(2), Team::A), &world);
        assert!(!h.is_empty());
        assert_eq!(h.matches.len(), 1);
        assert_eq!(h.player_snapshots[0].len(), 2);
        assert!(h.reality_snapshots[0].is_empty());
    }
    fn replay_config() -> crate::config::ExperimentConfig {
        serde_yaml::from_str(
            r#"
experiment:
  name: replay-arm
  seed: 1
  population:
    size: 2
    seed: 1
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
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
  metrics: [rating_accuracy]
  cohorts: []
  duration:
    matches: 6
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        )
        .expect("valid replay config")
    }
    fn lua_spy() -> Box<dyn RatingSystem> {
        let params =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0").unwrap();
        registry::from_script("plugins/_test/spy_rating.lua", &params).expect("spy loads")
    }
    #[test]
    fn replay_produces_result_with_only_valid_metrics() {
        let h = history();
        let sys = lua_elo();
        let res = ReplayEngine::replay(
            &h,
            sys.as_ref(),
            &replay_config(),
            &[
                crate::config::MetricEntry::Name("rating_accuracy".to_string()),
                crate::config::MetricEntry::Name("queue_time".to_string()),
            ],
        )
        .expect("replay succeeds");
        assert_eq!(res.matches_completed, 3);
        assert_eq!(res.matches_formed, 3);
        assert_eq!(res.name, "replay-arm");
        assert!(res.metrics.contains_key("rating_accuracy"));
        assert!(
            !res.metrics.contains_key("queue_time"),
            "queue_time is not replay-valid"
        );
    }
    #[test]
    fn replay_same_system_twice_is_identical() {
        let h = history();
        let sys_a = lua_elo();
        let sys_b = lua_elo();
        let cfg = replay_config();
        let mut a = ReplayEngine::replay(
            &h,
            sys_a.as_ref(),
            &cfg,
            &[crate::config::MetricEntry::Name(
                "rating_accuracy".to_string(),
            )],
        )
        .expect("replay a");
        let mut b = ReplayEngine::replay(
            &h,
            sys_b.as_ref(),
            &cfg,
            &[crate::config::MetricEntry::Name(
                "rating_accuracy".to_string(),
            )],
        )
        .expect("replay b");
        a.timestamp.clear();
        b.timestamp.clear();
        assert_eq!(a, b, "same system + history must replay identically");
    }
    #[test]
    fn replay_sanitizes_update_to_budget() {
        let h = history();
        let sys = lua_spy();
        let res = ReplayEngine::replay(
            &h,
            sys.as_ref(),
            &replay_config(),
            &[crate::config::MetricEntry::Name(
                "rating_accuracy".to_string(),
            )],
        )
        .expect("spy-rating replay must pass sanitized updates");
        assert_eq!(res.matches_completed, 3);
    }
    #[test]
    fn replay_empty_history_errors() {
        let sys = lua_elo();
        let h = GameHistory::new();
        let err = ReplayEngine::replay(&h, sys.as_ref(), &replay_config(), &[]);
        assert!(err.is_err());
    }
    #[test]
    fn replay_rating_tracks_live_recording() {
        let cfg = replay_config();
        let (live, history) =
            crate::runner::ExperimentRunner::run_recording(&cfg, true).expect("recording run");
        let h = history.expect("history captured");
        let sys = lua_elo();
        let replay = ReplayEngine::replay(
            &h,
            sys.as_ref(),
            &cfg,
            &[crate::config::MetricEntry::Name(
                "rating_accuracy".to_string(),
            )],
        )
        .expect("replay");
        let live_mean = extract_mean(
            &live.metrics["rating_accuracy"],
            "recording run rating_accuracy",
        );
        let replay_mean =
            extract_mean(&replay.metrics["rating_accuracy"], "replay rating_accuracy");
        assert!(
            (live_mean - replay_mean).abs() / live_mean.abs() < 1e-6,
            "replay accuracy ({replay_mean}) must track the live recording ({live_mean})"
        );
    }
    fn extract_mean(result: &MetricResult, what: &str) -> f64 {
        match result {
            MetricResult::Summary { mean, .. } => *mean,
            other => panic!("{what}: expected summary, got {other:?}"),
        }
    }
}
/// Counterfactual mode for strategic behavior.
#[derive(Debug, Clone, PartialEq)]
pub enum StrategicMode {
    /// Players use non-adaptive policies (no adaptation).
    FixedBehavior,
    /// Players use adaptive policies (with adaptation).
    AdaptiveBehavior,
    /// Mixed: some players adaptive, some fixed.
    MixedPopulation { adaptive_fraction: f64 },
}
/// A strategic counterfactual comparison between two worlds.
#[derive(Debug, Clone)]
pub struct StrategicCounterfactual {
    pub baseline_mode: StrategicMode,
    pub alternative_mode: StrategicMode,
}
/// The result of comparing two counterfactual worlds.
#[derive(Debug, Clone)]
pub struct CounterfactualComparison {
    /// Direct effect of the matchmaking policy change.
    pub direct_effect: MatchObjectiveVector,
    /// Effect caused by behavioral adaptation.
    pub adaptation_effect: MatchObjectiveVector,
    /// Total equilibrium effect after the ecosystem responds.
    pub equilibrium_effect: MatchObjectiveVector,
}
/// Multi-objective vector for matchmaking comparison .
#[derive(Debug, Clone, Default)]
pub struct MatchObjectiveVector {
    pub match_quality: f64,
    pub queue_time: f64,
    pub player_utility: f64,
    pub population_stability: f64,
}
impl MatchObjectiveVector {
    /// Compute the difference between two objective vectors.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            match_quality: self.match_quality - other.match_quality,
            queue_time: self.queue_time - other.queue_time,
            player_utility: self.player_utility - other.player_utility,
            population_stability: self.population_stability - other.population_stability,
        }
    }
}
/// Compare two strategic counterfactual worlds.
pub fn compare_counterfactuals(
    baseline: &MatchObjectiveVector,
    alternative: &MatchObjectiveVector,
) -> CounterfactualComparison {
    let direct = alternative.diff(baseline);
    CounterfactualComparison {
        direct_effect: direct.clone(),
        adaptation_effect: MatchObjectiveVector::default(),
        equilibrium_effect: direct,
    }
}
