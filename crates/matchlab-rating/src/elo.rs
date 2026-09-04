use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use std::collections::HashMap;

use crate::hooks::LuaHooks;
use crate::system::{ObservationType, RatingState, RatingSystem};

pub struct EloConfig {
    pub k_factor: f64,
    pub initial_rating: f64,
    pub beta: f64,
}

pub struct EloRatingSystem {
    pub config: EloConfig,
    hooks: Option<LuaHooks>,
}

impl EloRatingSystem {
    pub fn new(config: EloConfig) -> Self {
        Self { config, hooks: None }
    }

    pub fn with_hooks(config: EloConfig, hooks: LuaHooks) -> Self {
        Self {
            config,
            hooks: Some(hooks),
        }
    }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let k_factor = value.get("k_factor").and_then(serde_yaml::Value::as_f64)?;
        let initial_rating = value
            .get("initial_rating")
            .and_then(serde_yaml::Value::as_f64)?;
        let beta = value
            .get("beta")
            .and_then(serde_yaml::Value::as_f64)
            .unwrap_or(400.0);
        Some(Self::new(EloConfig {
            k_factor,
            initial_rating,
            beta,
        }))
    }

    fn divisor(&self) -> f64 {
        self.config.beta * std::f64::consts::LN_10
    }

    fn expected_score(&self, rating_a: f64, rating_b: f64) -> f64 {
        1.0 / (1.0 + 10f64.powf((rating_b - rating_a) / self.divisor()))
    }

    fn team_average(ids: &[PlayerId], obs: &HashMap<PlayerId, PlayerObservation>) -> f64 {
        let sum: f64 = ids
            .iter()
            .filter_map(|id| obs.get(id))
            .map(|o| o.rating)
            .sum();
        sum / ids.len() as f64
    }
}

impl RatingSystem for EloRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_rating,
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
        }
    }

    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let avg_a = team_a.iter().map(|p| p.rating).sum::<f64>() / team_a.len() as f64;
        let avg_b = team_b.iter().map(|p| p.rating).sum::<f64>() / team_b.len() as f64;
        self.expected_score(avg_a, avg_b)
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mut updates = HashMap::new();
        let avg_a = Self::team_average(&match_result.team_a, observations);
        let avg_b = Self::team_average(&match_result.team_b, observations);
        let expected_a = self.expected_score(avg_a, avg_b);
        let expected_b = 1.0 - expected_a;
        let actual_a = if match_result.winner == Team::A {
            1.0
        } else {
            0.0
        };
        let actual_b = 1.0 - actual_a;

        for &pid in &match_result.team_a {
            if let Some(obs) = observations.get(&pid) {
                let k = self
                    .hooks
                    .as_ref()
                    .and_then(|h| {
                        h.call_k_factor(
                            pid.0,
                            obs.rating,
                            obs.games_played,
                            obs.win_rate,
                        )
                    })
                    .unwrap_or(self.config.k_factor);
                let new_rating = obs.rating + k * (actual_a - expected_a);
                let new_rating = self
                    .hooks
                    .as_ref()
                    .and_then(|h| h.call_rating_bounds())
                    .map(|(floor, ceiling)| new_rating.clamp(floor, ceiling))
                    .unwrap_or(new_rating);
                updates.insert(
                    pid,
                    RatingState {
                        rating: new_rating,
                        rating_deviation: obs.rating_deviation,
                        volatility: obs.volatility,
                        games_played: obs.games_played + 1,
                    },
                );
            }
        }
        for &pid in &match_result.team_b {
            if let Some(obs) = observations.get(&pid) {
                let k = self
                    .hooks
                    .as_ref()
                    .and_then(|h| {
                        h.call_k_factor(
                            pid.0,
                            obs.rating,
                            obs.games_played,
                            obs.win_rate,
                        )
                    })
                    .unwrap_or(self.config.k_factor);
                let new_rating = obs.rating + k * (actual_b - expected_b);
                let new_rating = self
                    .hooks
                    .as_ref()
                    .and_then(|h| h.call_rating_bounds())
                    .map(|(floor, ceiling)| new_rating.clamp(floor, ceiling))
                    .unwrap_or(new_rating);
                updates.insert(
                    pid,
                    RatingState {
                        rating: new_rating,
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(prefix: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("{}_{}_{}.lua", prefix, std::process::id(), n))
    }

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
            volatility: 0.06,
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

    fn elo(beta: f64) -> EloRatingSystem {
        EloRatingSystem::new(EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta,
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
    fn equal_ratings_produce_50_percent() {
        let sys = elo(400.0);
        let p = sys.predict(&[obs(0, 1000.0)], &[obs(1, 1000.0)]);
        assert!((p - 0.5).abs() < 0.001, "p = {p}");
    }

    #[test]
    fn higher_rated_player_has_higher_win_probability() {
        let sys = elo(400.0);
        let p = sys.predict(&[obs(0, 1500.0)], &[obs(1, 1000.0)]);
        assert!(p > 0.5, "p = {p}");
        assert!(p < 1.0);
    }

    #[test]
    fn known_ratings_upset_shifts_more_than_favorite_winning() {
        let sys = elo(400.0);
        let obs_a_strong = obs(1, 1600.0);
        let obs_b_weak = obs(2, 1200.0);
        let obs_c_strong = obs(3, 1600.0);
        let obs_d_weak = obs(4, 1200.0);

        // Scenario 1: weak team upsets (B wins)
        let mr_upset = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::B);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs_a_strong.clone());
        obs_map.insert(PlayerId(2), obs_b_weak.clone());
        let updates_upset = sys.update(&mr_upset, &obs_map);

        // Scenario 2: favorite wins (A wins)
        let mr_favored = make_match_result(vec![PlayerId(3)], vec![PlayerId(4)], Team::A);
        let mut obs_map2 = HashMap::new();
        obs_map2.insert(PlayerId(3), obs_c_strong.clone());
        obs_map2.insert(PlayerId(4), obs_d_weak.clone());
        let updates_favored = sys.update(&mr_favored, &obs_map2);

        let weak_upset_gain = updates_upset[&PlayerId(2)].rating - obs_b_weak.rating;
        let strong_favored_gain = updates_favored[&PlayerId(3)].rating - obs_c_strong.rating;

        // Upset produces a larger rating gain for the underdog.
        assert!(
            weak_upset_gain > strong_favored_gain,
            "upset gain {weak_upset_gain} should be > favored gain {strong_favored_gain}"
        );
    }

    #[test]
    fn initialize_returns_configured_initial_rating() {
        let sys = elo(400.0);
        let state = sys.initialize(PlayerId(0));
        assert_eq!(state.rating, 1000.0);
        assert_eq!(state.games_played, 0);
    }

    #[test]
    fn update_increments_games_played_for_all_participants() {
        let sys = elo(400.0);
        let mr = make_match_result(
            vec![PlayerId(1), PlayerId(2)],
            vec![PlayerId(3), PlayerId(4)],
            Team::A,
        );
        let mut obs_map = HashMap::new();
        for i in 1..=4 {
            obs_map.insert(PlayerId(i), obs(i, 1000.0));
        }
        let updates = sys.update(&mr, &obs_map);
        for pid in [PlayerId(1), PlayerId(2), PlayerId(3), PlayerId(4)] {
            assert_eq!(updates[&pid].games_played, 51);
        }
    }

    #[test]
    fn from_yaml_round_trips_config() {
        let yaml =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1500.0\nbeta: 400.0\n").unwrap();
        let sys = EloRatingSystem::from_yaml(&yaml).unwrap();
        assert_eq!(sys.config.k_factor, 32.0);
        assert_eq!(sys.config.initial_rating, 1500.0);
        assert_eq!(sys.config.beta, 400.0);
    }

    #[test]
    fn from_yaml_requires_k_factor_and_initial_rating() {
        let yaml = serde_yaml::from_str("k_factor: 32.0\n").unwrap();
        assert!(EloRatingSystem::from_yaml(&yaml).is_none());
        let yaml2 = serde_yaml::from_str("initial_rating: 1000.0\n").unwrap();
        assert!(EloRatingSystem::from_yaml(&yaml2).is_none());
    }

    #[test]
    fn information_budget_only_winloss() {
        let sys = elo(400.0);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn beta_consistency_with_logistic_model() {
        let sys = elo(400.0);
        let p = sys.predict(&[obs(0, 1000.0)], &[obs(1, 1400.0)]);
        // Same as logistic model: 1/(1+exp(-(1000-1400)/400)) = 1/(1+exp(1))
        let logistic_p = 1.0 / (1.0 + (-(-400.0_f64) / 400.0).exp());
        // 1/(1+exp(1)) ≈ 0.2689
        assert!(
            (p - logistic_p).abs() < 0.001,
            "elo p={p} vs logistic p={logistic_p}"
        );
    }

    #[test]
    fn lua_hook_overrides_k_factor() {
        let script = r#"
function on_k_factor(player_id, rating, games_played, recent_win_rate)
    return 64.0
end
"#;
        let path = temp_path("test_elo_hooks");
        std::fs::write(&path, script).unwrap();

        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let config = EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        };
        let sys = EloRatingSystem::with_hooks(config, hooks);

        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1000.0));
        obs_map.insert(PlayerId(2), obs(2, 1000.0));
        let updates = sys.update(&mr, &obs_map);

        // With k=64 and equal ratings, expected=0.5, actual=1.0
        // delta = 64 * (1.0 - 0.5) = 32.0
        let expected_delta = 64.0 * 0.5;
        assert!(
            (updates[&PlayerId(1)].rating - 1000.0 - expected_delta).abs() < 0.001,
            "rating change should use Lua k_factor"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn lua_hook_undefined_falls_back_to_config() {
        let script = "-- no hooks defined";
        let path = temp_path("test_elo_fallback");
        std::fs::write(&path, script).unwrap();

        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let config = EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        };
        let sys = EloRatingSystem::with_hooks(config, hooks);

        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1000.0));
        obs_map.insert(PlayerId(2), obs(2, 1000.0));
        let updates = sys.update(&mr, &obs_map);

        // Falls back to config k_factor=32.0
        let expected_delta = 32.0 * 0.5;
        assert!(
            (updates[&PlayerId(1)].rating - 1000.0 - expected_delta).abs() < 0.001,
            "rating change should use config k_factor when hook undefined"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn lua_hook_rating_bounds_clamps_rating() {
        let script = r#"
function on_k_factor() return 1000.0 end
function on_rating_bounds()
    return { floor = 900.0, ceiling = 1100.0 }
end
"#;
        let path = temp_path("test_elo_bounds");
        std::fs::write(&path, script).unwrap();

        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let config = EloConfig {
            k_factor: 32.0,
            initial_rating: 1000.0,
            beta: 400.0,
        };
        let sys = EloRatingSystem::with_hooks(config, hooks);

        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1000.0));
        obs_map.insert(PlayerId(2), obs(2, 1000.0));
        let updates = sys.update(&mr, &obs_map);

        // k=1000, delta = 1000 * 0.5 = 500, but clamped to ceiling 1100
        assert_eq!(updates[&PlayerId(1)].rating, 1100.0);

        let _ = std::fs::remove_file(&path);
    }
}
