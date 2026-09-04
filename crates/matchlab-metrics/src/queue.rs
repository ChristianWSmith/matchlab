use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};

/// Actual queue wait per participant: `now.duration_since(queue_joined_at)`
/// at match formation time (spec §11.3). This measures join → formation wait —
/// never match duration.
pub struct QueueTimeCollector {
    times_secs: Vec<f64>,
}

impl QueueTimeCollector {
    pub fn new() -> Self {
        Self {
            times_secs: Vec::new(),
        }
    }
}

impl Default for QueueTimeCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for QueueTimeCollector {
    fn name(&self) -> &str {
        "queue_time"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let match_time = world.time;
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(obs) = world.observations.get(pid) {
                if let Some(joined_at) = obs.queue_joined_at {
                    let wait = match_time.duration_since(joined_at).as_secs_f64();
                    self.times_secs.push(wait);
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let s = crate::stats::summary(&self.times_secs);
        MetricResult::Summary {
            mean: s.mean,
            median: s.median,
            p75: s.p75,
            p90: s.p90,
            p95: s.p95,
            p99: s.p99,
            stddev: s.stddev,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{
        DetectionFlag, PlayerId, PlayerObservation, SkillVector, VisibleRank,
    };
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn obs(id: u64, joined_at: Option<SimTime>) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: VisibleRank {
                tier: "unranked".to_string(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: joined_at,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            skill_vector: SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn mr_with_long_duration(team_a: Vec<PlayerId>, team_b: Vec<PlayerId>) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a,
            team_b,
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(99_999.0),
            disconnected: false,
            forfeited: false,
            variance: 0.1,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn queue_time_equals_join_to_formation_wait_not_match_duration() {
        let mut world = World::new(SimRng::from_seed(5));
        world.time = SimTime::from_secs(200.0);
        world
            .observations
            .insert(PlayerId(1), obs(1, Some(SimTime::from_secs(190.0))));
        world
            .observations
            .insert(PlayerId(2), obs(2, Some(SimTime::from_secs(195.0))));

        let mut c = QueueTimeCollector::new();
        c.record_match(
            &mr_with_long_duration(vec![PlayerId(1)], vec![PlayerId(2)]),
            &world,
        );

        // Formation at t=200s; 1 queued 10s, 2 queued 5s → waits 10, 5.
        assert_eq!(c.times_secs.len(), 2);
        assert!((c.times_secs[0] - 10.0).abs() < 1e-9);
        assert!((c.times_secs[1] - 5.0).abs() < 1e-9);

        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 7.5).abs() < 1e-9),
            other => panic!("expected Summary, got {other:?}"),
        }
    }

    #[test]
    fn players_without_queue_joined_at_are_skipped() {
        let mut world = World::new(SimRng::from_seed(6));
        world.time = SimTime::from_secs(100.0);
        world.observations.insert(PlayerId(1), obs(1, None));

        let mut c = QueueTimeCollector::new();
        c.record_match(
            &mr_with_long_duration(vec![PlayerId(1)], Vec::new()),
            &world,
        );
        assert!(c.times_secs.is_empty());
    }
}
