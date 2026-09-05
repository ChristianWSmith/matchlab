use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::intervention::InterventionAction;

pub trait DetectionSystem: Send + Sync {
    fn observe(&mut self, match_result: &MatchResult, world: &World);
    fn evaluate(&self, player_id: PlayerId, world: &World) -> DetectionResult;
    fn recommend_action(&self, result: &DetectionResult) -> InterventionAction;
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub player_id: PlayerId,
    pub probability_of_anomaly: f64,
    pub confidence: f64,
    pub evidence: Vec<String>,
}
