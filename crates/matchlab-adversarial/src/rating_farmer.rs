use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::agent::{AdversarialAgent, AdversarialObjective};

/// Queues, then immediately quits/disconnects after starting. Keeps
/// games_played minimal so the account looks fresh/smurf-like.
pub struct RatingFarmerAgent {
    pub quit_probability: f64,
    pub quit_after_minutes: f64,
}

impl RatingFarmerAgent {
    pub fn new(quit_probability: f64, quit_after_minutes: f64) -> Self {
        Self {
            quit_probability,
            quit_after_minutes,
        }
    }
}

impl AdversarialAgent for RatingFarmerAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        if world.rng.gen_bool(self.quit_probability) {
            if let Some(reality) = world.players.get_mut(&player_id) {
                reality.quit_probability = 1.0;
            }
            if let Some(obs) = world.observations.get_mut(&player_id) {
                obs.is_online = false;
            }
        }
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MaximizeWinRate { target_games: 10 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::rng::SimRng;

    fn world_with_player() -> World {
        use matchlab_core::player::{SkillVector, VisibleRank};
        use std::collections::VecDeque;
        let mut world = World::new(SimRng::from_seed(7));
        world.players.insert(
            PlayerId(1),
            matchlab_core::player::PlayerReality {
                id: PlayerId(1),
                skill: SkillVector::one_dimensional(1000.0),
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
        world.observations.insert(
            PlayerId(1),
            matchlab_core::player::PlayerObservation {
                id: PlayerId(1),
                rating: 1000.0,
                hidden_mmr: 1000.0,
                visible_rank: VisibleRank { tier: "silver".into(), division: 1 },
                rating_deviation: 350.0,
                volatility: 0.06,
                games_played: 0,
                win_rate: 0.5,
                recent_performances: Vec::new(),
                queue_joined_at: None,
                is_online: true,
                party_id: None,
                session_history: VecDeque::new(),
                quit_history: VecDeque::new(),
                tilt_level: 0.0,
                game_mode: "ranked".into(),
                skill_vector: SkillVector::one_dimensional(1000.0),
                detection_flags: Vec::new(),
            },
        );
        world
    }

    #[test]
    fn rating_farmer_quits_and_goes_offline() {
        let mut world = world_with_player();
        let mut agent = RatingFarmerAgent::new(1.0, 5.0);
        agent.tick(PlayerId(1), &mut world);
        assert_eq!(world.players[&PlayerId(1)].quit_probability, 1.0);
        assert!(!world.observations[&PlayerId(1)].is_online);
    }

    #[test]
    fn rating_farmer_does_nothing_when_probability_zero() {
        let mut world = world_with_player();
        let mut agent = RatingFarmerAgent::new(0.0, 5.0);
        agent.tick(PlayerId(1), &mut world);
        assert_eq!(world.players[&PlayerId(1)].quit_probability, 0.01);
        assert!(world.observations[&PlayerId(1)].is_online);
    }

    #[test]
    fn rating_farmer_objective_is_maximize_win_rate() {
        let agent = RatingFarmerAgent::new(0.5, 5.0);
        assert_eq!(
            agent.objective(),
            AdversarialObjective::MaximizeWinRate { target_games: 10 }
        );
    }
}