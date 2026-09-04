use std::collections::HashMap;

use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

use crate::collector::{MetricCollector, MetricResult};

/// Aggregates registered collectors over the course of a run, then folds each
/// into a named result (spec §11.1).
#[derive(Default)]
pub struct MetricsEngine {
    collectors: Vec<Box<dyn MetricCollector>>,
    results: HashMap<String, MetricResult>,
}

impl MetricsEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, collector: Box<dyn MetricCollector>) {
        self.collectors.push(collector);
    }

    pub fn record_match(&mut self, match_result: &MatchResult, world: &World) {
        for collector in &mut self.collectors {
            collector.record_match(match_result, world);
        }
    }

    pub fn finalize(&mut self) {
        self.results.clear();
        for collector in &self.collectors {
            self.results
                .insert(collector.name().to_string(), collector.compute());
            if let Some(bucket_means) = collector.time_buckets() {
                self.results.insert(
                    format!("{}_by_time", collector.name()),
                    MetricResult::TimeSeries { bucket_means },
                );
            }
        }
    }

    pub fn results(&self) -> &HashMap<String, MetricResult> {
        &self.results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accuracy::RatingAccuracyCollector;
    use crate::quality::MatchQualityCollector;
    use crate::queue::QueueTimeCollector;
    use matchlab_core::match_::{MatchId, MatchResult, Team};
    use matchlab_core::player::{
        PlayerId, PlayerObservation, PlayerReality, SkillVector, VisibleRank,
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
            detection_flags: Vec::new(),
        }
    }

    fn reality(id: u64, skill: f64) -> PlayerReality {
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
            region: matchlab_core::player::Region::NA,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".to_string(),
        }
    }

    fn result(team_a: Vec<PlayerId>, team_b: Vec<PlayerId>) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a,
            team_b,
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
    fn engine_finalize_aggregates_all_registered_collectors() {
        let mut world = World::new(SimRng::from_seed(1));
        world.time = SimTime::from_secs(300.0);
        for id in 1..=4u64 {
            let mut o = obs(id, 1000.0);
            o.queue_joined_at = Some(SimTime::from_secs(290.0));
            world.add_player(reality(id, 1500.0), o);
        }

        let mut engine = MetricsEngine::new();
        engine.register(Box::new(RatingAccuracyCollector::new()));
        engine.register(Box::new(MatchQualityCollector::new()));
        engine.register(Box::new(QueueTimeCollector::new()));

        for _ in 0..3 {
            engine.record_match(
                &result(
                    vec![PlayerId(1), PlayerId(2)],
                    vec![PlayerId(3), PlayerId(4)],
                ),
                &world,
            );
        }
        engine.finalize();

        let results = engine.results();
        assert!(results.contains_key("rating_accuracy"));
        assert!(results.contains_key("match_quality"));
        assert!(results.contains_key("queue_time"));
        // Accuracy collector emits the time-bucketed convergence series.
        assert!(results.contains_key("rating_accuracy_by_time"));
        match &results["rating_accuracy_by_time"] {
            MetricResult::TimeSeries { bucket_means } => assert_eq!(bucket_means.len(), 20),
            other => panic!("expected TimeSeries, got {other:?}"),
        }
    }
}
