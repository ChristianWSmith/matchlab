use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterventionAction {
    None,
    AccelerateRating { multiplier: f64 },
    IncreaseKFactor { new_k: f64 },
    FlagForReview,
    RestrictQueue { duration_ticks: u64 },
    TempBan { duration_ticks: u64 },
    Probation { duration_ticks: u64 },
    Ban,
}

#[derive(Debug, Clone)]
pub struct InterventionPolicy {
    pub thresholds: Vec<(f64, InterventionAction)>,
    pub escalation_window_ticks: u64,
    pub escalation_factor: f64,
    pub min_games_before_action: u64,
}

impl InterventionPolicy {
    pub fn default_ladder() -> Self {
        Self {
            thresholds: vec![
                (0.3, InterventionAction::None),
                (0.5, InterventionAction::AccelerateRating { multiplier: 1.5 }),
                (0.7, InterventionAction::FlagForReview),
                (0.8, InterventionAction::RestrictQueue { duration_ticks: 100 }),
                (0.9, InterventionAction::TempBan { duration_ticks: 500 }),
                (0.95, InterventionAction::Probation { duration_ticks: 1000 }),
                (0.99, InterventionAction::Ban),
            ],
            escalation_window_ticks: 500,
            escalation_factor: 0.9,
            min_games_before_action: 5,
        }
    }

    pub fn apply(
        &self,
        result: &crate::detector::DetectionResult,
        state: &PlayerInterventionState,
    ) -> InterventionAction {
        if state.games_played < self.min_games_before_action {
            return InterventionAction::None;
        }

        let effective_thresholds: Vec<(f64, &InterventionAction)> = self
            .thresholds
            .iter()
            .map(|(thresh, action)| {
                let escalated =
                    thresh * self.escalation_factor.powi(state.prior_interventions as i32);
                (escalated.min(*thresh), action)
            })
            .collect();

        let prob = result.probability_of_anomaly;
        let mut chosen = InterventionAction::None;
        for (thresh, action) in &effective_thresholds {
            if prob >= *thresh {
                chosen = (*action).clone();
            }
        }
        chosen
    }
}

#[derive(Debug, Clone)]
pub struct PlayerInterventionState {
    pub games_played: u64,
    pub prior_interventions: u32,
    pub last_intervention_tick: u64,
    pub escalation_history: Vec<(u64, InterventionAction)>,
}

impl Default for PlayerInterventionState {
    fn default() -> Self {
        Self {
            games_played: 0,
            prior_interventions: 0,
            last_intervention_tick: 0,
            escalation_history: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector::DetectionResult;
    use matchlab_core::player::PlayerId;

    #[test]
    fn below_min_games_returns_none() {
        let policy = InterventionPolicy::default_ladder();
        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.99,
            confidence: 1.0,
            evidence: Vec::new(),
        };
        let state = PlayerInterventionState {
            games_played: 2,
            ..Default::default()
        };
        assert!(matches!(policy.apply(&result, &state), InterventionAction::None));
    }

    #[test]
    fn low_probability_returns_none() {
        let policy = InterventionPolicy::default_ladder();
        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.2,
            confidence: 0.2,
            evidence: Vec::new(),
        };
        let state = PlayerInterventionState {
            games_played: 10,
            ..Default::default()
        };
        assert!(matches!(policy.apply(&result, &state), InterventionAction::None));
    }

    #[test]
    fn moderate_probability_triggers_accelerate_rating() {
        let policy = InterventionPolicy::default_ladder();
        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.6,
            confidence: 0.6,
            evidence: Vec::new(),
        };
        let state = PlayerInterventionState {
            games_played: 10,
            ..Default::default()
        };
        assert!(matches!(
            policy.apply(&result, &state),
            InterventionAction::AccelerateRating { .. }
        ));
    }

    #[test]
    fn high_probability_triggers_ban() {
        let policy = InterventionPolicy::default_ladder();
        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.995,
            confidence: 0.99,
            evidence: Vec::new(),
        };
        let state = PlayerInterventionState {
            games_played: 10,
            ..Default::default()
        };
        assert!(matches!(policy.apply(&result, &state), InterventionAction::Ban));
    }

    #[test]
    fn escalation_lowens_thresholds() {
        let policy = InterventionPolicy::default_ladder();
        let result = DetectionResult {
            player_id: PlayerId(1),
            probability_of_anomaly: 0.75,
            confidence: 0.75,
            evidence: Vec::new(),
        };
        let state = PlayerInterventionState {
            games_played: 10,
            prior_interventions: 5,
            ..Default::default()
        };
        // With escalation (0.9^5 ≈ 0.59), the 0.99 threshold drops to ~0.58,
        // so 0.75 now triggers Ban instead of just AccelerateRating.
        let action = policy.apply(&result, &state);
        assert!(matches!(action, InterventionAction::Ban));
    }
}
