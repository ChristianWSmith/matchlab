//! Player objectives and utility: general mechanism for agents
//! to express what they are trying to optimize, separable from behavior policies.
use crate::agent::{AgentObservations, AgentOutcome, PlayerObjective};
/// Maximize win rate toward a target.
pub struct WinRateObjective {
    pub target_win_rate: f64,
}
impl PlayerObjective for WinRateObjective {
    fn evaluate(&self, obs: &AgentObservations, _history: &[AgentOutcome]) -> f64 {
        -(obs.own_win_rate - self.target_win_rate).powi(2)
    }
    fn description(&self) -> String {
        format!("maximize win rate toward {}", self.target_win_rate)
    }
}
/// Maximize expected rating gain.
pub struct RatingGainObjective;
impl PlayerObjective for RatingGainObjective {
    fn evaluate(&self, _obs: &AgentObservations, history: &[AgentOutcome]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }
        history.iter().map(|o| o.rating_change).sum::<f64>() / history.len() as f64
    }
    fn description(&self) -> String {
        "maximize rating gain".to_string()
    }
}
/// Minimize rating uncertainty (rating deviation).
pub struct RatingVarianceObjective;
impl PlayerObjective for RatingVarianceObjective {
    fn evaluate(&self, obs: &AgentObservations, _history: &[AgentOutcome]) -> f64 {
        -obs.own_rating_deviation
    }
    fn description(&self) -> String {
        "minimize rating uncertainty".to_string()
    }
}
/// Minimize queue wait time.
pub struct QueueTimeObjective;
impl PlayerObjective for QueueTimeObjective {
    fn evaluate(&self, obs: &AgentObservations, _history: &[AgentOutcome]) -> f64 {
        -obs.queue_wait_time.unwrap_or(0.0)
    }
    fn description(&self) -> String {
        "minimize queue time".to_string()
    }
}
/// Maximize match quality.
pub struct MatchQualityObjective;
impl PlayerObjective for MatchQualityObjective {
    fn evaluate(&self, _obs: &AgentObservations, history: &[AgentOutcome]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }
        let wins = history
            .iter()
            .filter(|o| o.match_won.unwrap_or(false))
            .count();
        wins as f64 / history.len() as f64
    }
    fn description(&self) -> String {
        "maximize match quality".to_string()
    }
}
/// Maximize time spent playing.
pub struct TimeSpentObjective;
impl PlayerObjective for TimeSpentObjective {
    fn evaluate(&self, obs: &AgentObservations, _history: &[AgentOutcome]) -> f64 {
        obs.own_games_played as f64
    }
    fn description(&self) -> String {
        "maximize time spent playing".to_string()
    }
}
/// Minimize latency.
pub struct LatencyObjective;
impl PlayerObjective for LatencyObjective {
    fn evaluate(&self, obs: &AgentObservations, _history: &[AgentOutcome]) -> f64 {
        -obs.estimated_latency.unwrap_or(50.0)
    }
    fn description(&self) -> String {
        "minimize latency".to_string()
    }
}
/// Weighted combination of multiple objectives.
pub struct CompositeObjective {
    pub objectives: Vec<(Box<dyn PlayerObjective>, f64)>,
}
impl PlayerObjective for CompositeObjective {
    fn evaluate(&self, obs: &AgentObservations, history: &[AgentOutcome]) -> f64 {
        self.objectives
            .iter()
            .map(|(obj, weight)| weight * obj.evaluate(obs, history))
            .sum()
    }
    fn description(&self) -> String {
        let parts: Vec<String> = self
            .objectives
            .iter()
            .map(|(obj, w)| format!("{}×{}", w, obj.description()))
            .collect();
        format!("composite[{}]", parts.join(", "))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentAction, AgentObservations};
    #[test]
    fn win_rate_objective_evaluates() {
        let obj = WinRateObjective {
            target_win_rate: 0.7,
        };
        let obs = AgentObservations {
            own_win_rate: 0.7,
            ..AgentObservations::default()
        };
        assert!((obj.evaluate(&obs, &[]) - 0.0).abs() < 1e-9);
        let obs2 = AgentObservations {
            own_win_rate: 0.5,
            ..AgentObservations::default()
        };
        assert!((obj.evaluate(&obs2, &[]) - (-0.04)).abs() < 1e-9);
    }
    #[test]
    fn rating_gain_objective_averages_history() {
        let obj = RatingGainObjective;
        let obs = AgentObservations::default();
        let history = vec![
            AgentOutcome {
                action: AgentAction::NoOp,
                rating_change: 10.0,
                queue_time: 0.0,
                match_won: Some(true),
                timestamp: 0,
            },
            AgentOutcome {
                action: AgentAction::NoOp,
                rating_change: -5.0,
                queue_time: 0.0,
                match_won: Some(false),
                timestamp: 1,
            },
        ];
        let score = obj.evaluate(&obs, &history);
        assert!((score - 2.5).abs() < 1e-9);
    }
    #[test]
    fn composite_objective_weighted_sum() {
        let a = WinRateObjective {
            target_win_rate: 0.5,
        };
        let b = QueueTimeObjective;
        let composite = CompositeObjective {
            objectives: vec![(Box::new(a), 1.0), (Box::new(b), 2.0)],
        };
        let obs = AgentObservations {
            own_win_rate: 0.5,
            queue_wait_time: Some(10.0),
            ..AgentObservations::default()
        };
        let score = composite.evaluate(&obs, &[]);
        assert!((score - (-20.0)).abs() < 1e-9);
    }
    #[test]
    fn objectives_are_deterministic() {
        let obj = WinRateObjective {
            target_win_rate: 0.6,
        };
        let obs = AgentObservations::default();
        let s1 = obj.evaluate(&obs, &[]);
        let s2 = obj.evaluate(&obs, &[]);
        assert_eq!(s1, s2);
    }
}
