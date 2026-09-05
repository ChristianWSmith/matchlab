use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};

/// Smurf metrics (spec §11.3): detection rate, false-positive rate, damage
/// (unfairness), and mean games-to-detection. A player "looks like a smurf" by
/// properties — high skill with low games played — never a boolean flag.
pub struct SmurfMetricsCollector {
    smurf_ids: Vec<PlayerId>,
    detection_events: Vec<DetectionEvent>,
}

#[allow(dead_code)]
struct DetectionEvent {
    player_id: PlayerId,
    detected: bool,
    games_at_detection: Option<u64>,
    damage: f64,
    archetype: String,
}

impl SmurfMetricsCollector {
    pub fn new() -> Self {
        Self {
            smurf_ids: Vec::new(),
            detection_events: Vec::new(),
        }
    }
}

impl Default for SmurfMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn average_obs_rating(team: &[PlayerId], world: &World) -> f64 {
    let sum: f64 = team
        .iter()
        .filter_map(|pid| world.observations.get(pid))
        .map(|o| o.rating)
        .sum();
    sum / team.len().max(1) as f64
}

impl MetricCollector for SmurfMetricsCollector {
    fn name(&self) -> &str {
        "smurf"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            let Some(reality) = world.players.get(pid) else {
                continue;
            };
            let is_smurf = reality.skill.overall() > 1300.0 && reality.games_played < 20;
            if !is_smurf {
                continue;
            }
            let games_at_detection = world.observations.get(pid).map(|o| o.games_played);
            let avg_a = average_obs_rating(&mr.team_a, world);
            let avg_b = average_obs_rating(&mr.team_b, world);
            let p = 1.0 / (1.0 + 10f64.powf((avg_b - avg_a) / 400.0));
            let unfairness = (p - 0.5).abs() * 2.0;
            self.detection_events.push(DetectionEvent {
                player_id: *pid,
                detected: false,
                games_at_detection,
                damage: unfairness,
                archetype: reality.archetype.clone(),
            });
            if !self.smurf_ids.contains(pid) {
                self.smurf_ids.push(*pid);
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let total = self.detection_events.len() as f64;
        if total == 0.0 {
            return MetricResult::Scalar(0.0);
        }

        let mean_damage: f64 = self.detection_events.iter().map(|e| e.damage).sum::<f64>() / total;
        let mean_games: f64 = self
            .detection_events
            .iter()
            .filter_map(|e| e.games_at_detection)
            .map(|g| g as f64)
            .sum::<f64>()
            / total;

        MetricResult::Summary {
            mean: 0.0,   // detection rate (no detection wired yet → 0)
            median: 0.0, // false-positive rate
            p75: mean_damage,
            p90: mean_games,
            p95: self.smurf_ids.len() as f64,
            p99: 0.0,
            stddev: 0.0,
        }
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

    fn mr(a: Vec<PlayerId>, b: Vec<PlayerId>) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
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
                archetype: "smurf".into(),
            },
        );
    }

    #[test]
    fn records_smurf_damage_and_archetype() {
        let mut world = World::new(SimRng::from_seed(1));
        // Smurf: skill 1500, few games, low visible rating.
        add(&mut world, 1, 700.0, 1500.0, 3);
        add(&mut world, 2, 1500.0, 1400.0, 100); // not a smurf (many games)
        let mut c = SmurfMetricsCollector::new();
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        assert_eq!(c.detection_events.len(), 1);
        assert_eq!(c.smurf_ids, vec![PlayerId(1)]);
        assert!(c.detection_events[0].damage > 0.0);
        assert_eq!(c.detection_events[0].archetype, "smurf");
    }

    #[test]
    fn no_smurfs_is_scalar_zero() {
        let mut world = World::new(SimRng::from_seed(2));
        add(&mut world, 2, 1500.0, 1400.0, 100);
        let mut c = SmurfMetricsCollector::new();
        c.record_match(&mr(vec![PlayerId(2)], vec![PlayerId(2)]), &world);
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }
}
