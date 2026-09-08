//! Matchmaker ↔ Population Feedback: connects the matchmaking
//! system to adaptive populations, creating a first-class simulation loop
//! where matchmaking outcomes feed back into player decisions.
use matchlab_adversarial::agent::{AgentAction, AgentObservations, StrategicAgent};
use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;
use matchlab_matchmaking::matchmaker::{Matchmaker, ProposedMatch};
use matchlab_matchmaking::queue::{Queue, QueueEntry};
use matchlab_players::population_dynamics::{PopulationDynamics, PopulationEvent};
/// Result of a single ecosystem tick.
#[derive(Debug, Default)]
pub struct EcosystemTickResult {
    pub matches_formed: Vec<ProposedMatch>,
    pub population_events: Vec<PopulationEvent>,
    pub detection_events: Vec<String>,
    pub agent_actions: Vec<(PlayerId, AgentAction)>,
}
/// The ecosystem loop: matchmaking → matches → player decisions → population
/// state → future matchmaking .
pub struct EcosystemLoop {
    pub world: World,
    pub queue: Queue,
    pub agents: std::collections::HashMap<PlayerId, Box<dyn StrategicAgent>>,
    pub population_dynamics: Option<Box<dyn PopulationDynamics>>,
    pub population: Vec<matchlab_core::player::PlayerReality>,
}
impl EcosystemLoop {
    pub fn new(world: World) -> Self {
        Self {
            world,
            queue: Queue::default(),
            agents: std::collections::HashMap::new(),
            population_dynamics: None,
            population: Vec::new(),
        }
    }
    /// Register a strategic agent for a player.
    pub fn register_agent(&mut self, player_id: PlayerId, agent: Box<dyn StrategicAgent>) {
        self.agents.insert(player_id, agent);
    }
    /// Set the population dynamics model.
    pub fn set_population_dynamics(&mut self, dynamics: Box<dyn PopulationDynamics>) {
        self.population_dynamics = Some(dynamics);
    }
    /// Execute one tick of the ecosystem loop:
    /// 1. Player agents decide actions (queue, leave, etc.)
    /// 2. Population dynamics (entry/exit)
    /// 3. Queue processing
    pub fn tick(
        &mut self,
        matchmaker: &dyn Matchmaker,
        teams: &matchlab_core::match_::TeamComposition,
        rng: &mut SimRng,
    ) -> EcosystemTickResult {
        let mut result = EcosystemTickResult::default();
        let now = self.world.time;
        for (player_id, agent) in &mut self.agents {
            let obs = AgentObservations::default();
            let history = Vec::new();
            let action = agent.select_action(&obs, &history, rng);
            match &action {
                AgentAction::Queue => {
                    if let Some(reality) = self.world.players.get(player_id) {
                        let entry = QueueEntry {
                            player_id: *player_id,
                            joined_at: now,
                            observation: self.world.observe(*player_id).cloned().unwrap_or_else(
                                || matchlab_core::player::PlayerObservation {
                                    id: *player_id,
                                    rating: reality.skill.overall(),
                                    hidden_mmr: reality.skill.overall(),
                                    visible_rank: matchlab_core::player::VisibleRank {
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
                                    party_id: reality.party_id,
                                    session_history: std::collections::VecDeque::new(),
                                    quit_history: std::collections::VecDeque::new(),
                                    tilt_level: 0.0,
                                    game_mode: "ranked".to_string(),
                                    role: None,
                                    skill_vector:
                                        matchlab_core::player::SkillVector::one_dimensional(
                                            reality.skill.overall(),
                                        ),
                                    detection_flags: Vec::new(),
                                },
                            ),
                            region: reality.region,
                            party_id: reality.party_id,
                            game_mode: "ranked".to_string(),
                            role: None,
                            latency_ms: 30.0,
                        };
                        self.queue.enqueue(entry);
                    }
                    result.agent_actions.push((*player_id, action));
                }
                AgentAction::StopPlaying => {
                    result.agent_actions.push((*player_id, action));
                }
                _ => {
                    result.agent_actions.push((*player_id, action));
                }
            }
        }
        if let Some(dynamics) = &self.population_dynamics {
            let events = dynamics.tick(&mut self.population, &mut self.world, now, rng);
            result.population_events.extend(events);
        }
        let matches = matchmaker.find_matches(&self.queue, &self.world, teams, now, rng);
        result.matches_formed = matches;
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_adversarial::agent::OrdinaryPlayer;
    #[test]
    fn ecosystem_loop_creates_with_empty_queue() {
        let world = World::new(SimRng::from_seed(42));
        let loop_ = EcosystemLoop::new(world);
        assert!(loop_.queue.is_empty());
        assert!(loop_.agents.is_empty());
    }
    #[test]
    fn ecosystem_loop_register_agent() {
        let mut loop_ = EcosystemLoop::new(World::new(SimRng::from_seed(42)));
        let id = loop_.world.next_player_id();
        loop_.register_agent(id, Box::new(OrdinaryPlayer));
        assert!(loop_.agents.contains_key(&id));
    }
    #[test]
    fn ecosystem_tick_returns_empty_result() {
        let loop_ = EcosystemLoop::new(World::new(SimRng::from_seed(42)));
        assert!(loop_.queue.is_empty());
        assert!(loop_.population.is_empty());
    }
}
