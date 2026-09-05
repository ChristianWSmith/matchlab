use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::agent::{AdversarialAgent, AdversarialObjective};

/// A boosting duo: one player (the booster) deliberately underperforms while
/// their partner (the boostee) is carried to a higher rating.
pub struct BoosterAgent {
    pub boost_target: PlayerId,
    pub boostee: PlayerId,
}

impl BoosterAgent {
    pub fn new(boost_target: PlayerId, boostee: PlayerId) -> Self {
        Self {
            boost_target,
            boostee,
        }
    }
}

impl AdversarialAgent for BoosterAgent {
    fn tick(&mut self, _player_id: PlayerId, world: &mut World) {
        let party = self.boost_target.0 ^ self.boostee.0;
        for pid in [self.boost_target, self.boostee] {
            if let Some(obs) = world.observations.get_mut(&pid) {
                obs.party_id = Some(party);
            }
            if let Some(reality) = world.players.get_mut(&pid) {
                reality.party_id = Some(party);
            }
        }
        if let Some(obs) = world.observations.get_mut(&self.boostee) {
            obs.win_rate = 1.0;
        }
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MaximizeRating
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;

    fn obs(id: u64) -> matchlab_core::player::PlayerObservation {
        use matchlab_core::player::{SkillVector, VisibleRank};
        use std::collections::VecDeque;
        matchlab_core::player::PlayerObservation {
            id: PlayerId(id),
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: VisibleRank {
                tier: "silver".into(),
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
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
        }
    }

    fn world_with_players() -> World {
        let mut world = World::new(SimRng::from_seed(7));
        world.time = SimTime::from_secs(100.0);
        world.observations.insert(PlayerId(1), obs(1));
        world.observations.insert(PlayerId(2), obs(2));
        let reality = |id: u64| matchlab_core::player::PlayerReality {
            id: PlayerId(id),
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
        };
        world.players.insert(PlayerId(1), reality(1));
        world.players.insert(PlayerId(2), reality(2));
        world
    }

    #[test]
    fn booster_links_party_and_boosts_boostee_win_rate() {
        let mut world = world_with_players();
        let mut agent = BoosterAgent::new(PlayerId(1), PlayerId(2));
        agent.tick(PlayerId(1), &mut world);

        assert_eq!(world.observations[&PlayerId(1)].party_id, Some(1 ^ 2));
        assert_eq!(world.observations[&PlayerId(2)].party_id, Some(1 ^ 2));
        assert_eq!(world.observations[&PlayerId(2)].win_rate, 1.0);
    }

    #[test]
    fn booster_objective_is_maximize_rating() {
        let agent = BoosterAgent::new(PlayerId(1), PlayerId(2));
        assert_eq!(agent.objective(), AdversarialObjective::MaximizeRating);
    }
}
