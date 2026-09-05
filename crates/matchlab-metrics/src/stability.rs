use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use std::collections::HashMap;

use crate::collector::{MetricCollector, MetricResult};

/// Stability (spec §11.3): rating variance for "stable" players (low
/// improvement_rate). A stable system should show small fluctuations for
/// non-drifting players.
pub struct StabilityCollector {
    rating_history: HashMap<PlayerId, Vec<f64>>,
}

impl StabilityCollector {
    pub fn new() -> Self {
        Self {
            rating_history: HashMap::new(),
        }
    }
}

impl Default for StabilityCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for StabilityCollector {
    fn name(&self) -> &str {
        "stability"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            let Some(obs) = world.observations.get(pid) else {
                continue;
            };
            let stable = world
                .players
                .get(pid)
                .map(|reality| reality.improvement_rate.abs() < 0.1)
                .unwrap_or(true);
            if stable {
                self.rating_history
                    .entry(*pid)
                    .or_default()
                    .push(obs.rating);
            }
        }
    }

    fn compute(&self) -> MetricResult {
        // Sort by (variance, player_id) so aggregation is independent of the
        // HashMap's (per-process randomized) iteration order; the id breaks
        // ties between equal variances.
        let mut variances: Vec<(PlayerId, f64)> = Vec::new();
        for (pid, history) in &self.rating_history {
            let mean = history.iter().sum::<f64>() / history.len().max(1) as f64;
            let var = history.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
                / history.len().max(1) as f64;
            variances.push((*pid, var));
        }
        if variances.is_empty() {
            return MetricResult::Scalar(0.0);
        }
        variances.sort_by(|(p1, v1), (p2, v2)| {
            v1.partial_cmp(v2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| p1.0.cmp(&p2.0))
        });
        let mean_var = variances.iter().map(|(_, v)| *v).sum::<f64>() / variances.len() as f64;
        MetricResult::Scalar(mean_var.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{PlayerObservation, Region, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn mr(player: PlayerId) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: vec![player],
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

    fn add(world: &mut World, id: u64, rating: f64, improvement_rate: f64) {
        world.observations.insert(
            PlayerId(id),
            PlayerObservation {
                id: PlayerId(id),
                rating,
                hidden_mmr: rating,
                visible_rank: VisibleRank {
                    tier: "unranked".into(),
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
                improvement_rate,
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

    #[test]
    fn stable_ratings_report_low_stddev() {
        let mut world = World::new(SimRng::from_seed(1));
        add(&mut world, 1, 1000.0, 0.0);
        let mut c = StabilityCollector::new();
        for i in 0..5 {
            world.observations.get_mut(&PlayerId(1)).unwrap().rating = 1000.0 + i as f64;
            c.record_match(&mr(PlayerId(1)), &world);
        }
        let MetricResult::Scalar(s) = c.compute() else {
            panic!("expected scalar");
        };
        // Small fluctuations → small stddev (< 2).
        assert!(s < 2.0, "stddev = {s}");
    }

    #[test]
    fn drifting_players_are_excluded() {
        let mut world = World::new(SimRng::from_seed(2));
        add(&mut world, 1, 1000.0, 5.0); // rapid improvement → excluded
        let mut c = StabilityCollector::new();
        c.record_match(&mr(PlayerId(1)), &world);
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }
}
