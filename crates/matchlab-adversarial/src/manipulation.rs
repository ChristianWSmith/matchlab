//! Adversarial matchmaking manipulation: configurable strategy
//! modules for rating manipulation, queue manipulation, party manipulation,
//! region manipulation, and information exploitation.
use crate::agent::{AgentObservations, AgentOutcome};
use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
/// A manipulation strategy that an agent can employ.
pub trait ManipulationStrategy: Send + Sync {
    fn manipulate(
        &self,
        observations: &AgentObservations,
        history: &[AgentOutcome],
        rng: &mut SimRng,
    ) -> ManipulationAction;
}
/// The result of a manipulation strategy.
#[derive(Debug, Clone)]
pub enum ManipulationAction {
    /// Alter observed rating.
    RatingManipulation { target_rating: f64 },
    /// Delay queue entry.
    QueueManipulation { delay_seconds: f64 },
    /// Form or dissolve a party.
    PartyManipulation {
        form_party: bool,
        members: Vec<PlayerId>,
    },
    /// Switch to a different region.
    RegionManipulation {
        target_region: matchlab_core::player::Region,
    },
    /// Infer hidden matchmaking behavior.
    InformationExploitation {
        inferred_data: std::collections::HashMap<String, f64>,
    },
    /// No manipulation.
    None,
}
/// Lose matches to lower visible rating (smurfing / deranking).
pub struct RatingDumpStrategy {
    pub target_rating: f64,
}
impl ManipulationStrategy for RatingDumpStrategy {
    fn manipulate(
        &self,
        observations: &AgentObservations,
        _history: &[AgentOutcome],
        _rng: &mut SimRng,
    ) -> ManipulationAction {
        if observations.own_rating > self.target_rating {
            ManipulationAction::RatingManipulation {
                target_rating: self.target_rating,
            }
        } else {
            ManipulationAction::None
        }
    }
}
/// Time queue entry for favorable conditions.
pub struct QueueGamingStrategy {
    pub ideal_queue_time: f64,
}
impl ManipulationStrategy for QueueGamingStrategy {
    fn manipulate(
        &self,
        observations: &AgentObservations,
        _history: &[AgentOutcome],
        _rng: &mut SimRng,
    ) -> ManipulationAction {
        match observations.queue_wait_time {
            Some(wait) if wait < self.ideal_queue_time => ManipulationAction::QueueManipulation {
                delay_seconds: self.ideal_queue_time - wait,
            },
            _ => ManipulationAction::None,
        }
    }
}
/// Form parties to exploit party-aware matchmaking.
pub struct PartyExploitStrategy {
    pub target_party_size: usize,
}
impl ManipulationStrategy for PartyExploitStrategy {
    fn manipulate(
        &self,
        observations: &AgentObservations,
        _history: &[AgentOutcome],
        _rng: &mut SimRng,
    ) -> ManipulationAction {
        if observations.party_size < self.target_party_size && observations.party_size > 0 {
            ManipulationAction::PartyManipulation {
                form_party: true,
                members: observations.party_members.clone(),
            }
        } else {
            ManipulationAction::None
        }
    }
}
/// Switch regions to find easier matches.
pub struct RegionHoppingStrategy {
    pub target_region: matchlab_core::player::Region,
}
impl ManipulationStrategy for RegionHoppingStrategy {
    fn manipulate(
        &self,
        _observations: &AgentObservations,
        _history: &[AgentOutcome],
        _rng: &mut SimRng,
    ) -> ManipulationAction {
        ManipulationAction::RegionManipulation {
            target_region: self.target_region,
        }
    }
}
/// Infer hidden matchmaking behavior from observable outcomes.
pub struct InformationInferenceStrategy;
impl ManipulationStrategy for InformationInferenceStrategy {
    fn manipulate(
        &self,
        observations: &AgentObservations,
        history: &[AgentOutcome],
        _rng: &mut SimRng,
    ) -> ManipulationAction {
        let mut inferred = std::collections::HashMap::new();
        if !history.is_empty() {
            let avg_change: f64 =
                history.iter().map(|o| o.rating_change).sum::<f64>() / history.len() as f64;
            inferred.insert("avg_rating_change".to_string(), avg_change);
        }
        if observations.queue_wait_time.is_some() {
            inferred.insert(
                "queue_pattern".to_string(),
                observations.queue_wait_time.unwrap_or(0.0),
            );
        }
        ManipulationAction::InformationExploitation {
            inferred_data: inferred,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentObservations;
    #[test]
    fn rating_dump_strategy_manipulates_when_above_target() {
        let strategy = RatingDumpStrategy {
            target_rating: 1000.0,
        };
        let obs = AgentObservations {
            own_rating: 1500.0,
            ..AgentObservations::default()
        };
        let action = strategy.manipulate(&obs, &[], &mut SimRng::from_seed(42));
        assert!(matches!(
            action,
            ManipulationAction::RatingManipulation { .. }
        ));
    }
    #[test]
    fn rating_dump_strategy_no_action_when_below_target() {
        let strategy = RatingDumpStrategy {
            target_rating: 1000.0,
        };
        let obs = AgentObservations {
            own_rating: 800.0,
            ..AgentObservations::default()
        };
        let action = strategy.manipulate(&obs, &[], &mut SimRng::from_seed(42));
        assert!(matches!(action, ManipulationAction::None));
    }
    #[test]
    fn queue_gaming_strategy_delays() {
        let strategy = QueueGamingStrategy {
            ideal_queue_time: 30.0,
        };
        let obs = AgentObservations {
            queue_wait_time: Some(10.0),
            ..AgentObservations::default()
        };
        let action = strategy.manipulate(&obs, &[], &mut SimRng::from_seed(42));
        match action {
            ManipulationAction::QueueManipulation { delay_seconds } => {
                assert!((delay_seconds - 20.0).abs() < 1e-9);
            }
            _ => panic!("expected QueueManipulation"),
        }
    }
    #[test]
    fn information_inference_strategy_infers() {
        let strategy = InformationInferenceStrategy;
        let history = vec![
            AgentOutcome {
                action: crate::agent::AgentAction::NoOp,
                rating_change: 10.0,
                queue_time: 5.0,
                match_won: Some(true),
                timestamp: 0,
            },
            AgentOutcome {
                action: crate::agent::AgentAction::NoOp,
                rating_change: -5.0,
                queue_time: 15.0,
                match_won: Some(false),
                timestamp: 1,
            },
        ];
        let obs = AgentObservations {
            queue_wait_time: Some(12.0),
            ..AgentObservations::default()
        };
        let action = strategy.manipulate(&obs, &history, &mut SimRng::from_seed(42));
        match action {
            ManipulationAction::InformationExploitation { inferred_data } => {
                assert!(inferred_data.contains_key("avg_rating_change"));
                assert!((inferred_data["avg_rating_change"] - 2.5).abs() < 1e-9);
            }
            _ => panic!("expected InformationExploitation"),
        }
    }
    #[test]
    fn strategies_are_deterministic() {
        let strategy = RatingDumpStrategy {
            target_rating: 1000.0,
        };
        let obs = AgentObservations {
            own_rating: 1500.0,
            ..AgentObservations::default()
        };
        let a1 = strategy.manipulate(&obs, &[], &mut SimRng::from_seed(42));
        let a2 = strategy.manipulate(&obs, &[], &mut SimRng::from_seed(42));
        assert!(format!("{:?}", a1) == format!("{:?}", a2));
    }
}
