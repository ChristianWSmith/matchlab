use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};
use crate::stats::summary_to_result;

/// Distribution of expected win probabilities across all matches (spec §11.3).
/// A well-matched system clusters near 0.5; a poorly matched system has a
/// fat-tailed distribution.
pub struct MatchInequalityCollector {
    win_probabilities: Vec<f64>,
}

impl MatchInequalityCollector {
    pub fn new() -> Self {
        Self {
            win_probabilities: Vec::new(),
        }
    }
}

impl Default for MatchInequalityCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for MatchInequalityCollector {
    fn name(&self) -> &str {
        "match_inequality"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let avg_a: f64 = mr
            .team_a
            .iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating)
            .sum::<f64>()
            / mr.team_a.len().max(1) as f64;
        let avg_b: f64 = mr
            .team_b
            .iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating)
            .sum::<f64>()
            / mr.team_b.len().max(1) as f64;
        let p = 1.0 / (1.0 + 10f64.powf((avg_b - avg_a) / 400.0));
        self.win_probabilities.push(p);
    }

    fn compute(&self) -> MetricResult {
        summary_to_result(&self.win_probabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{PlayerId, PlayerObservation, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank { tier: "unranked".into(), division: 1 },
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
        }
    }

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
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn balanced_match_records_half() {
        let mut world = World::new(SimRng::from_seed(1));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        let mut c = MatchInequalityCollector::new();
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        assert_eq!(c.win_probabilities.len(), 1);
        assert!((c.win_probabilities[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lopsided_match_records_extreme_probability() {
        let mut world = World::new(SimRng::from_seed(2));
        world.observations.insert(PlayerId(1), obs(1, 1800.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        let mut c = MatchInequalityCollector::new();
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        assert!(c.win_probabilities[0] > 0.9);
    }
}