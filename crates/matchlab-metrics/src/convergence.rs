use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use std::collections::HashMap;

use crate::collector::{MetricCollector, MetricResult};
use crate::stats::summary_to_result;

/// Convergence (spec §11.3): the number of games each player needs before
/// `|rating − true_skill|` falls below a threshold. Fewer games is better.
pub struct ConvergenceCollector {
    convergence_games: HashMap<PlayerId, Option<u64>>,
    threshold: f64,
}

impl ConvergenceCollector {
    pub fn new(threshold: f64) -> Self {
        Self {
            convergence_games: HashMap::new(),
            threshold,
        }
    }
}

impl Default for ConvergenceCollector {
    fn default() -> Self {
        Self::new(50.0)
    }
}

impl MetricCollector for ConvergenceCollector {
    fn name(&self) -> &str {
        "convergence"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let (Some(obs), Some(reality)) =
                (world.observations.get(pid), world.players.get(pid))
            {
                let error = (obs.rating - reality.skill.overall()).abs();
                let entry = self.convergence_games.entry(*pid).or_insert(None);
                if error < self.threshold && entry.is_none() {
                    *entry = Some(obs.games_played);
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        // Sort by (games, player_id) so aggregation is independent of the
        // HashMap's (per-process randomized) iteration order. The player id
        // breaks ties between equal game counts, giving a total order.
        let mut games: Vec<(PlayerId, f64)> = self
            .convergence_games
            .iter()
            .filter_map(|(pid, v)| v.map(|g| (*pid, g as f64)))
            .collect();
        games.sort_by(|(p1, g1), (p2, g2)| {
            g1.partial_cmp(g2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| p1.0.cmp(&p2.0))
        });
        let values: Vec<f64> = games.into_iter().map(|(_, g)| g).collect();
        if values.is_empty() {
            return MetricResult::Scalar(f64::INFINITY);
        }
        summary_to_result(&values)
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

    fn add(world: &mut World, id: u64, rating: f64, skill: f64, games: u64) {
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
                games_played: games,
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
            matchlab_core::player::PlayerReality {
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
                games_played: games,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "stable".into(),
            },
        );
    }

    #[test]
    fn records_games_to_convergence() {
        let mut world = World::new(SimRng::from_seed(1));
        add(&mut world, 1, 1000.0, 1020.0, 5); // error 20 < 50 → converged at 5
        add(&mut world, 2, 1000.0, 900.0, 30); // error 100 ≥ 50 → never
        let mut c = ConvergenceCollector::new(50.0);
        c.record_match(&mr(PlayerId(1)), &world);
        c.record_match(&mr(PlayerId(2)), &world);

        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 5.0).abs() < 1e-9),
            other => panic!("expected Summary, got {other:?}"),
        }
    }

    #[test]
    fn none_converged_is_infinity() {
        let mut world = World::new(SimRng::from_seed(2));
        add(&mut world, 1, 1000.0, 1200.0, 10); // error 200, threshold 50
        let mut c = ConvergenceCollector::new(50.0);
        c.record_match(&mr(PlayerId(1)), &world);
        assert_eq!(c.compute(), MetricResult::Scalar(f64::INFINITY));
    }
}
