use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::agent::{AdversarialAgent, AdversarialObjective};

/// Randomly goes AFK or disconnects during matches to minimize games played.
pub struct AfkAgent {
    pub go_afk_probability: f64,
}

impl AfkAgent {
    pub fn new(go_afk_probability: f64) -> Self {
        Self { go_afk_probability }
    }
}

impl AdversarialAgent for AfkAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        if world.rng.gen_bool(self.go_afk_probability) {
            if let Some(reality) = world.players.get_mut(&player_id) {
                reality.quit_probability = 1.0;
            }
        }
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MinimizeGamesPlayed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::rng::SimRng;

    #[test]
    fn afk_with_probability_one_always_disconnects() {
        let mut world = World::new(SimRng::from_seed(7));
        world.players.insert(
            PlayerId(1),
            matchlab_core::player::PlayerReality {
                id: PlayerId(1),
                skill: matchlab_core::player::SkillVector::one_dimensional(1000.0),
                skill_volatility: 5.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: matchlab_core::player::Region::NA,
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "stable".into(),
            },
        );
        let mut agent = AfkAgent::new(1.0);
        agent.tick(PlayerId(1), &mut world);
        assert_eq!(world.players[&PlayerId(1)].quit_probability, 1.0);
    }

    #[test]
    fn afk_with_probability_zero_never_disconnects() {
        let mut world = World::new(SimRng::from_seed(7));
        world.players.insert(
            PlayerId(1),
            matchlab_core::player::PlayerReality {
                id: PlayerId(1),
                skill: matchlab_core::player::SkillVector::one_dimensional(1000.0),
                skill_volatility: 5.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: matchlab_core::player::Region::NA,
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "stable".into(),
            },
        );
        let mut agent = AfkAgent::new(0.0);
        agent.tick(PlayerId(1), &mut world);
        assert_eq!(world.players[&PlayerId(1)].quit_probability, 0.01);
    }

    #[test]
    fn afk_objective_is_minimize_games_played() {
        let agent = AfkAgent::new(0.5);
        assert_eq!(agent.objective(), AdversarialObjective::MinimizeGamesPlayed);
    }
}
