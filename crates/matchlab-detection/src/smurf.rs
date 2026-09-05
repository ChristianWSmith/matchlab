use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use std::collections::{HashMap, VecDeque};

use crate::detector::{DetectionResult, DetectionSystem};
use crate::intervention::{InterventionAction, InterventionPolicy, PlayerInterventionState};

pub struct SmurfDetector {
    player_states: HashMap<PlayerId, SmurfState>,
    intervention_states: HashMap<PlayerId, PlayerInterventionState>,
    policy: InterventionPolicy,
    pub sigma_threshold: f64,
    pub min_anomalous_games: u64,
}

struct SmurfState {
    recent_performance: VecDeque<f64>,
    expected_performance: VecDeque<f64>,
    consecutive_anomalous: u32,
}

impl SmurfDetector {
    pub fn new(policy: InterventionPolicy) -> Self {
        Self {
            player_states: HashMap::new(),
            intervention_states: HashMap::new(),
            policy,
            sigma_threshold: 3.0,
            min_anomalous_games: 5,
        }
    }

    pub fn intervention_state(&self, player_id: PlayerId) -> Option<&PlayerInterventionState> {
        self.intervention_states.get(&player_id)
    }

    pub fn intervention_state_mut(&mut self, player_id: PlayerId) -> &mut PlayerInterventionState {
        self.intervention_states.entry(player_id).or_default()
    }
}

impl DetectionSystem for SmurfDetector {
    fn observe(&mut self, match_result: &MatchResult, world: &World) {
        for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
            let Some(obs) = world.observations.get(pid) else {
                continue;
            };
            let Some(perf) = match_result
                .player_performances
                .iter()
                .find(|p| &p.player_id == pid)
            else {
                continue;
            };

            let expected = obs.rating / 100.0;
            let actual = perf.impact + perf.kills as f64 / 10.0;

            let state = self.player_states.entry(*pid).or_insert(SmurfState {
                recent_performance: VecDeque::new(),
                expected_performance: VecDeque::new(),
                consecutive_anomalous: 0,
            });
            state.recent_performance.push_back(actual);
            state.expected_performance.push_back(expected);
            if state.recent_performance.len() > 20 {
                state.recent_performance.pop_front();
                state.expected_performance.pop_front();
            }

            let dev = (actual - expected).abs();
            let spread = state
                .recent_performance
                .iter()
                .map(|p| (p - expected).abs())
                .fold(0.0f64, f64::max);
            let sigmas = if spread > 0.0 { dev / spread } else { 0.0 };

            if sigmas >= self.sigma_threshold {
                state.consecutive_anomalous += 1;
            } else {
                state.consecutive_anomalous = 0;
            }
        }
    }

    fn evaluate(&self, player_id: PlayerId, _world: &World) -> DetectionResult {
        let state = match self.player_states.get(&player_id) {
            Some(s) => s,
            None => {
                return DetectionResult {
                    player_id,
                    probability_of_anomaly: 0.0,
                    confidence: 0.0,
                    evidence: Vec::new(),
                };
            }
        };

        let flagged = state.consecutive_anomalous as u64 >= self.min_anomalous_games;
        let probability_of_anomaly = if flagged {
            let extra = state.consecutive_anomalous as f64 - self.min_anomalous_games as f64;
            (0.7 + 0.25 * extra.min(1.2)).min(0.99)
        } else {
            state.consecutive_anomalous as f64 / self.min_anomalous_games as f64 * 0.3
        };

        DetectionResult {
            player_id,
            probability_of_anomaly,
            confidence: (state.consecutive_anomalous as f64 / self.min_anomalous_games as f64)
                .min(1.0),
            evidence: vec![
                format!("consecutive_anomalous={}", state.consecutive_anomalous),
                format!("min_required={}", self.min_anomalous_games),
            ],
        }
    }

    fn recommend_action(&self, result: &DetectionResult) -> InterventionAction {
        let state = self
            .intervention_states
            .get(&result.player_id)
            .cloned()
            .unwrap_or_default();
        self.policy.apply(result, &state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, PlayerPerformance, Team};
    use matchlab_core::player::{DetectionFlag, PlayerObservation, SkillVector, VisibleRank};
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
            game_mode: "ranked".to_string(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn make_match_result(
        match_id: u64,
        team_a: Vec<PlayerId>,
        team_b: Vec<PlayerId>,
        performances: Vec<PlayerPerformance>,
    ) -> MatchResult {
        MatchResult {
            match_id: MatchId(match_id),
            winner: Team::A,
            team_a,
            team_b,
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: performances,
            duration: matchlab_core::time::SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn normal_player_has_low_probability() {
        let policy = InterventionPolicy::default_ladder();
        let mut detector = SmurfDetector::new(policy);
        let mut world = World::new(matchlab_core::rng::SimRng::from_seed(42));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));

        for i in 0..5 {
            let mr = make_match_result(
                i,
                vec![PlayerId(1)],
                vec![PlayerId(2)],
                vec![PlayerPerformance {
                    player_id: PlayerId(1),
                    kills: 5,
                    deaths: 5,
                    assists: 3,
                    objective_score: 50.0,
                    impact: 0.0,
                    variance: 0.5,
                }],
            );
            detector.observe(&mr, &world);
        }

        let result = detector.evaluate(PlayerId(1), &world);
        assert!(result.probability_of_anomaly < 0.3);
    }

    #[test]
    fn consecutive_anomalous_increases_probability() {
        let policy = InterventionPolicy::default_ladder();
        let mut detector = SmurfDetector::new(policy);
        detector.min_anomalous_games = 3;
        detector.sigma_threshold = 1.0;
        let mut world = World::new(matchlab_core::rng::SimRng::from_seed(42));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));

        for i in 0..5 {
            let mr = make_match_result(
                i,
                vec![PlayerId(1)],
                vec![PlayerId(2)],
                vec![PlayerPerformance {
                    player_id: PlayerId(1),
                    kills: 50,
                    deaths: 0,
                    assists: 0,
                    objective_score: 100.0,
                    impact: 10.0,
                    variance: 0.0,
                }],
            );
            detector.observe(&mr, &world);
        }

        let result = detector.evaluate(PlayerId(1), &world);
        assert!(result.probability_of_anomaly > 0.7);
    }

    #[test]
    fn evaluate_returns_zero_for_unknown_player() {
        let policy = InterventionPolicy::default_ladder();
        let detector = SmurfDetector::new(policy);
        let world = World::new(matchlab_core::rng::SimRng::from_seed(42));

        let result = detector.evaluate(PlayerId(999), &world);
        assert_eq!(result.probability_of_anomaly, 0.0);
        assert_eq!(result.confidence, 0.0);
        assert!(result.evidence.is_empty());
    }

    #[test]
    fn recommend_action_below_threshold_returns_none() {
        let policy = InterventionPolicy::default_ladder();
        let detector = SmurfDetector::new(policy);

        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.1,
            confidence: 0.1,
            evidence: Vec::new(),
        };
        assert!(matches!(
            detector.recommend_action(&result),
            InterventionAction::None
        ));
    }

    #[test]
    fn recommend_action_above_threshold_returns_action() {
        let policy = InterventionPolicy::default_ladder();
        let mut detector = SmurfDetector::new(policy);

        detector.intervention_state_mut(PlayerId(1)).games_played = 10;

        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.95,
            confidence: 0.95,
            evidence: Vec::new(),
        };
        assert!(matches!(
            detector.recommend_action(&result),
            InterventionAction::Probation { .. }
        ));
    }
}
