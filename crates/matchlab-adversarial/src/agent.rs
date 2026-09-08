//! Strategic agent model: first-class abstraction for strategic
//! player behavior. Agents select among actions based on observations and
//! objectives, using the same simulation machinery as ordinary players.
use matchlab_core::player::{PlayerId, Region};
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;
/// Legacy adversarial agent trait (kept for backward compat).
pub trait AdversarialAgent: Send + Sync {
    fn tick(&mut self, player_id: PlayerId, rng: &mut SimRng, world: &mut World);
    fn objective(&self) -> AdversarialObjective;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdversarialObjective {
    MaximizeRating,
    MinimizeGamesPlayed,
    MaximizeWinRate { target_games: u64 },
    MaintainLowRating,
    WinTrade { partner: PlayerId },
    Derate,
}
/// What an agent can observe about itself and the world.
#[derive(Debug, Clone)]
pub struct AgentObservations {
    pub own_rating: f64,
    pub own_rating_deviation: f64,
    pub own_games_played: u64,
    pub own_win_rate: f64,
    pub own_skill_overall: f64,
    pub queue_wait_time: Option<f64>,
    pub recent_match_results: Vec<MatchSummary>,
    pub party_members: Vec<PlayerId>,
    pub party_size: usize,
    pub region: Region,
    pub estimated_latency: Option<f64>,
    pub current_streak: i32,
}
impl Default for AgentObservations {
    fn default() -> Self {
        Self {
            own_rating: 0.0,
            own_rating_deviation: 0.0,
            own_games_played: 0,
            own_win_rate: 0.0,
            own_skill_overall: 0.0,
            queue_wait_time: None,
            recent_match_results: Vec::new(),
            party_members: Vec::new(),
            party_size: 0,
            region: Region::NA,
            estimated_latency: None,
            current_streak: 0,
        }
    }
}
/// Summary of a recent match from the agent's perspective.
#[derive(Debug, Clone)]
pub struct MatchSummary {
    pub won: bool,
    pub rating_change: f64,
    pub queue_time: f64,
    pub teammates: usize,
    pub opponents: usize,
}
/// Actions available to a strategic agent.
#[derive(Debug, Clone)]
pub enum AgentAction {
    Queue,
    LeaveQueue,
    ContinuePlaying,
    StopPlaying,
    FormParty(Vec<PlayerId>),
    LeaveParty,
    SelectRegion(Region),
    ModifyObservable { key: String, value: f64 },
    NoOp,
}
/// The result of an agent's action.
#[derive(Debug, Clone)]
pub struct AgentOutcome {
    pub action: AgentAction,
    pub rating_change: f64,
    pub queue_time: f64,
    pub match_won: Option<bool>,
    pub timestamp: u64,
}
/// A general-purpose player objective .
pub trait PlayerObjective: Send + Sync {
    fn evaluate(&self, observations: &AgentObservations, history: &[AgentOutcome]) -> f64;
    fn description(&self) -> String;
}
/// A strategic agent that observes, decides, and adapts ().
pub trait StrategicAgent: Send + Sync {
    /// Build the agent's observation from the world state.
    fn observe(
        &self,
        player_id: PlayerId,
        world: &World,
        queue_wait: Option<f64>,
    ) -> AgentObservations;
    /// Select an action based on current observations and history.
    fn select_action(
        &self,
        observations: &AgentObservations,
        history: &[AgentOutcome],
        rng: &mut SimRng,
    ) -> AgentAction;
    /// The objective this agent is pursuing.
    fn objective(&self) -> Box<dyn PlayerObjective>;
    /// Update internal state after an action's outcome.
    fn update(&mut self, outcome: AgentOutcome);
}
/// A non-strategic (ordinary) player — the simplest policy.
pub struct OrdinaryPlayer;
impl StrategicAgent for OrdinaryPlayer {
    fn observe(
        &self,
        _player_id: PlayerId,
        _world: &World,
        _queue_wait: Option<f64>,
    ) -> AgentObservations {
        AgentObservations::default()
    }
    fn select_action(
        &self,
        _observations: &AgentObservations,
        _history: &[AgentOutcome],
        _rng: &mut SimRng,
    ) -> AgentAction {
        AgentAction::ContinuePlaying
    }
    fn objective(&self) -> Box<dyn PlayerObjective> {
        Box::new(NullObjective)
    }
    fn update(&mut self, _outcome: AgentOutcome) {}
}
/// Null objective: no preference.
pub struct NullObjective;
impl PlayerObjective for NullObjective {
    fn evaluate(&self, _obs: &AgentObservations, _history: &[AgentOutcome]) -> f64 {
        0.0
    }
    fn description(&self) -> String {
        "null".to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::Region;
    #[test]
    fn ordinary_player_returns_continue() {
        let player = OrdinaryPlayer;
        let obs = AgentObservations::default();
        let action = player.select_action(&obs, &[], &mut SimRng::from_seed(42));
        assert!(matches!(action, AgentAction::ContinuePlaying));
    }
    #[test]
    fn null_objective_returns_zero() {
        let obj = NullObjective;
        let obs = AgentObservations::default();
        assert_eq!(obj.evaluate(&obs, &[]), 0.0);
    }
    #[test]
    fn agent_observations_default() {
        let obs = AgentObservations::default();
        assert_eq!(obs.own_rating, 0.0);
        assert_eq!(obs.party_size, 0);
        assert!(obs.recent_match_results.is_empty());
    }
    #[test]
    fn agent_action_clone() {
        let action = AgentAction::SelectRegion(Region::NA);
        let cloned = action.clone();
        assert!(matches!(cloned, AgentAction::SelectRegion(Region::NA)));
    }
}
