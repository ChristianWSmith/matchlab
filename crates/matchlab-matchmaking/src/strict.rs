use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;

use crate::hooks::LuaHooks;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

/// Only matches players within a fixed skill difference. Outliers may wait
/// indefinitely — that is the intended "strict" behavior (quality over speed).
pub struct StrictMatchmaker {
    pub max_skill_diff: f64,
    hooks: Option<LuaHooks>,
}

impl StrictMatchmaker {
    pub fn new(max_skill_diff: f64) -> Self {
        Self {
            max_skill_diff,
            hooks: None,
        }
    }

    pub fn with_hooks(max_skill_diff: f64, hooks: LuaHooks) -> Self {
        Self {
            max_skill_diff,
            hooks: Some(hooks),
        }
    }
}

impl Matchmaker for StrictMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        _now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();
        let mut used = Vec::new();

        let max_diff = self
            .hooks
            .as_ref()
            .and_then(|h| h.call_max_skill_diff(0.0))
            .unwrap_or(self.max_skill_diff);

        for entry in queue.entries() {
            if used.contains(&entry.player_id) {
                continue;
            }
            let mut team_a = vec![entry];
            let mut team_b = Vec::new();

            for other in queue.entries() {
                if used.contains(&other.player_id) || other.player_id == entry.player_id {
                    continue;
                }
                let diff = (entry.observation.rating - other.observation.rating).abs();
                if diff <= max_diff {
                    if team_a.len() <= team_b.len() {
                        team_a.push(other);
                    } else {
                        team_b.push(other);
                    }
                }
                if team_a.len() == team_size && team_b.len() == team_size {
                    break;
                }
            }

            if team_a.len() == team_size && team_b.len() == team_size {
                let team_a_ids: Vec<_> = team_a.iter().map(|e| e.player_id).collect();
                let team_b_ids: Vec<_> = team_b.iter().map(|e| e.player_id).collect();
                used.extend(&team_a_ids);
                used.extend(&team_b_ids);
                let quality = ProposedMatch::match_quality(&team_a_ids, &team_b_ids, world);
                matches.push(ProposedMatch {
                    team_a: team_a_ids,
                    team_b: team_b_ids,
                    quality_score: quality,
                });
            }
        }

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{DetectionFlag, PlayerId, PlayerObservation, Region, SkillVector, VisibleRank};
    use std::collections::VecDeque;

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
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn entry(id: u64, joined_at: SimTime, rating: f64) -> crate::queue::QueueEntry {
        crate::queue::QueueEntry {
            player_id: PlayerId(id),
            joined_at,
            observation: obs(id, rating),
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".into(),
            role: None,
            latency_ms: 30.0,
        }
    }

    fn build_world(ratings: &[(u64, f64)]) -> World {
        let mut world = World::new(matchlab_core::rng::SimRng::from_seed(42));
        for &(id, rating) in ratings {
            world.observations.insert(PlayerId(id), obs(id, rating));
        }
        world
    }

    #[test]
    fn matches_within_max_skill_diff() {
        let mut queue = Queue::default();
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0));
        }
        let world = build_world(&(1..=10u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());

        let mm = StrictMatchmaker::new(50.0);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn rejects_matches_exceeding_max_skill_diff() {
        let mut queue = Queue::default();
        // Ratings spread 1000..1900 — 900 apart, far beyond max diff 50.
        for (id, rating) in [(1, 1000.0), (2, 1100.0), (3, 1200.0), (4, 1300.0), (5, 1400.0), (6, 1500.0), (7, 1600.0), (8, 1700.0), (9, 1800.0), (10, 1900.0)] {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), rating));
        }
        let world = build_world(&[(1, 1000.0), (2, 1100.0), (3, 1200.0), (4, 1300.0), (5, 1400.0), (6, 1500.0), (7, 1600.0), (8, 1700.0), (9, 1800.0), (10, 1900.0)]);

        let mm = StrictMatchmaker::new(50.0);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert!(matches.is_empty());
    }

    #[test]
    fn wide_diff_forms_matches() {
        let mut queue = Queue::default();
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0));
        }
        let world = build_world(&(1..=10u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());

        let mm = StrictMatchmaker::new(1000.0);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert_eq!(matches.len(), 1);
    }
}