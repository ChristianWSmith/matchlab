use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::agent::{AdversarialAgent, AdversarialObjective};

/// Two partners queue together and alternate wins to maintain rating while
/// farming games.
pub struct WinTraderAgent {
    pub partner: PlayerId,
    pub alternating: bool,
}

impl WinTraderAgent {
    pub fn new(partner: PlayerId, alternating: bool) -> Self {
        Self {
            partner,
            alternating,
        }
    }
}

impl AdversarialAgent for WinTraderAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        let party = player_id.0 ^ self.partner.0;
        for pid in [player_id, self.partner] {
            if let Some(obs) = world.observations.get_mut(&pid) {
                obs.party_id = Some(party);
            }
            if let Some(reality) = world.players.get_mut(&pid) {
                reality.party_id = Some(party);
            }
        }
        self.alternating = !self.alternating;
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::WinTrade {
            partner: self.partner,
        }
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
        }
    }

    #[test]
    fn win_trader_links_party() {
        let mut world = World::new(SimRng::from_seed(7));
        world.time = SimTime::from_secs(100.0);
        world.observations.insert(PlayerId(1), obs(1));
        world.observations.insert(PlayerId(2), obs(2));

        let mut agent = WinTraderAgent::new(PlayerId(2), false);
        agent.tick(PlayerId(1), &mut world);

        assert_eq!(world.observations[&PlayerId(1)].party_id, Some(1 ^ 2));
        assert_eq!(world.observations[&PlayerId(2)].party_id, Some(1 ^ 2));
        assert!(agent.alternating);
    }

    #[test]
    fn win_trader_objective_is_win_trade() {
        let agent = WinTraderAgent::new(PlayerId(2), false);
        assert_eq!(
            agent.objective(),
            AdversarialObjective::WinTrade { partner: PlayerId(2) }
        );
    }
}