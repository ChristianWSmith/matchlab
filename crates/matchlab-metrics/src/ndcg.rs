use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};

/// Normalized Discounted Cumulative Gain over match qualities (spec §11.3).
/// Measures whether high-quality matches appear early in the experiment — i.e.
/// whether the matchmaker produces good matches from the start vs ramping up.
pub struct NDCGCollector {
    qualities: Vec<f64>,
}

impl NDCGCollector {
    pub fn new() -> Self {
        Self { qualities: Vec::new() }
    }
}

impl Default for NDCGCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for NDCGCollector {
    fn name(&self) -> &str {
        "ndcg"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let avg_a = mr
            .team_a
            .iter()
            .filter_map(|p| world.observations.get(p))
            .map(|o| o.rating)
            .sum::<f64>()
            / mr.team_a.len().max(1) as f64;
        let avg_b = mr
            .team_b
            .iter()
            .filter_map(|p| world.observations.get(p))
            .map(|o| o.rating)
            .sum::<f64>()
            / mr.team_b.len().max(1) as f64;
        let p = 1.0 / (1.0 + (-(avg_a - avg_b) / 400.0).exp());
        let quality = 1.0 - (p - 0.5).abs() * 2.0;
        self.qualities.push(quality);
    }

    fn compute(&self) -> MetricResult {
        if self.qualities.is_empty() {
            return MetricResult::Scalar(0.0);
        }

        let mut ideal = self.qualities.clone();
        ideal.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let mut dcg = 0.0;
        let mut idcg = 0.0;
        for (i, (actual, ideal_val)) in self.qualities.iter().zip(ideal.iter()).enumerate() {
            let discount = (i as f64 + 2.0).log2();
            dcg += actual / discount;
            idcg += ideal_val / discount;
        }

        let ndcg = if idcg > 0.0 { dcg / idcg } else { 0.0 };
        MetricResult::Scalar(ndcg)
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
    fn empty_is_zero() {
        let c = NDCGCollector::new();
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }

    #[test]
    fn perfectly_ordered_is_one() {
        let mut world = World::new(SimRng::from_seed(1));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        let mut c = NDCGCollector::new();
        for _ in 0..5 {
            c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        }
        // All qualities identical → NDCG = 1.0.
        assert_eq!(c.compute(), MetricResult::Scalar(1.0));
    }

    #[test]
    fn degraded_early_quality_lowers_ndcg() {
        let mut world = World::new(SimRng::from_seed(2));
        // Mix of balanced (quality 1) and lopsided (low quality) matches.
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        world.observations.insert(PlayerId(3), obs(3, 2000.0));
        let mut c = NDCGCollector::new();
        // Lopsided first, balanced later.
        c.record_match(&mr(vec![PlayerId(3)], vec![PlayerId(1)]), &world);
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        let MetricResult::Scalar(ndcg) = c.compute() else {
            panic!("expected scalar");
        };
        assert!(ndcg < 1.0, "ndcg = {ndcg}");
        assert!(ndcg > 0.0);
    }
}