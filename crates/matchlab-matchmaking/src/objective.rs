use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

use crate::matchmaker::ProposedMatch;
use crate::queue::QueueEntry;

/// Per-match optimization scoring (§7.4). A weighted combination of predicted
/// quality, queue waiting cost, ping cost, and rating-uncertainty cost.
pub struct MatchObjective {
    pub weight_quality: f64,
    pub weight_queue_time: f64,
    pub weight_ping: f64,
    pub weight_rating_uncertainty: f64,
}

impl MatchObjective {
    pub fn new(
        weight_quality: f64,
        weight_queue_time: f64,
        weight_ping: f64,
        weight_rating_uncertainty: f64,
    ) -> Self {
        Self {
            weight_quality,
            weight_queue_time,
            weight_ping,
            weight_rating_uncertainty,
        }
    }

    pub fn score(
        &self,
        proposed: &ProposedMatch,
        queue_entries: &[QueueEntry],
        world: &World,
    ) -> f64 {
        let q = self.match_quality(proposed, world);
        let t = self.queue_time_cost(proposed, queue_entries, world);
        let p = self.ping_cost(proposed, world);
        let r = self.rating_uncertainty_cost(proposed, world);

        self.weight_quality * q
            - self.weight_queue_time * t
            - self.weight_ping * p
            - self.weight_rating_uncertainty * r
    }

    fn match_quality(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        let avg_a = average_rating(&proposed.team_a, world);
        let avg_b = average_rating(&proposed.team_b, world);
        let diff = (avg_a - avg_b).abs();
        1.0 - (diff / 400.0).min(1.0)
    }

    fn queue_time_cost(
        &self,
        proposed: &ProposedMatch,
        queue_entries: &[QueueEntry],
        world: &World,
    ) -> f64 {
        let max_wait = proposed
            .team_a
            .iter()
            .chain(proposed.team_b.iter())
            .filter_map(|pid| {
                queue_entries
                    .iter()
                    .find(|e| e.player_id == *pid)
                    .map(|e| world.time.duration_since(e.joined_at).as_secs_f64())
            })
            .fold(0.0_f64, f64::max);
        max_wait / 60.0 // normalize: 60 sec = cost of 1.0
    }

    fn ping_cost(&self, _proposed: &ProposedMatch, _world: &World) -> f64 {
        0.0 // placeholder: geographic distance model
    }

    fn rating_uncertainty_cost(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        let avg_rd: f64 = proposed
            .team_a
            .iter()
            .chain(proposed.team_b.iter())
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating_deviation)
            .sum::<f64>()
            / (proposed.team_a.len() + proposed.team_b.len()).max(1) as f64;
        avg_rd / 350.0 // normalize by default RD
    }
}

fn average_rating(team: &[PlayerId], world: &World) -> f64 {
    let sum: f64 = team
        .iter()
        .filter_map(|pid| world.observations.get(pid))
        .map(|o| o.rating)
        .sum();
    sum / team.len().max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{DetectionFlag, PlayerObservation, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64, rd: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
                division: 1,
            },
            rating_deviation: rd,
            volatility: 0.06,
            games_played: 10,
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
            detection_flags: Vec::<DetectionFlag>::new(),
            role: None,
        }
    }

    fn entry(id: u64, joined_at: SimTime) -> QueueEntry {
        QueueEntry {
            player_id: PlayerId(id),
            joined_at,
            observation: obs(id, 1000.0, 350.0),
            region: matchlab_core::player::Region::NA,
            party_id: None,
            game_mode: "ranked".into(),
            role: None,
            latency_ms: 30.0,
        }
    }

    fn world_with(ratings: &[(u64, f64, f64)]) -> World {
        let mut world = World::new(SimRng::from_seed(1));
        for &(id, rating, rd) in ratings {
            world.observations.insert(PlayerId(id), obs(id, rating, rd));
        }
        world
    }

    fn objective() -> MatchObjective {
        MatchObjective::new(1.0, 1.0, 1.0, 1.0)
    }

    #[test]
    fn score_with_all_zero_weights_is_zero() {
        let o = MatchObjective::new(0.0, 0.0, 0.0, 0.0);
        let world = world_with(&[(1, 1000.0, 350.0), (2, 1000.0, 350.0)]);
        let pm = ProposedMatch {
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            quality_score: 1.0,
        };
        assert_eq!(o.score(&pm, &[], &world), 0.0);
    }

    #[test]
    fn balanced_match_scores_high() {
        let o = objective();
        let mut world = World::new(SimRng::from_seed(1));
        world.time = SimTime::from_secs(0.0);
        world
            .observations
            .insert(PlayerId(1), obs(1, 1000.0, 100.0));
        world
            .observations
            .insert(PlayerId(2), obs(2, 1000.0, 100.0));
        let entries = vec![entry(1, SimTime::ZERO), entry(2, SimTime::ZERO)];
        let pm = ProposedMatch {
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            quality_score: 1.0,
        };
        let s = o.score(&pm, &entries, &world);
        // quality 1.0, queue cost 0, ping 0, uncertainty 100/350 ≈ 0.2857
        assert!((s - (1.0 - 100.0 / 350.0)).abs() < 1e-6, "score = {s}");
    }

    #[test]
    fn queue_time_cost_increases_with_wait() {
        let o = objective();
        let mut world = World::new(SimRng::from_seed(1));
        world.time = SimTime::from_secs(120.0);
        world
            .observations
            .insert(PlayerId(1), obs(1, 1000.0, 350.0));
        world
            .observations
            .insert(PlayerId(2), obs(2, 1000.0, 350.0));
        let entries = vec![
            entry(1, SimTime::from_secs(0.0)),
            entry(2, SimTime::from_secs(0.0)),
        ];
        let pm = ProposedMatch {
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            quality_score: 1.0,
        };
        let s = o.score(&pm, &entries, &world);
        // 120s wait → cost 2.0 subtracted
        assert!((s - (1.0 - 2.0 - 1.0)).abs() < 1e-6, "score = {s}");
    }

    #[test]
    fn ping_cost_is_placeholder_zero() {
        let o = objective();
        let world = world_with(&[(1, 1000.0, 350.0), (2, 1000.0, 350.0)]);
        let pm = ProposedMatch {
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            quality_score: 1.0,
        };
        assert_eq!(o.ping_cost(&pm, &world), 0.0);
    }
}
