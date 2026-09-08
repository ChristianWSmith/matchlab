//! Adaptive player policies: policies that change behavior
//! based on experience, conditioning future actions on recent outcomes.
use crate::agent::{AgentAction, AgentObservations, AgentOutcome, PlayerObjective};
use matchlab_core::rng::SimRng;
/// An adaptive policy that conditions actions on observations and history.
pub trait AdaptivePolicy: Send + Sync {
    fn decide(
        &self,
        _observations: &AgentObservations,
        _history: &[AgentOutcome],
        _objective: &dyn PlayerObjective,
        _rng: &mut SimRng,
    ) -> AgentAction {
        AgentAction::ContinuePlaying
    }
}
/// Queue only when expected queue time is below a threshold.
pub struct ThresholdQueuePolicy {
    pub max_queue_time: f64,
}
impl AdaptivePolicy for ThresholdQueuePolicy {
    fn decide(
        &self,
        observations: &AgentObservations,
        _history: &[AgentOutcome],
        _objective: &dyn PlayerObjective,
        _rng: &mut SimRng,
    ) -> AgentAction {
        match observations.queue_wait_time {
            Some(wait) if wait <= self.max_queue_time => AgentAction::Queue,
            None => AgentAction::Queue,
            _ => AgentAction::NoOp,
        }
    }
}
/// Leave after repeated losses.
pub struct LossAversionPolicy {
    pub max_consecutive_losses: u64,
}
impl AdaptivePolicy for LossAversionPolicy {
    fn decide(
        &self,
        _observations: &AgentObservations,
        history: &[AgentOutcome],
        _objective: &dyn PlayerObjective,
        _rng: &mut SimRng,
    ) -> AgentAction {
        let mut consecutive_losses = 0u64;
        for outcome in history.iter().rev() {
            if outcome.match_won == Some(false) {
                consecutive_losses += 1;
            } else {
                break;
            }
        }
        if consecutive_losses >= self.max_consecutive_losses {
            AgentAction::StopPlaying
        } else {
            AgentAction::ContinuePlaying
        }
    }
}
/// Switch regions after poor latency.
pub struct RegionSwitchPolicy {
    pub latency_threshold: f64,
    pub target_region: matchlab_core::player::Region,
}
impl AdaptivePolicy for RegionSwitchPolicy {
    fn decide(
        &self,
        observations: &AgentObservations,
        _history: &[AgentOutcome],
        _objective: &dyn PlayerObjective,
        _rng: &mut SimRng,
    ) -> AgentAction {
        match observations.estimated_latency {
            Some(latency) if latency > self.latency_threshold => {
                AgentAction::SelectRegion(self.target_region)
            }
            _ => AgentAction::ContinuePlaying,
        }
    }
}
/// Form a party after observing strong synergy.
pub struct PartyFormationPolicy {
    pub synergy_threshold: f64,
}
impl AdaptivePolicy for PartyFormationPolicy {
    fn decide(
        &self,
        observations: &AgentObservations,
        _history: &[AgentOutcome],
        _objective: &dyn PlayerObjective,
        _rng: &mut SimRng,
    ) -> AgentAction {
        if observations.party_size > 1 {
            return AgentAction::ContinuePlaying;
        }
        if !observations.recent_match_results.is_empty() {
            let teammates: Vec<matchlab_core::player::PlayerId> = observations
                .recent_match_results
                .iter()
                .take(2)
                .map(|m| matchlab_core::player::PlayerId(m.teammates as u64))
                .collect();
            if !teammates.is_empty() {
                AgentAction::FormParty(teammates)
            } else {
                AgentAction::ContinuePlaying
            }
        } else {
            AgentAction::ContinuePlaying
        }
    }
}
/// Adapt after detecting a matchmaking pattern.
pub struct PatternDetectionPolicy {
    pub pattern_window: usize,
}
impl AdaptivePolicy for PatternDetectionPolicy {
    fn decide(
        &self,
        _observations: &AgentObservations,
        history: &[AgentOutcome],
        _objective: &dyn PlayerObjective,
        _rng: &mut SimRng,
    ) -> AgentAction {
        if history.len() < self.pattern_window {
            return AgentAction::ContinuePlaying;
        }
        let recent: &[AgentOutcome] = &history[history.len() - self.pattern_window..];
        let win_rate: f64 = recent
            .iter()
            .filter(|o| o.match_won.unwrap_or(false))
            .count() as f64
            / recent.len() as f64;
        if !(0.2..=0.9).contains(&win_rate) {
            AgentAction::LeaveQueue
        } else {
            AgentAction::ContinuePlaying
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentAction, AgentObservations, NullObjective};
    use matchlab_core::player::Region;
    #[test]
    fn threshold_queue_policy_queues_when_under_threshold() {
        let policy = ThresholdQueuePolicy {
            max_queue_time: 30.0,
        };
        let obs = AgentObservations {
            queue_wait_time: Some(10.0),
            ..AgentObservations::default()
        };
        let action = policy.decide(&obs, &[], &NullObjective, &mut SimRng::from_seed(42));
        assert!(matches!(action, AgentAction::Queue));
    }
    #[test]
    fn threshold_queue_policy_waits_when_over_threshold() {
        let policy = ThresholdQueuePolicy {
            max_queue_time: 30.0,
        };
        let obs = AgentObservations {
            queue_wait_time: Some(60.0),
            ..AgentObservations::default()
        };
        let action = policy.decide(&obs, &[], &NullObjective, &mut SimRng::from_seed(42));
        assert!(matches!(action, AgentAction::NoOp));
    }
    #[test]
    fn loss_aversion_policy_leaves_after_losses() {
        let policy = LossAversionPolicy {
            max_consecutive_losses: 3,
        };
        let history = vec![
            AgentOutcome {
                action: AgentAction::NoOp,
                rating_change: -5.0,
                queue_time: 0.0,
                match_won: Some(false),
                timestamp: 0,
            },
            AgentOutcome {
                action: AgentAction::NoOp,
                rating_change: -3.0,
                queue_time: 0.0,
                match_won: Some(false),
                timestamp: 1,
            },
            AgentOutcome {
                action: AgentAction::NoOp,
                rating_change: -2.0,
                queue_time: 0.0,
                match_won: Some(false),
                timestamp: 2,
            },
        ];
        let action = policy.decide(
            &AgentObservations::default(),
            &history,
            &NullObjective,
            &mut SimRng::from_seed(42),
        );
        assert!(matches!(action, AgentAction::StopPlaying));
    }
    #[test]
    fn region_switch_policy_switches_on_high_latency() {
        let policy = RegionSwitchPolicy {
            latency_threshold: 50.0,
            target_region: Region::EU,
        };
        let obs = AgentObservations {
            estimated_latency: Some(100.0),
            ..AgentObservations::default()
        };
        let action = policy.decide(&obs, &[], &NullObjective, &mut SimRng::from_seed(42));
        assert!(matches!(action, AgentAction::SelectRegion(Region::EU)));
    }
    #[test]
    fn policies_are_deterministic() {
        let policy = ThresholdQueuePolicy {
            max_queue_time: 30.0,
        };
        let obs = AgentObservations {
            queue_wait_time: Some(10.0),
            ..AgentObservations::default()
        };
        let a1 = policy.decide(&obs, &[], &NullObjective, &mut SimRng::from_seed(42));
        let a2 = policy.decide(&obs, &[], &NullObjective, &mut SimRng::from_seed(42));
        assert!(format!("{:?}", a1) == format!("{:?}", a2));
    }
}
