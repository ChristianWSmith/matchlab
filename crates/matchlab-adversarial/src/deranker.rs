use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::agent::{AdversarialAgent, AdversarialObjective};

/// Intentionally loses matches to drop rating — may AFK, disconnect, or throw.
pub struct DerankerAgent {
    pub target_rating: f64,
}

impl DerankerAgent {
    pub fn new(target_rating: f64) -> Self {
        Self { target_rating }
    }
}

impl AdversarialAgent for DerankerAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        let target = self.target_rating;
        let below_target = world
            .observations
            .get(&player_id)
            .map(|o| o.rating <= target)
            .unwrap_or(true);
        if below_target {
            return;
        }
        if let Some(reality) = world.players.get_mut(&player_id) {
            reality.quit_probability = 0.9;
        }
        if let Some(obs) = world.observations.get_mut(&player_id) {
            obs.tilt_level = 1.0;
        }
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MaintainLowRating
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::rng::SimRng;

    fn world_with_player(id: u64, rating: f64) -> World {
        use matchlab_core::player::{SkillVector, VisibleRank};
        use std::collections::VecDeque;
        let mut world = World::new(SimRng::from_seed(7));
        world.observations.insert(
            PlayerId(id),
            matchlab_core::player::PlayerObservation {
                id: PlayerId(id),
                rating,
                hidden_mmr: rating,
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
                skill_vector: SkillVector::one_dimensional(rating),
                detection_flags: Vec::new(),
            },
        );
        world.players.insert(
            PlayerId(id),
            matchlab_core::player::PlayerReality {
                id: PlayerId(id),
                skill: SkillVector::one_dimensional(rating),
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
        world
    }

    #[test]
    fn deranker_increases_quit_probability_above_target() {
        let mut world = world_with_player(1, 1500.0);
        let mut agent = DerankerAgent::new(1000.0);
        agent.tick(PlayerId(1), &mut world);
        assert_eq!(world.players[&PlayerId(1)].quit_probability, 0.9);
        assert_eq!(world.observations[&PlayerId(1)].tilt_level, 1.0);
    }

    #[test]
    fn deranker_does_nothing_below_target() {
        let mut world = world_with_player(1, 900.0);
        let mut agent = DerankerAgent::new(1000.0);
        agent.tick(PlayerId(1), &mut world);
        assert_eq!(world.players[&PlayerId(1)].quit_probability, 0.01);
        assert_eq!(world.observations[&PlayerId(1)].tilt_level, 0.0);
    }

    #[test]
    fn deranker_objective_is_maintain_low_rating() {
        let agent = DerankerAgent::new(1000.0);
        assert_eq!(agent.objective(), AdversarialObjective::MaintainLowRating);
    }
}