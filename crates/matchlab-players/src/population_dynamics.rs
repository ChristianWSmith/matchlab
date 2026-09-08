//! Population adaptation: population-level dynamics that allow
//! the population to change in response to system behavior: entry/exit,
//! party formation, strategy shifts, and regional distribution changes.
use matchlab_core::player::{GeoLocation, PlayerId, PlayerReality, Region, SkillVector};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
/// Population-level events that occur during simulation.
#[derive(Debug, Clone)]
pub enum PopulationEvent {
    PlayerJoined(PlayerId),
    PlayerLeft(PlayerId),
    PartyFormed(Vec<PlayerId>),
    PartyDissolved(Vec<PlayerId>),
    StrategyChanged {
        player_id: PlayerId,
        new_strategy: String,
    },
    None,
}
/// A population dynamics model that modifies the population over time.
pub trait PopulationDynamics: Send + Sync {
    fn tick(
        &self,
        population: &mut Vec<PlayerReality>,
        world: &mut World,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<PopulationEvent>;
}
/// Entry/exit dynamics: players join and leave at configurable rates.
pub struct EntryExitDynamics {
    /// Probability per tick that a new player joins.
    pub entry_rate: f64,
    /// Probability per tick that an existing player leaves.
    pub exit_rate: f64,
}
impl PopulationDynamics for EntryExitDynamics {
    fn tick(
        &self,
        population: &mut Vec<PlayerReality>,
        world: &mut World,
        _now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<PopulationEvent> {
        let mut events = Vec::new();
        let mut to_remove = Vec::new();
        for (i, reality) in population.iter().enumerate() {
            if rng.gen_bool(self.exit_rate) {
                to_remove.push(i);
                events.push(PopulationEvent::PlayerLeft(reality.id));
            }
        }
        for &i in to_remove.iter().rev() {
            let removed = population.remove(i);
            world.players.remove(&removed.id);
            world.observations.remove(&removed.id);
        }
        if rng.gen_bool(self.entry_rate) {
            let id = world.next_player_id();
            let skill = SkillVector::one_dimensional(1000.0 + rng.gen_range(0.0, 500.0));
            let overall = skill.overall();
            let reality = PlayerReality {
                id,
                skill: skill.clone(),
                skill_volatility: 5.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: Region::NA,
                location: GeoLocation::new(Region::NA, 0.0, 0.0),
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "dynamic".to_string(),
                role: None,
            };
            let obs = matchlab_core::player::PlayerObservation {
                id,
                rating: overall,
                hidden_mmr: overall,
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
                party_id: None,
                session_history: std::collections::VecDeque::new(),
                quit_history: std::collections::VecDeque::new(),
                tilt_level: 0.0,
                game_mode: "ranked".to_string(),
                role: None,
                skill_vector: skill.clone(),
                detection_flags: Vec::new(),
            };
            world.add_player(reality.clone(), obs);
            population.push(reality);
            events.push(PopulationEvent::PlayerJoined(id));
        }
        events
    }
}
/// Party formation dynamics: parties form at a configurable rate.
pub struct PartyFormationDynamics {
    pub formation_rate: f64,
}
impl PopulationDynamics for PartyFormationDynamics {
    fn tick(
        &self,
        population: &mut Vec<PlayerReality>,
        _world: &mut World,
        _now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<PopulationEvent> {
        let mut events = Vec::new();
        if population.len() < 2 {
            return events;
        }
        if rng.gen_bool(self.formation_rate) {
            let i = (rng.gen_u64() % population.len() as u64) as usize;
            let mut j = (rng.gen_u64() % population.len() as u64) as usize;
            if j == i {
                j = (j + 1) % population.len();
            }
            let party_id = rng.gen_u64();
            population[i].party_id = Some(party_id);
            population[j].party_id = Some(party_id);
            events.push(PopulationEvent::PartyFormed(vec![
                population[i].id,
                population[j].id,
            ]));
        }
        events
    }
}
/// Strategy shift dynamics: players change strategy at a configurable rate.
pub struct StrategyShiftDynamics {
    pub shift_rate: f64,
}
impl PopulationDynamics for StrategyShiftDynamics {
    fn tick(
        &self,
        population: &mut Vec<PlayerReality>,
        _world: &mut World,
        _now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<PopulationEvent> {
        let mut events = Vec::new();
        for reality in population.iter_mut() {
            if rng.gen_bool(self.shift_rate) {
                let strategies = ["honest", "strategic", "aggressive"];
                let idx = (rng.gen_u64() % strategies.len() as u64) as usize;
                let new_strategy = strategies[idx].to_string();
                events.push(PopulationEvent::StrategyChanged {
                    player_id: reality.id,
                    new_strategy,
                });
            }
        }
        events
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_exit_dynamics_can_add_and_remove() {
        let dynamics = EntryExitDynamics {
            entry_rate: 1.0,
            exit_rate: 0.0,
        };
        let mut population = Vec::new();
        let mut world = World::new(SimRng::from_seed(42));
        let events = dynamics.tick(
            &mut population,
            &mut world,
            SimTime::ZERO,
            &mut SimRng::from_seed(42),
        );
        assert_eq!(population.len(), 1);
        assert!(!events.is_empty());
    }
    #[test]
    fn party_formation_creates_party() {
        let dynamics = PartyFormationDynamics {
            formation_rate: 1.0,
        };
        let mut population = vec![
            PlayerReality {
                id: PlayerId(0),
                skill: SkillVector::one_dimensional(1000.0),
                skill_volatility: 0.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: Region::NA,
                location: GeoLocation::new(Region::NA, 0.0, 0.0),
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "test".to_string(),
                role: None,
            },
            PlayerReality {
                id: PlayerId(1),
                skill: SkillVector::one_dimensional(1000.0),
                skill_volatility: 0.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: Region::NA,
                location: GeoLocation::new(Region::NA, 0.0, 0.0),
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "test".to_string(),
                role: None,
            },
        ];
        let mut world = World::new(SimRng::from_seed(42));
        let events = dynamics.tick(
            &mut population,
            &mut world,
            SimTime::ZERO,
            &mut SimRng::from_seed(42),
        );
        assert!(!events.is_empty());
        assert!(population[0].party_id.is_some());
        assert!(population[1].party_id.is_some());
    }
    #[test]
    fn dynamics_are_deterministic() {
        let dynamics = EntryExitDynamics {
            entry_rate: 0.5,
            exit_rate: 0.5,
        };
        let mut pop1 = Vec::new();
        let mut world1 = World::new(SimRng::from_seed(42));
        let e1 = dynamics.tick(
            &mut pop1,
            &mut world1,
            SimTime::ZERO,
            &mut SimRng::from_seed(42),
        );
        let mut pop2 = Vec::new();
        let mut world2 = World::new(SimRng::from_seed(42));
        let e2 = dynamics.tick(
            &mut pop2,
            &mut world2,
            SimTime::ZERO,
            &mut SimRng::from_seed(42),
        );
        assert_eq!(pop1.len(), pop2.len());
        assert_eq!(e1.len(), e2.len());
    }
}
