use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};

/// Population health (spec §11.3): rating inflation/deflation over time and
/// compression (stddev change). Returns `[inflation, compression,
/// initial_mean, final_mean]`.
pub struct PopulationHealthCollector {
    ratings_over_time: Vec<Vec<f64>>,
}

impl PopulationHealthCollector {
    pub fn new() -> Self {
        Self {
            ratings_over_time: Vec::new(),
        }
    }
}

impl Default for PopulationHealthCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

fn stddev(values: &[f64]) -> f64 {
    let m = mean(values);
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len().max(1) as f64;
    var.sqrt()
}

impl MetricCollector for PopulationHealthCollector {
    fn name(&self) -> &str {
        "population_health"
    }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        let ratings: Vec<f64> = world.observations.values().map(|o| o.rating).collect();
        if !ratings.is_empty() {
            self.ratings_over_time.push(ratings);
        }
    }

    fn compute(&self) -> MetricResult {
        if self.ratings_over_time.is_empty() {
            return MetricResult::Scalar(0.0);
        }

        let initial_mean = mean(&self.ratings_over_time[0]);
        let final_mean = mean(self.ratings_over_time.last().unwrap());
        let inflation = final_mean - initial_mean;

        let initial_stddev = stddev(&self.ratings_over_time[0]);
        let final_stddev = stddev(self.ratings_over_time.last().unwrap());
        let compression = initial_stddev - final_stddev;

        MetricResult::Distribution(vec![inflation, compression, initial_mean, final_mean])
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

    fn mr() -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: Vec::new(),
            team_b: Vec::new(),
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

    #[test]
    fn detects_inflation() {
        let mut world = World::new(SimRng::from_seed(1));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        world.observations.insert(PlayerId(2), obs(2, 1000.0));
        let mut c = PopulationHealthCollector::new();

        c.record_match(&mr(), &world);
        world.observations.get_mut(&PlayerId(1)).unwrap().rating = 1100.0;
        world.observations.get_mut(&PlayerId(2)).unwrap().rating = 1200.0;
        c.record_match(&mr(), &world);

        let MetricResult::Distribution(d) = c.compute() else {
            panic!("expected distribution");
        };
        // Inflation = 1150 − 1000 = 150.
        assert!((d[0] - 150.0).abs() < 1e-9, "inflation = {}", d[0]);
        assert_eq!(d[2], 1000.0);
        assert_eq!(d[3], 1150.0);
    }

    #[test]
    fn empty_is_zero() {
        let c = PopulationHealthCollector::new();
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }
}