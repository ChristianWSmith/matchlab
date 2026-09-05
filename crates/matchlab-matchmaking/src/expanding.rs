use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;

use crate::hooks::LuaHooks;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

/// Skills matched within a window that widens the longer a player waits.
/// Stepped tiers: `[(max_secs, allowed_diff)]`; the first tier whose
/// `max_secs` is not exceeded wins, otherwise `max_window` applies.
pub struct ExpandingWindowMatchmaker {
    pub tiers: Vec<(f64, f64)>,
    pub max_window: f64,
    hooks: Option<LuaHooks>,
}

impl ExpandingWindowMatchmaker {
    pub fn default_tiers() -> Self {
        Self {
            tiers: vec![
                (5.0, 25.0),
                (10.0, 50.0),
                (20.0, 100.0),
                (30.0, 200.0),
            ],
            max_window: 400.0,
            hooks: None,
        }
    }

    pub fn with_tiers(tiers: Vec<(f64, f64)>, max_window: f64) -> Self {
        Self {
            tiers,
            max_window,
            hooks: None,
        }
    }

    pub fn with_hooks(tiers: Vec<(f64, f64)>, max_window: f64, hooks: LuaHooks) -> Self {
        Self {
            tiers,
            max_window,
            hooks: Some(hooks),
        }
    }

    fn skill_window(&self, waiting_secs: f64) -> f64 {
        if let Some(ref hooks) = self.hooks {
            if let Some(window) = hooks.call_max_skill_diff(waiting_secs) {
                return window;
            }
        }
        for &(max_secs, diff) in &self.tiers {
            if waiting_secs <= max_secs {
                return diff;
            }
        }
        self.max_window
    }
}

impl Matchmaker for ExpandingWindowMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();
        let mut used = Vec::new();

        for entry in queue.entries() {
            if used.contains(&entry.player_id) {
                continue;
            }

            let waiting_secs = now.duration_since(entry.joined_at).as_secs_f64();
            let window = self.skill_window(waiting_secs);

            let mut team_a = vec![entry];
            let mut team_b = Vec::new();

            for other in queue.entries() {
                if used.contains(&other.player_id) || other.player_id == entry.player_id {
                    continue;
                }
                let diff = (entry.observation.rating - other.observation.rating).abs();
                if diff <= window {
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
    fn skill_window_uses_stepped_tiers() {
        let mm = ExpandingWindowMatchmaker::default_tiers();
        assert_eq!(mm.skill_window(2.0), 25.0);
        assert_eq!(mm.skill_window(7.0), 50.0);
        assert_eq!(mm.skill_window(15.0), 100.0);
        assert_eq!(mm.skill_window(25.0), 200.0);
        assert_eq!(mm.skill_window(999.0), 400.0);
    }

    #[test]
    fn matches_within_window_only() {
        let mut queue = Queue::default();
        // Two clusters: 1000-rating players and 1700-rating players.
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(1.0), 1000.0));
        }
        for id in 11..=20u64 {
            queue.enqueue(entry(id, SimTime::from_secs(1.0), 1700.0));
        }
        let ratings: Vec<(u64, f64)> = (1..=20u64)
            .map(|id| (id, if id <= 10 { 1000.0 } else { 1700.0 }))
            .collect();
        let world = build_world(&ratings);

        let mm = ExpandingWindowMatchmaker::default_tiers();
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::from_secs(2.0), &mut rng);

        // Window 25 keeps the two clusters separate: each 10-player cluster
        // forms its own match, never mixing with the other cluster.
        assert_eq!(matches.len(), 2);
        for m in &matches {
            let all_a_1000 = m.team_a.iter().all(|p| p.0 <= 10);
            let all_a_1700 = m.team_a.iter().all(|p| p.0 > 10);
            let all_b_1000 = m.team_b.iter().all(|p| p.0 <= 10);
            let all_b_1700 = m.team_b.iter().all(|p| p.0 > 10);
            assert!(
                (all_a_1000 && all_b_1000) || (all_a_1700 && all_b_1700),
                "match must be homogeneous within a cluster"
            );
        }
    }

    #[test]
    fn wider_window_allows_more_matches() {
        let mut queue = Queue::default();
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(1.0), 1000.0));
        }
        let world = build_world(&(1..=10u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());

        // max_window 400 allows everyone to match.
        let mm = ExpandingWindowMatchmaker::with_tiers(vec![], 400.0);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::from_secs(2.0), &mut rng);
        assert_eq!(matches.len(), 1);
    }
}