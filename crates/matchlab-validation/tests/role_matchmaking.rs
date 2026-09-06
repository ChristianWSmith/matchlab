//! Role-aware matchmaking baselines (ticket T-08).
//!
//! Team A must be filled exclusively from queue entries whose `role` matches
//! `teams.a.role` when that role is set, and team B likewise; an entry whose
//! role matches neither side waits. With roles unset each side accepts any
//! queued player (counts-only behavior preserved byte-for-byte by batch).

use matchlab_core::match_::TeamComposition;
use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_matchmaking::objective::MatchObjective;
use matchlab_matchmaking::queue::{Queue, QueueEntry};
use matchlab_matchmaking::search::{
    BeamSearch, GreedySearch, RandomSamplingSearch, SearchStrategy,
};
use matchlab_validation::{observation, queue_entry};

fn world_with(ratings: &[(u64, f64)]) -> World {
    let mut world = World::new(SimRng::from_seed(1));
    for &(id, rating) in ratings {
        world
            .observations
            .insert(PlayerId(id), observation(id, rating));
    }
    world
}

fn role_entry(id: u64, joined_secs: f64, rating: f64, role: Option<&str>) -> QueueEntry {
    let mut e = queue_entry(id, SimTime::from_secs(joined_secs), rating);
    e.role = role.map(|s| s.to_string());
    e
}

fn sym(n: usize) -> TeamComposition {
    TeamComposition {
        team_size_a: n,
        team_size_b: n,
        role_a: None,
        role_b: None,
    }
}

fn batch() -> LuaMatchmaker {
    LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap()
}

#[test]
fn batch_1v4_fills_teams_by_role() {
    // One killer + four survivors must form a single 1v4 proposal with the
    // killer on team A and every survivor on team B — no cross-role borrowing.
    let mut queue = Queue::default();
    queue.enqueue(role_entry(1, 1.0, 1000.0, Some("killer")));
    queue.enqueue(role_entry(2, 2.0, 1000.0, Some("survivor")));
    queue.enqueue(role_entry(3, 3.0, 1000.0, Some("survivor")));
    queue.enqueue(role_entry(4, 4.0, 1000.0, Some("survivor")));
    queue.enqueue(role_entry(5, 5.0, 1000.0, Some("survivor")));
    let world = world_with(&[
        (1, 1000.0),
        (2, 1000.0),
        (3, 1000.0),
        (4, 1000.0),
        (5, 1000.0),
    ]);
    let teams = TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    };
    let mm = batch();
    let mut rng = SimRng::from_seed(7);
    let matches = mm.find_matches(&queue, &world, &teams, SimTime::ZERO, &mut rng);
    assert_eq!(matches.len(), 1, "one killer + four survivors → one match");
    assert_eq!(matches[0].team_a, vec![PlayerId(1)], "team A is the killer");
    assert_eq!(
        matches[0].team_b,
        vec![PlayerId(2), PlayerId(3), PlayerId(4), PlayerId(5)]
    );
}

#[test]
fn batch_roles_unset_is_byte_identical_regression() {
    // Pre-T-08 contract: unset roles ⇒ the single-queue rating alternation.
    // The exact team_a/team_b id arrays are pinned here, so any drift in the
    // role-unset path fails loudly.
    let mut queue = Queue::default();
    for (id, t, rating) in [
        (1, 100.0, 800.0),
        (2, 90.0, 900.0),
        (3, 80.0, 1000.0),
        (4, 70.0, 1100.0),
        (5, 60.0, 1200.0),
        (6, 50.0, 1300.0),
        (7, 40.0, 1400.0),
        (8, 30.0, 1500.0),
        (9, 20.0, 1600.0),
        (10, 10.0, 1700.0),
    ] {
        queue.enqueue(role_entry(id, t, rating, None));
    }
    let world = world_with(&[
        (1, 800.0),
        (2, 900.0),
        (3, 1000.0),
        (4, 1100.0),
        (5, 1200.0),
        (6, 1300.0),
        (7, 1400.0),
        (8, 1500.0),
        (9, 1600.0),
        (10, 1700.0),
    ]);
    let mm = batch();
    let mut rng = SimRng::from_seed(13);
    let matches = mm.find_matches(&queue, &world, &sym(5), SimTime::ZERO, &mut rng);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].team_a,
        vec![
            PlayerId(1),
            PlayerId(3),
            PlayerId(5),
            PlayerId(7),
            PlayerId(9)
        ]
    );
    assert_eq!(
        matches[0].team_b,
        vec![
            PlayerId(2),
            PlayerId(4),
            PlayerId(6),
            PlayerId(8),
            PlayerId(10)
        ]
    );
}

#[test]
fn batch_short_pool_stalls_instead_of_borrowing() {
    // 2 killers + 1 survivor can never fill a 1v4 → nothing is formed (the
    // survivors wait for more killers; killers wait for more survivors).
    let mut queue = Queue::default();
    queue.enqueue(role_entry(1, 1.0, 1000.0, Some("killer")));
    queue.enqueue(role_entry(2, 2.0, 1000.0, Some("killer")));
    queue.enqueue(role_entry(3, 3.0, 1000.0, Some("survivor")));
    let world = world_with(&[(1, 1000.0), (2, 1000.0), (3, 1000.0)]);
    let teams = TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    };
    let mm = batch();
    let mut rng = SimRng::from_seed(7);
    let matches = mm.find_matches(&queue, &world, &teams, SimTime::ZERO, &mut rng);
    assert!(
        matches.is_empty(),
        "short pools must stall, not borrow across roles"
    );
}

#[test]
fn batch_role_filter_negative() {
    // Only survivors queued, but team A requires a killer → no match, even
    // though team B could be filled. Proves the killer filter is not
    // short-circuited by any other eligible pool.
    let mut queue = Queue::default();
    for id in 1..=4u64 {
        queue.enqueue(role_entry(id, id as f64, 1000.0, Some("survivor")));
    }
    let world = world_with(&(1..=4u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());
    let teams = TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    };
    let mm = batch();
    let mut rng = SimRng::from_seed(7);
    let matches = mm.find_matches(&queue, &world, &teams, SimTime::ZERO, &mut rng);
    assert!(matches.is_empty(), "team A role unmatched ⇒ no match");
}

#[test]
fn batch_role_formation_is_deterministic() {
    let mut queue = Queue::default();
    for id in 1..=4u64 {
        queue.enqueue(role_entry(id, id as f64, 1000.0, Some("survivor")));
    }
    queue.enqueue(role_entry(5, 5.0, 1000.0, Some("killer")));
    let world = world_with(&(1..=5u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());
    let teams = TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    };
    let mm = batch();

    let mut rng = SimRng::from_seed(99);
    let first = mm.find_matches(&queue, &world, &teams, SimTime::ZERO, &mut rng);
    let mut rng = SimRng::from_seed(99);
    let second = mm.find_matches(&queue, &world, &teams, SimTime::ZERO, &mut rng);
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.team_a, b.team_a);
        assert_eq!(a.team_b, b.team_b);
        assert_eq!(a.quality_score, b.quality_score);
    }
}

#[test]
fn strict_and_expanding_respect_roles() {
    // A 1v1 game with killer/survivor roles: the survivor awaiting a killer
    // forms nothing with strict (any diff) — the role gate applies even when
    // the skill window would accept.
    let strict = LuaMatchmaker::load(
        "plugins/matchmaking/strict.lua",
        &serde_yaml::from_str("max_skill_diff: 400.0").unwrap(),
    )
    .unwrap();
    let expanding = LuaMatchmaker::load(
        "plugins/matchmaking/expanding_window.lua",
        &serde_yaml::from_str("").unwrap(),
    )
    .unwrap();

    let mut queue = Queue::default();
    queue.enqueue(role_entry(1, 0.0, 1000.0, Some("survivor")));
    queue.enqueue(role_entry(2, 0.0, 1001.0, Some("survivor")));
    let world = world_with(&[(1, 1000.0), (2, 1001.0)]);
    let teams = TeamComposition {
        team_size_a: 1,
        team_size_b: 1,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    };

    let mut rng = SimRng::from_seed(1);
    let strict_matches = strict.find_matches(&queue, &world, &teams, SimTime::ZERO, &mut rng);
    assert!(
        strict_matches.is_empty(),
        "strict: no killer in queue ⇒ no match even within max_skill_diff"
    );

    let mut rng = SimRng::from_seed(1);
    let expanding_matches =
        expanding.find_matches(&queue, &world, &teams, SimTime::from_secs(60.0), &mut rng);
    assert!(
        expanding_matches.is_empty(),
        "expanding: 60s wait widens the window but the killer role is still empty"
    );
}

#[test]
fn search_strategies_preserve_equal_size_behavior() {
    // All three public (non-wired) strategies must accept `TeamComposition`
    // and reproduce the legacy equal-size `team_size` behavior.
    let ratings = vec![(1, 1000.0), (2, 1000.0), (3, 1000.0), (4, 1000.0)];
    let queue: Vec<QueueEntry> = ratings
        .iter()
        .map(|(id, r)| role_entry(*id, *id as f64, *r, None))
        .collect();
    let world = world_with(&ratings);
    let objective = MatchObjective::new(1.0, 0.5, 0.0, 0.1);
    let comp = sym(2);

    let mut rng = SimRng::from_seed(7);
    let greedy = GreedySearch.search(&queue, &objective, &comp, &world, &mut rng);
    assert_eq!(greedy.len(), 1);
    assert_eq!(greedy[0].team_a.len(), 2);
    assert_eq!(greedy[0].team_b.len(), 2);

    let mut rng = SimRng::from_seed(7);
    let sampling =
        RandomSamplingSearch { samples: 4 }.search(&queue, &objective, &comp, &world, &mut rng);
    assert!(!sampling.is_empty());
    assert_eq!(sampling[0].team_a.len() + sampling[0].team_b.len(), 4);

    let mut rng = SimRng::from_seed(7);
    let beam = BeamSearch { width: 8 }.search(&queue, &objective, &comp, &world, &mut rng);
    assert!(!beam.is_empty());
    assert_eq!(beam[0].team_a.len(), 2);
    assert_eq!(beam[0].team_b.len(), 2);
}
