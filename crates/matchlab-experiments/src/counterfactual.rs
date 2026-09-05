//! Counterfactual evaluation (spec §13.6): replay the identical game history
//! through multiple rating systems to isolate rating-system effects from
//! matchmaking and game-model effects.

use matchlab_core::match_::MatchResult;
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_rating::filter::filter_match_result;
use matchlab_rating::system::RatingSystem;
use std::collections::HashMap;

/// A recorded game history: the sequence of matches and the observation
/// snapshot at each match (what each rating system would have seen).
pub struct GameHistory {
    pub matches: Vec<MatchResult>,
    pub player_snapshots: Vec<HashMap<PlayerId, PlayerObservation>>,
}

impl GameHistory {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            player_snapshots: Vec::new(),
        }
    }

    /// Record one match along with the observation snapshot in effect when it
    /// resolved. Snapshot stores only the participants.
    pub fn record(&mut self, match_result: &MatchResult, world: &matchlab_core::world::World) {
        let mut snapshot = HashMap::new();
        for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
            if let Some(o) = world.observations.get(pid) {
                snapshot.insert(*pid, o.clone());
            }
        }
        self.matches.push(match_result.clone());
        self.player_snapshots.push(snapshot);
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}

impl Default for GameHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Run multiple rating systems through identical history. Each system's full
/// `RatingState` (rating, RD, volatility, games played) is preserved so
/// Bayesian systems like Glicko-2 and TrueSkill update correctly across
/// matches. Each system only sees the data in its information budget
/// (WinLoss-only results are budget-sanitized before `update`).
pub fn counterfactual_eval(
    history: &GameHistory,
    systems: &[(&str, Box<dyn RatingSystem>)],
) -> HashMap<String, Vec<(PlayerId, matchlab_rating::system::RatingState)>> {
    let mut results = HashMap::new();

    for (name, system) in systems {
        let mut states: HashMap<PlayerId, matchlab_rating::system::RatingState> = HashMap::new();

        for (i, match_result) in history.matches.iter().enumerate() {
            let observations = &history.player_snapshots[i];

            for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
                if !states.contains_key(pid) {
                    states.insert(*pid, system.initialize(*pid));
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

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{DetectionFlag, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use matchlab_core::world::World;
    use matchlab_rating::elo::{EloConfig, EloRatingSystem};
    use matchlab_rating::flat::{FlatPointsConfig, FlatPointsRatingSystem};
    use std::collections::VecDeque;

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
        let sys_a = Box::new(EloRatingSystem::new(EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        })) as Box<dyn RatingSystem>;
        let sys_b = Box::new(EloRatingSystem::new(EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        })) as Box<dyn RatingSystem>;

        let a = counterfactual_eval(&h, &[("elo", sys_a)]);
        let b = counterfactual_eval(&h, &[("elo", sys_b)]);
        // Same system + same history → identical final states.
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
        let elo = Box::new(EloRatingSystem::new(EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        })) as Box<dyn RatingSystem>;
        let flat = Box::new(FlatPointsRatingSystem::new(FlatPointsConfig {
            win_points: 10.0,
            loss_points: 10.0,
            initial_rating: 1000.0,
        })) as Box<dyn RatingSystem>;

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
        let elo = Box::new(EloRatingSystem::new(EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        })) as Box<dyn RatingSystem>;
        let res = counterfactual_eval(&h, &[("elo", elo)]);
        let states: HashMap<PlayerId, _> = res["elo"].iter().cloned().collect();
        // Player 1 won 2 of 3 → higher rating than player 2.
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
    }
}
