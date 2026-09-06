use matchlab_core::match_::TeamComposition;
use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;

use crate::queue::Queue;

pub trait Matchmaker: Send + Sync {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        teams: &TeamComposition,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch>;
}

#[derive(Debug, Clone)]
pub struct ProposedMatch {
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
    pub quality_score: f64,
}

impl ProposedMatch {
    /// Predicted balance quality: 1.0 when the team averages match, 0.0 when
    /// they differ by 400+ rating points. Computed from `world.observations`
    /// only — never from `PlayerReality` (truth separation).
    pub fn match_quality(team_a: &[PlayerId], team_b: &[PlayerId], world: &World) -> f64 {
        let avg_a: f64 = team_a
            .iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating)
            .sum::<f64>()
            / team_a.len().max(1) as f64;
        let avg_b: f64 = team_b
            .iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating)
            .sum::<f64>()
            / team_b.len().max(1) as f64;
        let diff = (avg_a - avg_b).abs();
        1.0 - (diff / 400.0).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{DetectionFlag, PlayerObservation, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
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
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    #[test]
    fn match_quality_equal_teams_is_one() {
        let mut world = World::new(SimRng::from_seed(1));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        world.observations.insert(PlayerId(3), obs(3, 1000.0));
        world.observations.insert(PlayerId(4), obs(4, 1000.0));

        let q = ProposedMatch::match_quality(
            &[PlayerId(1), PlayerId(2)],
            &[PlayerId(3), PlayerId(4)],
            &world,
        );
        assert!((q - 1.0).abs() < 1e-9, "quality = {q}");
    }

    #[test]
    fn match_quality_lopsided_approaches_zero() {
        let mut world = World::new(SimRng::from_seed(2));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        world.observations.insert(PlayerId(3), obs(3, 1800.0));
        world.observations.insert(PlayerId(4), obs(4, 1800.0));

        let q = ProposedMatch::match_quality(
            &[PlayerId(1), PlayerId(2)],
            &[PlayerId(3), PlayerId(4)],
            &world,
        );
        // diff = 800 → clamped to 1.0 → quality 0.0
        assert_eq!(q, 0.0);
    }

    #[test]
    fn match_quality_is_computed_from_observations_not_reality() {
        let mut world = World::new(SimRng::from_seed(3));
        // Reality is wildly unbalanced (true skill 1500 vs 500) but the
        // observations are balanced (rating 1000 vs 1000). Quality must follow
        // the observations only — matching algorithms never read reality.
        let reality = |id: u64, skill: f64| matchlab_core::player::PlayerReality {
            id: PlayerId(id),
            skill: SkillVector::one_dimensional(skill),
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
            archetype: "stable".to_string(),
        };
        world.add_player(reality(1, 1500.0), obs(1, 1000.0));
        world.add_player(reality(2, 500.0), obs(2, 1000.0));

        let q = ProposedMatch::match_quality(&[PlayerId(1)], &[PlayerId(2)], &world);
        assert!((q - 1.0).abs() < 1e-9, "quality = {q}");
    }

    #[test]
    fn match_quality_partial_gap_scales_between_zero_and_one() {
        let mut world = World::new(SimRng::from_seed(4));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1200.0));

        // diff = 200 → quality 0.5
        let q = ProposedMatch::match_quality(&[PlayerId(1)], &[PlayerId(2)], &world);
        assert!((q - 0.5).abs() < 1e-9, "quality = {q}");
    }
}
