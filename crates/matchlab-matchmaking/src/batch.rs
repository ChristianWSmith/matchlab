use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;

use crate::constraint::Constraint;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

/// v0.1 matchmaker: process the whole queue periodically, FIFO by join time,
/// filling team A then team B in consecutive blocks of `2 × team_size`.
///
/// `interval_ticks` is metadata the event handler (Ticket 08) reads to decide
/// *when* to trigger matchmaking; the handler issues the `MatchFormed` events.
pub struct BatchMatchmaker {
    pub interval_ticks: u64,
    pub constraints: Vec<Box<dyn Constraint>>,
}

impl BatchMatchmaker {
    pub fn new(interval_ticks: u64) -> Self {
        Self {
            interval_ticks,
            constraints: Vec::new(),
        }
    }
}

impl Matchmaker for BatchMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        _now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut candidates: Vec<_> = queue.entries().iter().collect();
        candidates.sort_by_key(|a| a.joined_at);

        let mut matches: Vec<ProposedMatch> = Vec::new();
        let mut team_a: Vec<PlayerId> = Vec::new();
        let mut team_b: Vec<PlayerId> = Vec::new();

        let mut emit = |team_a: &mut Vec<PlayerId>, team_b: &mut Vec<PlayerId>| {
            if team_a.len() != team_size || team_b.len() != team_size {
                return;
            }
            let a = std::mem::take(team_a);
            let b = std::mem::take(team_b);
            let quality = ProposedMatch::match_quality(&a, &b, world);
            let proposed = ProposedMatch {
                team_a: a,
                team_b: b,
                quality_score: quality,
            };
            if self
                .constraints
                .iter()
                .all(|c| c.is_satisfied(&proposed, world))
            {
                matches.push(proposed);
            }
        };

        for entry in candidates {
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

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{
        DetectionFlag, PlayerObservation, Region, SkillVector, VisibleRank,
    };
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

    fn entry(id: u64, joined_at: SimTime, rating: f64) -> crate::queue::QueueEntry {
        crate::queue::QueueEntry {
            player_id: PlayerId(id),
            joined_at,
            observation: obs(id, rating),
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".to_string(),
            role: None,
            latency_ms: 30.0,
        }
    }

    fn build_world(ratings: &[(u64, f64)]) -> World {
        let mut world = World::new(SimRng::from_seed(42));
        for &(id, rating) in ratings {
            world.observations.insert(PlayerId(id), obs(id, rating));
        }
        world
    }

    #[test]
    fn ten_players_team_size_five_forms_one_match_fifo() {
        let mut queue = Queue::default();
        // Join times deliberately out of enqueue order to prove FIFO sorting.
        for (id, t) in [
            (10, 100.0),
            (9, 90.0),
            (8, 80.0),
            (7, 70.0),
            (6, 60.0),
            (5, 50.0),
            (4, 40.0),
            (3, 30.0),
            (2, 20.0),
            (1, 10.0),
        ] {
            queue.enqueue(entry(id, SimTime::from_secs(t), 1000.0));
        }
        let world = build_world(&[
            (1, 1000.0),
            (2, 1000.0),
            (3, 1000.0),
            (4, 1000.0),
            (5, 1000.0),
            (6, 1000.0),
            (7, 1000.0),
            (8, 1000.0),
            (9, 1000.0),
            (10, 1000.0),
        ]);

        let mm = BatchMatchmaker::new(1);
        let mut rng = SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);

        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        // FIFO: the five longest-waiting players (1..5) form team A.
        assert_eq!(
            m.team_a,
            vec![
                PlayerId(1),
                PlayerId(2),
                PlayerId(3),
                PlayerId(4),
                PlayerId(5)
            ]
        );
        assert_eq!(
            m.team_b,
            vec![
                PlayerId(6),
                PlayerId(7),
                PlayerId(8),
                PlayerId(9),
                PlayerId(10)
            ]
        );
        assert_eq!(m.quality_score, 1.0);
    }

    #[test]
    fn fewer_than_twice_team_size_yields_no_matches() {
        let mut queue = Queue::default();
        for id in 1..=9u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0));
        }
        let world = build_world(&[
            (1, 1000.0),
            (2, 1000.0),
            (3, 1000.0),
            (4, 1000.0),
            (5, 1000.0),
            (6, 1000.0),
            (7, 1000.0),
            (8, 1000.0),
            (9, 1000.0),
        ]);

        let mm = BatchMatchmaker::new(1);
        let mut rng = SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert!(matches.is_empty());
    }

    #[test]
    fn multiple_blocks_form_multiple_matches() {
        let mut queue = Queue::default();
        for id in 1..=20u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0));
        }
        let world = build_world(&(1..=20u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());

        let mm = BatchMatchmaker::new(1);
        let mut rng = SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);

        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches[0].team_a,
            vec![
                PlayerId(1),
                PlayerId(2),
                PlayerId(3),
                PlayerId(4),
                PlayerId(5)
            ]
        );
        assert_eq!(
            matches[1].team_b,
            vec![
                PlayerId(16),
                PlayerId(17),
                PlayerId(18),
                PlayerId(19),
                PlayerId(20)
            ]
        );
    }

    #[test]
    fn match_quality_reflects_rating_balance() {
        let mut queue = Queue::default();
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0));
        }
        let world = build_world(&[
            (1, 1500.0),
            (2, 1500.0),
            (3, 1500.0),
            (4, 1500.0),
            (5, 1500.0),
            (6, 900.0),
            (7, 900.0),
            (8, 900.0),
            (9, 900.0),
            (10, 900.0),
        ]);

        let mm = BatchMatchmaker::new(1);
        let mut rng = SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);

        assert_eq!(matches.len(), 1);
        // avg_a=1500 vs avg_b=900 → diff=600 → clamped to 1.0 → quality 0.0
        assert_eq!(matches[0].quality_score, 0.0);
    }

    #[test]
    fn deterministic_given_queue_and_seed() {
        let mut queue = Queue::default();
        for id in 1..=10u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0));
        }
        let world = build_world(&(1..=10u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());

        let mm = BatchMatchmaker::new(1);
        let mut rng_a = SimRng::from_seed(99);
        let mut rng_b = SimRng::from_seed(12345);
        let a = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng_a);
        let b = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng_b);

        assert_eq!(a.len(), b.len());
        for (ma, mb) in a.iter().zip(b.iter()) {
            assert_eq!(ma.team_a, mb.team_a);
            assert_eq!(ma.team_b, mb.team_b);
            assert_eq!(ma.quality_score, mb.quality_score);
        }
    }
}
