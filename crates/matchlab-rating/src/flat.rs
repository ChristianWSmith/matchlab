use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use std::collections::HashMap;

use crate::system::{ObservationType, RatingState, RatingSystem};

pub struct FlatPointsConfig {
    pub win_points: f64,
    pub loss_points: f64,
    pub initial_rating: f64,
}

pub struct FlatPointsRatingSystem {
    pub config: FlatPointsConfig,
}

impl FlatPointsRatingSystem {
    pub fn new(config: FlatPointsConfig) -> Self {
        Self { config }
    }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let initial_rating = value
            .get("initial_rating")
            .and_then(serde_yaml::Value::as_f64)?;
        Some(Self::new(FlatPointsConfig {
            win_points: value
                .get("win_points")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(10.0),
            loss_points: value
                .get("loss_points")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(10.0),
            initial_rating,
        }))
    }
}

impl RatingSystem for FlatPointsRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_rating,
            rating_deviation: 350.0,
            volatility: 0.0,
            games_played: 0,
        }
    }

    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let avg_a = team_a.iter().map(|p| p.rating).sum::<f64>() / team_a.len() as f64;
        let avg_b = team_b.iter().map(|p| p.rating).sum::<f64>() / team_b.len() as f64;
        1.0 / (1.0 + 10f64.powf((avg_b - avg_a) / 400.0))
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mut updates = HashMap::new();

        for &pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
            if let Some(obs) = observations.get(&pid) {
                let is_team_a = match_result.team_a.contains(&pid);
                let won = (is_team_a && match_result.winner == Team::A)
                    || (!is_team_a && match_result.winner == Team::B);

                let delta = if won {
                    self.config.win_points
                } else {
                    -self.config.loss_points
                };
                updates.insert(
                    pid,
                    RatingState {
                        rating: obs.rating + delta,
                        rating_deviation: obs.rating_deviation,
                        volatility: obs.volatility,
                        games_played: obs.games_played + 1,
                    },
                );
            }
        }

        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::MatchId;
    use matchlab_core::player::{DetectionFlag, SkillVector, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".to_string(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.0,
            games_played: 50,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn flat() -> FlatPointsRatingSystem {
        FlatPointsRatingSystem::new(FlatPointsConfig {
            win_points: 10.0,
            loss_points: 10.0,
            initial_rating: 1000.0,
        })
    }

    fn make_match_result(
        team_a: Vec<PlayerId>,
        team_b: Vec<PlayerId>,
        winner: Team,
    ) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner,
            team_a,
            team_b,
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: Vec::new(),
            duration: matchlab_core::time::SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn winner_gains_win_points_loser_loses_loss_points() {
        let sys = flat();
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1000.0));
        obs_map.insert(PlayerId(2), obs(2, 1000.0));
        let updates = sys.update(&mr, &obs_map);

        assert_eq!(updates[&PlayerId(1)].rating, 1010.0);
        assert_eq!(updates[&PlayerId(2)].rating, 990.0);
    }

    #[test]
    fn asymmetric_points_applied_per_side() {
        let sys = FlatPointsRatingSystem::new(FlatPointsConfig {
            win_points: 25.0,
            loss_points: 5.0,
            initial_rating: 1000.0,
        });
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::B);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1000.0));
        obs_map.insert(PlayerId(2), obs(2, 1000.0));
        let updates = sys.update(&mr, &obs_map);

        assert_eq!(updates[&PlayerId(1)].rating, 995.0);
        assert_eq!(updates[&PlayerId(2)].rating, 1025.0);
    }

    #[test]
    fn initialize_returns_configured_initial_rating() {
        let sys = flat();
        let state = sys.initialize(PlayerId(0));
        assert_eq!(state.rating, 1000.0);
        assert_eq!(state.games_played, 0);
        assert_eq!(state.volatility, 0.0);
    }

    #[test]
    fn update_increments_games_played() {
        let sys = flat();
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1000.0));
        obs_map.insert(PlayerId(2), obs(2, 1000.0));
        let updates = sys.update(&mr, &obs_map);
        assert_eq!(updates[&PlayerId(1)].games_played, 51);
        assert_eq!(updates[&PlayerId(2)].games_played, 51);
    }

    #[test]
    fn from_yaml_round_trips_config_with_defaults() {
        let yaml = serde_yaml::from_str("initial_rating: 800.0\n").unwrap();
        let sys = FlatPointsRatingSystem::from_yaml(&yaml).unwrap();
        assert_eq!(sys.config.initial_rating, 800.0);
        assert_eq!(sys.config.win_points, 10.0);
        assert_eq!(sys.config.loss_points, 10.0);

        let yaml2 =
            serde_yaml::from_str("win_points: 2.5\nloss_points: 1.0\ninitial_rating: 600.0\n")
                .unwrap();
        let sys2 = FlatPointsRatingSystem::from_yaml(&yaml2).unwrap();
        assert_eq!(sys2.config.win_points, 2.5);
        assert_eq!(sys2.config.loss_points, 1.0);
        assert_eq!(sys2.config.initial_rating, 600.0);
    }

    #[test]
    fn information_budget_only_winloss() {
        let sys = flat();
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn equal_ratings_predict_half() {
        let sys = flat();
        let p = sys.predict(&[obs(0, 1000.0)], &[obs(1, 1000.0)]);
        assert!((p - 0.5).abs() < 0.001, "p = {p}");
    }
}
