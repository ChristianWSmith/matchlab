use matchlab_core::player::Region;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use std::collections::HashMap;

use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::{Queue, QueueEntry};

/// Decomposes matchmaking into a hub (global orchestrator) and spokes
/// (regional matchmakers). The hub distributes overflow workloads to spokes
/// and rebalances when a spoke is overloaded (spec §7.9).
pub struct HubSpokeMatchmaker {
    pub spokes: HashMap<Region, Box<dyn Matchmaker>>,
    pub spoke_capacity: usize,
}

impl HubSpokeMatchmaker {
    pub fn new(spokes: HashMap<Region, Box<dyn Matchmaker>>, spoke_capacity: usize) -> Self {
        Self {
            spokes,
            spoke_capacity,
        }
    }
}

impl Matchmaker for HubSpokeMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();

        let mut by_region: HashMap<Region, Vec<QueueEntry>> = HashMap::new();
        for entry in queue.entries() {
            by_region.entry(entry.region).or_default().push(entry.clone());
        }

        for (region, entries) in &by_region {
            if let Some(spoke) = self.spokes.get(region) {
                if entries.len() <= self.spoke_capacity {
                    let sub_queue = Queue::from_entries(entries.clone());
                    matches.extend(spoke.find_matches(&sub_queue, world, team_size, now, rng));
                } else {
                    let mut overflow: Vec<_> = entries.iter().collect();
                    overflow.sort_by(|a, b| a.joined_at.cmp(&b.joined_at));
                    let mut team_a: Vec<_> = Vec::new();
                    let mut team_b: Vec<_> = Vec::new();
                    let mut emit = |team_a: &mut Vec<_>, team_b: &mut Vec<_>| {
                        if team_a.len() < team_size || team_b.len() < team_size {
                            return;
                        }
                        let a = std::mem::take(team_a);
                        let b = std::mem::take(team_b);
                        let quality = ProposedMatch::match_quality(&a, &b, world);
                        matches.push(ProposedMatch {
                            team_a: a,
                            team_b: b,
                            quality_score: quality,
                        });
                    };
                    for entry in overflow {
                        if team_a.len() < team_size {
                            team_a.push(entry.player_id);
                        } else if team_b.len() < team_size {
                            team_b.push(entry.player_id);
                        } else {
                            emit(&mut team_a, &mut team_b);
                            team_a.push(entry.player_id);
                        }
                    }
                    emit(&mut team_a, &mut team_b);
                }
            }
        }

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{DetectionFlag, PlayerId, PlayerObservation, SkillVector, VisibleRank};
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

    fn entry(id: u64, joined_at: SimTime, rating: f64, region: Region) -> crate::queue::QueueEntry {
        crate::queue::QueueEntry {
            player_id: PlayerId(id),
            joined_at,
            observation: obs(id, rating),
            region,
            party_id: None,
            game_mode: "ranked".into(),
            role: None,
            latency_ms: 30.0,
        }
    }

    fn build_world(ids: &[u64]) -> World {
        let mut world = World::new(matchlab_core::rng::SimRng::from_seed(42));
        for &id in ids {
            world.observations.insert(PlayerId(id), obs(id, 1000.0));
        }
        world
    }

    struct SpokeStub {
        matches_per_call: usize,
    }

    impl Matchmaker for SpokeStub {
        fn find_matches(
            &self,
            _queue: &Queue,
            _world: &World,
            _team_size: usize,
            _now: SimTime,
            _rng: &mut SimRng,
        ) -> Vec<ProposedMatch> {
            (0..self.matches_per_call)
                .map(|i| {
                    let base = (i * 2) as u64;
                    ProposedMatch {
                        team_a: vec![PlayerId(base)],
                        team_b: vec![PlayerId(base + 1)],
                        quality_score: 1.0,
                    }
                })
                .collect()
        }
    }

    #[test]
    fn partitions_queue_by_region_and_delegates_to_spokes() {
        let mut queue = Queue::default();
        for id in 1..=4u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0, Region::NA));
        }
        for id in 5..=8u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0, Region::EU));
        }
        let world = build_world(&[1, 2, 3, 4, 5, 6, 7, 8]);

        let mut spokes: HashMap<Region, Box<dyn Matchmaker>> = HashMap::new();
        spokes.insert(Region::NA, Box::new(SpokeStub { matches_per_call: 1 }));
        spokes.insert(Region::EU, Box::new(SpokeStub { matches_per_call: 2 }));

        let mm = HubSpokeMatchmaker::new(spokes, 100);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn overflow_handled_by_hub_when_spoke_exceeds_capacity() {
        let mut queue = Queue::default();
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0, Region::NA));
        }
        let world = build_world(&(1..=10u64).collect::<Vec<_>>());

        let mut spokes: HashMap<Region, Box<dyn Matchmaker>> = HashMap::new();
        // Spoke stub returns 0 matches; capacity 2 means 10 entries overflow.
        spokes.insert(Region::NA, Box::new(SpokeStub { matches_per_call: 0 }));

        let mm = HubSpokeMatchmaker::new(spokes, 2);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        // 10 players / 10 per match = 1 match from the hub's greedy overflow.
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn under_capacity_delegates_to_spoke() {
        let mut queue = Queue::default();
        for id in 1..=4u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0, Region::NA));
        }
        let world = build_world(&[1, 2, 3, 4]);

        let mut spokes: HashMap<Region, Box<dyn Matchmaker>> = HashMap::new();
        spokes.insert(Region::NA, Box::new(SpokeStub { matches_per_call: 2 }));

        let mm = HubSpokeMatchmaker::new(spokes, 100);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert_eq!(matches.len(), 2);
    }
}