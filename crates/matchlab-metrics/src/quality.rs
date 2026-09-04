use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};
use crate::stats::summary_to_result;

/// Per-match balance quality `1 − (|avg_a − avg_b| / 400).clamp(0,1)` computed
/// from observation ratings only (spec §11.3). Summarized over all matches.
pub struct MatchQualityCollector {
    values: Vec<f64>,
}

impl MatchQualityCollector {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl Default for MatchQualityCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for MatchQualityCollector {
    fn name(&self) -> &str {
        "match_quality"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let team_average = |team: &[matchlab_core::player::PlayerId]| -> f64 {
            let sum: f64 = team
                .iter()
                .filter_map(|pid| world.observations.get(pid))
                .map(|o| o.rating)
                .sum();
            sum / team.len().max(1) as f64
        };
        let avg_a = team_average(&mr.team_a);
        let avg_b = team_average(&mr.team_b);
        let diff = (avg_a - avg_b).abs();
        self.values.push(1.0 - (diff / 400.0).min(1.0));
    }

    fn compute(&self) -> MetricResult {
        summary_to_result(&self.values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{
        DetectionFlag, PlayerId, PlayerObservation, Region, SkillVector, VisibleRank,
    };
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
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

    fn build_world_5v5_equal_ratings() -> World {
        let mut world = World::new(SimRng::from_seed(3));
        for id in 1..=10u64 {
            world.add_player(
                matchlab_core::player::PlayerReality {
                    id: PlayerId(id),
                    skill: SkillVector::one_dimensional(1000.0),
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
                },
                obs(id, 1000.0),
            );
        }
        world
    }

    fn match_5v5() -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: (1..=5).map(PlayerId).collect(),
            team_b: (6..=10).map(PlayerId).collect(),
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(30.0),
            disconnected: false,
            forfeited: false,
            variance: 0.1,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn equal_5v5_match_reports_quality_one() {
        let world = build_world_5v5_equal_ratings();
        let mut c = MatchQualityCollector::new();
        c.record_match(&match_5v5(), &world);

        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 1.0).abs() < 1e-9),
            other => panic!("expected Summary, got {other:?}"),
        }
    }

    #[test]
    fn lopsided_match_reports_low_quality() {
        let mut world = World::new(SimRng::from_seed(4));
        for id in 1..=5u64 {
            world.add_player(
                matchlab_core::player::PlayerReality {
                    id: PlayerId(id),
                    skill: SkillVector::one_dimensional(1500.0),
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
                },
                obs(id, 1500.0),
            );
        }
        for id in 6..=10u64 {
            world.add_player(
                matchlab_core::player::PlayerReality {
                    id: PlayerId(id),
                    skill: SkillVector::one_dimensional(800.0),
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
                    archetype: "static".to_string(),
                },
                obs(id, 800.0),
            );
        }

        let mut c = MatchQualityCollector::new();
        c.record_match(&match_5v5(), &world);
        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!(mean < 0.2),
            other => panic!("expected Summary, got {other:?}"),
        }
    }
}
