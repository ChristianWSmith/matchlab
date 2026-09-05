use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use std::collections::HashMap;

use crate::collector::{MetricCollector, MetricResult};

/// Responsiveness (spec §11.3): the fraction of rating updates that moved in
/// the direction the outcome predicts (winners gain, losers lose). Measures
/// how quickly a rating system tracks results.
pub struct ResponsivenessCollector {
    prev_ratings: HashMap<PlayerId, f64>,
    responses: Vec<bool>,
}

impl ResponsivenessCollector {
    pub fn new() -> Self {
        Self {
            prev_ratings: HashMap::new(),
            responses: Vec::new(),
        }
    }
}

impl Default for ResponsivenessCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for ResponsivenessCollector {
    fn name(&self) -> &str {
        "responsiveness"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let winner_is_a = mr.winner == Team::A;
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            let Some(obs) = world.observations.get(pid) else {
                continue;
            };
            let prev = match self.prev_ratings.insert(*pid, obs.rating) {
                Some(p) => p,
                None => continue,
            };
            let delta = obs.rating - prev;
            if delta == 0.0 {
                continue;
            }
            let won = (mr.team_a.contains(pid) && winner_is_a)
                || (mr.team_b.contains(pid) && !winner_is_a);
            let responsive = (delta > 0.0) == won;
            self.responses.push(responsive);
        }
    }

    fn compute(&self) -> MetricResult {
        if self.responses.is_empty() {
            return MetricResult::Scalar(0.0);
        }
        let correct = self.responses.iter().filter(|&&b| b).count() as f64;
        MetricResult::Scalar(correct / self.responses.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{PlayerObservation, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn mr(a: Vec<PlayerId>, b: Vec<PlayerId>, winner: Team) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner,
            team_a: a,
            team_b: b,
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::ZERO,
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    fn add(world: &mut World, id: u64, rating: f64) {
        world.observations.insert(
            PlayerId(id),
            PlayerObservation {
                id: PlayerId(id),
                rating,
                hidden_mmr: rating,
                visible_rank: VisibleRank { tier: "unranked".into(), division: 1 },
                rating_deviation: 350.0,
                volatility: 0.06,
                games_played: 1,
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
    }

    #[test]
    fn responsive_updates_score_one() {
        let mut world = World::new(SimRng::from_seed(1));
        let mut c = ResponsivenessCollector::new();

        add(&mut world, 1, 1000.0);
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)], Team::A), &world);
        // First observation: no prior, skipped.
        assert_eq!(c.responses.len(), 0);

        world.observations.get_mut(&PlayerId(1)).unwrap().rating = 1020.0; // winner gained
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)], Team::A), &world);
        assert_eq!(c.compute(), MetricResult::Scalar(1.0));
    }

    #[test]
    fn unresponsive_updates_score_zero() {
        let mut world = World::new(SimRng::from_seed(2));
        let mut c = ResponsivenessCollector::new();

        add(&mut world, 1, 1000.0);
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)], Team::A), &world);

        world.observations.get_mut(&PlayerId(1)).unwrap().rating = 980.0; // winner LOST rating
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)], Team::A), &world);
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }

    #[test]
    fn empty_is_zero() {
        let c = ResponsivenessCollector::new();
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }
}