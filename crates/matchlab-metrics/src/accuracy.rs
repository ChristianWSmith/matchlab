use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};
use crate::stats::summary_to_result;

/// MAE of `obs.rating` vs `reality.skill.overall()` for each match's
/// participants (spec §11.3). Metrics are the only legitimate consumer of
/// `PlayerReality` besides the simulation itself — collectors are read-only
/// aggregators, never feeding algorithms.
///
/// Sampling per participating player (like the match-quality and queue-time
/// collectors) keeps accuracy bounded: a whole-population snapshot per match
/// would store `matches × players` floats and slow the loop by orders of
/// magnitude at manifest scale.
///
/// Each sample is time-stamped so the collector can also emit a `{name}_by_time`
/// `TimeSeries` of equal-duration bucket means — the "MAE decreases over time"
/// convergence evidence for the v0.1 acceptance ticket.
pub struct RatingAccuracyCollector {
    samples: Vec<(u64, f64)>,
}

const TIME_BUCKETS: usize = 20;

impl RatingAccuracyCollector {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }
}

impl Default for RatingAccuracyCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for RatingAccuracyCollector {
    fn name(&self) -> &str {
        "rating_accuracy"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let tick = world.time.ticks();
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(obs) = world.observations.get(pid) {
                if let Some(reality) = world.players.get(pid) {
                    let error = (obs.rating - reality.skill.overall()).abs();
                    self.samples.push((tick, error));
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        summary_to_result(&self.errors())
    }

    fn time_buckets(&self) -> Option<Vec<f64>> {
        if self.samples.is_empty() {
            return None;
        }
        let end = self.samples.iter().map(|(t, _)| *t).max().unwrap();
        let width = if end == 0 {
            1
        } else {
            end.div_ceil(TIME_BUCKETS as u64)
        };
        let mut sums = [0.0f64; TIME_BUCKETS];
        let mut counts = [0u64; TIME_BUCKETS];
        for &(tick, error) in &self.samples {
            let idx = ((tick / width).min(TIME_BUCKETS as u64 - 1)) as usize;
            sums[idx] += error;
            counts[idx] += 1;
        }
        Some(
            sums.iter()
                .zip(counts.iter())
                .map(|(&s, &c)| if c == 0 { 0.0 } else { s / c as f64 })
                .collect(),
        )
    }
}

impl RatingAccuracyCollector {
    fn errors(&self) -> Vec<f64> {
        self.samples.iter().map(|(_, e)| *e).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{
        PlayerId, PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
    };
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

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
            variance: 0.1,
            unexpected_events: Vec::new(),
        }
    }

    fn add(world: &mut World, id: u64, rating: f64, true_skill: f64) {
        let o = PlayerObservation {
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
            detection_flags: Vec::new(),
        };
        let r = PlayerReality {
            id: PlayerId(id),
            skill: SkillVector::one_dimensional(true_skill),
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
        };
        world.add_player(r, o);
    }

    #[test]
    fn rating_accuracy_is_participant_mean_absolute_error_from_reality() {
        let mut world = World::new(SimRng::from_seed(2));
        // Participants 1 (1000→1200, err 200) and 2 (1100→1200, err 100).
        // Non-participant 3 (900→800) is deliberately excluded.
        add(&mut world, 1, 1000.0, 1200.0);
        add(&mut world, 2, 1100.0, 1200.0);
        add(&mut world, 3, 900.0, 800.0);

        let mut c = RatingAccuracyCollector::new();
        c.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        c.record_match(&mr(vec![PlayerId(2)], vec![PlayerId(1)]), &world);

        // Four participant samples: 200, 100, 100, 200 → MAE = 150.
        assert_eq!(c.samples.len(), 4);
        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 150.0).abs() < 1e-9),
            other => panic!("expected Summary, got {other:?}"),
        }
    }

    #[test]
    fn accuracy_samples_are_stable_across_repeated_records() {
        let mut world = World::new(SimRng::from_seed(3));
        add(&mut world, 1, 1000.0, 1200.0);
        add(&mut world, 2, 1100.0, 1200.0);

        let mut a = RatingAccuracyCollector::new();
        let mut b = RatingAccuracyCollector::new();
        for _ in 0..10 {
            a.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
            b.record_match(&mr(vec![PlayerId(1)], vec![PlayerId(2)]), &world);
        }
        assert_eq!(a.samples, b.samples);
    }

    #[test]
    fn time_buckets_split_samples_by_duration_windows() {
        let mut world = World::new(SimRng::from_seed(5));
        add(&mut world, 1, 1000.0, 2000.0); // error 1000
        let mut c = RatingAccuracyCollector::new();

        world.time = SimTime::from_secs(10.0);
        c.record_match(&mr(vec![PlayerId(1)], vec![]), &world);
        world.time = SimTime::from_secs(30.0);
        c.record_match(&mr(vec![PlayerId(1)], vec![]), &world);
        world.time = SimTime::from_secs(50.0);
        c.record_match(&mr(vec![PlayerId(1)], vec![]), &world);

        let buckets = c.time_buckets().expect("samples present");
        assert_eq!(buckets.len(), TIME_BUCKETS);
        // 50s sim span → 2.5s per bucket; 10s/30s/50s → indices 4, 12, 19.
        let expected = [buckets[4], buckets[12], buckets[19]];
        assert!(expected.iter().all(|b| (*b - 1000.0).abs() < 1e-9));
        assert!(
            buckets
                .iter()
                .enumerate()
                .all(|(i, b)| { [4, 12, 19].contains(&i) || (*b - 0.0).abs() < 1e-9 })
        );
    }
}
