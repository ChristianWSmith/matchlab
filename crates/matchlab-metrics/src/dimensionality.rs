use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};

/// Dimensionality fidelity (spec §11.3): how much fidelity is lost when a 1D
/// rating represents multidimensional skill. Computes the Pearson correlation
/// of 1D ratings vs true overall skill and of the SkillVector prediction vs
/// true overall skill; fidelity = improvement of multiD over 1D.
pub struct DimensionalityFidelityCollector {
    /// (1d_rating, multid_prediction, true_overall)
    samples: Vec<(f64, f64, f64)>,
}

impl DimensionalityFidelityCollector {
    pub fn new() -> Self {
        Self { samples: Vec::new() }
    }
}

impl Default for DimensionalityFidelityCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn pearson(pairs: &[(f64, f64)]) -> f64 {
    if pairs.len() < 2 {
        return 0.0;
    }
    let n = pairs.len() as f64;
    let sum_x: f64 = pairs.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = pairs.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = pairs.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = pairs.iter().map(|(x, _)| x * x).sum();
    let sum_y2: f64 = pairs.iter().map(|(_, y)| y * y).sum();
    let num = n * sum_xy - sum_x * sum_y;
    let den = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

impl MetricCollector for DimensionalityFidelityCollector {
    fn name(&self) -> &str {
        "dimensionality_fidelity"
    }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        for (pid, obs) in &world.observations {
            if let Some(reality) = world.players.get(pid) {
                let true_overall = reality.skill.overall();
                let oned_pred = obs.rating;
                let multid_pred = obs.skill_vector.overall();
                self.samples.push((oned_pred, multid_pred, true_overall));
            }
        }
    }

    fn compute(&self) -> MetricResult {
        if self.samples.is_empty() {
            return MetricResult::Scalar(0.0);
        }

        let oned: Vec<(f64, f64)> = self.samples.iter().map(|(a, _, c)| (*a, *c)).collect();
        let multid: Vec<(f64, f64)> = self.samples.iter().map(|(_, b, c)| (*b, *c)).collect();
        let oned_corr = pearson(&oned);
        let multid_corr = pearson(&multid);

        let fidelity = if oned_corr > 0.0 {
            ((multid_corr - oned_corr) / (1.0 - oned_corr)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        MetricResult::Summary {
            mean: oned_corr,
            median: multid_corr,
            p75: fidelity,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
            stddev: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{
        PlayerId, PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
    };
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn add(world: &mut World, id: u64, rating: f64, skill: f64) {
        world.observations.insert(
            PlayerId(id),
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
                skill_vector: SkillVector::one_dimensional(skill),
                detection_flags: Vec::new(),
            },
        );
        world.players.insert(
            PlayerId(id),
            PlayerReality {
                id: PlayerId(id),
                skill: SkillVector::one_dimensional(skill),
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
                archetype: "stable".into(),
            },
        );
    }

    fn empty_mr() -> MatchResult {
        MatchResult {
            match_id: matchlab_core::match_::MatchId(1),
            winner: matchlab_core::match_::Team::A,
            team_a: Vec::new(),
            team_b: Vec::new(),
            team_a_score: 0.0,
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
    fn perfect_1d_fidelity_scores_high_correlation() {
        let mut world = World::new(SimRng::from_seed(1));
        for i in 1..=10u64 {
            let skill = i as f64 * 100.0;
            add(&mut world, i, skill, skill); // rating == skill → perfect corr
        }
        let mut c = DimensionalityFidelityCollector::new();
        c.record_match(&empty_mr(), &world);
        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!(mean > 0.99, "corr = {mean}"),
            other => panic!("expected Summary, got {other:?}"),
        }
    }

    #[test]
    fn empty_world_is_scalar_zero() {
        let world = World::new(SimRng::from_seed(2));
        let c = DimensionalityFidelityCollector::new();
        // No samples recorded.
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
        let _ = &world;
    }
}