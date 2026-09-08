use crate::intervention::{EcosystemIntervention, InterventionAction};
use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
/// Legacy detection system trait (kept for backward compat).
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
/// Extended detection result for the ecosystem model .
#[derive(Debug, Clone)]
pub struct EcosystemDetectionResult {
    pub player_id: PlayerId,
    pub probability_of_anomaly: f64,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub detection_method: String,
}
/// A detector that observes permitted behavioral data and produces
/// a classification, score, or intervention .
pub trait EcosystemDetector: Send + Sync {
    /// Observe a match result for potential detection.
    fn observe_match(&mut self, match_result: &MatchResult, world: &World);
    /// Evaluate a player based on observed data.
    fn evaluate(&self, player_id: PlayerId, world: &World) -> EcosystemDetectionResult;
    /// Recommend an intervention based on the detection result.
    fn recommend_intervention(&self, result: &EcosystemDetectionResult) -> EcosystemIntervention;
    /// The detection method name (for provenance).
    fn method_name(&self) -> String;
}
