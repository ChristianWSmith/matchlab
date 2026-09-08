use crate::matchmaker::ProposedMatch;
use crate::queue::QueueEntry;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
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
        max_wait / 60.0
    }
    fn ping_cost(&self, _proposed: &ProposedMatch, _world: &World) -> f64 {
        0.0
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
        avg_rd / 350.0
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
/// Multi-objective vector for match quality .
#[derive(Debug, Clone, PartialEq)]
pub struct MatchObjectiveVector {
    pub skill_quality: f64,
    pub role_quality: f64,
    pub latency_quality: f64,
    pub party_quality: f64,
    pub wait_cost: f64,
    pub utilization: f64,
}
impl Default for MatchObjectiveVector {
    fn default() -> Self {
        Self {
            skill_quality: 0.0,
            role_quality: 0.0,
            latency_quality: 0.0,
            party_quality: 0.0,
            wait_cost: 0.0,
            utilization: 0.0,
        }
    }
}
impl MatchObjectiveVector {
    /// Scalarize the objective vector with given weights.
    pub fn scalarize(&self, weights: &ObjectiveWeights) -> f64 {
        weights.skill_quality * self.skill_quality
            + weights.role_quality * self.role_quality
            + weights.latency_quality * self.latency_quality
            + weights.party_quality * self.party_quality
            - weights.wait_cost * self.wait_cost
            + weights.utilization * self.utilization
    }
    /// Check if this objective vector dominates another (Pareto dominance).
    pub fn dominates(&self, other: &Self) -> bool {
        let mut strictly_better = false;
        let dims = [
            self.skill_quality,
            self.role_quality,
            self.latency_quality,
            self.party_quality,
            self.utilization,
        ];
        let other_dims = [
            other.skill_quality,
            other.role_quality,
            other.latency_quality,
            other.party_quality,
            other.utilization,
        ];
        for (a, b) in dims.iter().zip(other_dims.iter()) {
            if a < b {
                return false;
            }
            if a > b {
                strictly_better = true;
            }
        }
        if self.wait_cost < other.wait_cost {
            strictly_better = true;
        } else if self.wait_cost > other.wait_cost {
            return false;
        }
        strictly_better
    }
}
/// Weights for scalarizing the objective vector .
#[derive(Debug, Clone)]
pub struct ObjectiveWeights {
    pub skill_quality: f64,
    pub role_quality: f64,
    pub latency_quality: f64,
    pub party_quality: f64,
    pub wait_cost: f64,
    pub utilization: f64,
}
impl Default for ObjectiveWeights {
    fn default() -> Self {
        Self {
            skill_quality: 1.0,
            role_quality: 0.5,
            latency_quality: 0.3,
            party_quality: 0.2,
            wait_cost: 0.8,
            utilization: 0.0,
        }
    }
}
/// A soft objective that can be scored.
pub trait SoftObjective: Send + Sync {
    fn score(&self, match_: &ProposedMatch, world: &World) -> f64;
    fn description(&self) -> String;
}
/// Skill balance soft objective: minimize rating difference.
pub struct SkillBalanceObjective;
impl SoftObjective for SkillBalanceObjective {
    fn score(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        let avg_a = average_rating(&proposed.team_a, world);
        let avg_b = average_rating(&proposed.team_b, world);
        let diff = (avg_a - avg_b).abs();
        1.0 - (diff / 400.0).min(1.0)
    }
    fn description(&self) -> String {
        "skill balance".to_string()
    }
}
/// Latency soft objective: minimize latency.
pub struct LatencyObjective;
impl SoftObjective for LatencyObjective {
    fn score(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        let all: Vec<PlayerId> = proposed
            .team_a
            .iter()
            .chain(&proposed.team_b)
            .copied()
            .collect();
        let avg_latency: f64 = all
            .iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|obs| obs.rating)
            .sum::<f64>()
            / all.len().max(1) as f64;
        1.0 - (avg_latency / 200.0).min(1.0)
    }
    fn description(&self) -> String {
        "latency".to_string()
    }
}
/// Wait time soft objective: minimize queue wait.
pub struct WaitTimeObjective;
impl SoftObjective for WaitTimeObjective {
    fn score(&self, _proposed: &ProposedMatch, _world: &World) -> f64 {
        0.5
    }
    fn description(&self) -> String {
        "wait time".to_string()
    }
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
