use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};
use crate::stats::summary_to_result;

/// MAE of `obs.rating` vs `reality.skill.overall()` for each match's
/// participants (spec §11.3). Metrics are the only legitimate consumer of
/// `PlayerReality` besides the simulation itself — collectors are read-only
/// aggregators, never feeding algorithms.
///
/// Sampling per participating player (like the match-quality and queue-time
/// collectors) keeps accuracy bounded: a whole-population snapshot per match
/// would store `matches × players` floats and slow the loop by orders of
/// magnitude at manifest scale.
pub struct RatingAccuracyCollector {
    errors: Vec<f64>,
}

impl RatingAccuracyCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }
}

impl Default for RatingAccuracyCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for RatingAccuracyCollector {
    fn name(&self) -> &str {
        "rating_accuracy"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(obs) = world.observations.get(pid) {
                if let Some(reality) = world.players.get(pid) {
                    let error = (obs.rating - reality.skill.overall()).abs();
                    self.errors.push(error);
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        summary_to_result(&self.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{
        PlayerId, PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
    };
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn mr(team_a: Vec<PlayerId>, team_b: Vec<PlayerId>) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a,
            team_b,
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::ZERO,
            disconnected: false,
            forfeited: false,
            variance: 0.1,
            unexpected_events: Vec::new(),
        }
    }

    fn add(world: &mut World, id: u64, rating: f64, true_skill: f64) {
        let o = PlayerObservation {
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
            detection_flags: Vec::new(),
        };
        let r = PlayerReality {
            id: PlayerId(id),
            skill: SkillVector::one_dimensional(true_skill),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::NA,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".to_string(),
        };
        world.add_player(r, o);
    }

    #[test]
    fn rating_accuracy_is_participant_mean_absolute_error_from_reality() {
        let mut world = World::new(SimRng::from_seed(2));
        // Participants 1 (1000→1200, err 200) and 2 (1100→1200, err 100).
        // Non-participant 3 (900→800) is deliberately excluded.
        add(&mut world, 1, 1000.0, 1200.0);
        add(&mut world, 2, 1100.0, 1200.0);
        add(&mut world, 3, 900.0, 800.0);

        let mut c = RatingAccuracyCollector::new();
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        c.record_match(&mr(vec![PlayerId(2)], vec![PlayerId(1)]), &world);

        // Four participant samples: 200, 100, 100, 200 → MAE = 150.
        assert_eq!(c.errors.len(), 4);
        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 150.0).abs() < 1e-9),
            other => panic!("expected Summary, got {other:?}"),
        }
    }

    #[test]
    fn accuracy_samples_are_stable_across_repeated_records() {
        let mut world = World::new(SimRng::from_seed(3));
        add(&mut world, 1, 1000.0, 1200.0);
        add(&mut world, 2, 1100.0, 1200.0);

        let mut a = RatingAccuracyCollector::new();
        let mut b = RatingAccuracyCollector::new();
        for _ in 0..10 {
            a.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
            b.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        }
        assert_eq!(a.errors, b.errors);
    }
}
