//! Queue wait-time analytical baselines (ticket T-03).
//!
//! `Queue::waiting_time` must be exactly `now − joined_at` in tick
//! arithmetic (saturating at `SimTime::ZERO` when `now < joined_at`), and a
//! deterministic arrival tape must reproduce the analytic per-match wait.

use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_matchmaking::queue::Queue;
use matchlab_validation::{observation, queue_entry};

const NANOS_PER_SEC: u64 = 1_000_000_000;

fn q_with(entries: &[(u64, SimTime)]) -> Queue {
    let mut q = Queue::default();
    for &(id, joined_at) in entries {
        q.enqueue(queue_entry(id, joined_at, 1000.0));
    }
    q
}

#[test]
fn saturated_wait_is_exact_ticks() {
    let q = q_with(&[
        (1, SimTime::from_secs(10.0)),
        (2, SimTime::from_secs(2.5)),
        (3, SimTime::from_secs(0.0)),
    ]);
    // Exact integer-nanosecond waits (10s, 27.5s, 30s — all exactly
    // representable).
    let now = SimTime::from_secs(30.0);
    assert_eq!(
        q.waiting_time(PlayerId(1), now).unwrap().ticks(),
        20 * NANOS_PER_SEC
    );
    assert_eq!(
        q.waiting_time(PlayerId(2), now).unwrap().ticks(),
        27_500_000_000
    );
    assert_eq!(
        q.waiting_time(PlayerId(3), now).unwrap().ticks(),
        30 * NANOS_PER_SEC
    );
}

#[test]
fn waiting_time_saturates_when_now_precedes_joined_at() {
    let q = q_with(&[(1, SimTime::from_secs(100.0))]);
    // `now` strictly before `joined_at` must not wrap: the wait is zero.
    assert_eq!(
        q.waiting_time(PlayerId(1), SimTime::from_secs(0.0))
            .unwrap(),
        SimTime::ZERO
    );
    // Equal timestamps also read zero (nothing elapsed).
    assert_eq!(
        q.waiting_time(PlayerId(1), SimTime::from_secs(100.0))
            .unwrap(),
        SimTime::ZERO
    );
}

/// Drive the batch matchmaker over a fixed-interval arrival tape:
/// player `i` joins at exactly `i * Δt`; the batch matchmaker (team_size 1)
/// forms the next match immediately whenever 2 players are queued. Analytic
/// result: match `k` pairs the two oldest players and every formed match's
/// max wait is exactly `Δt`.
#[test]
fn deterministic_arrival_tape_produces_analytic_waits() {
    let mm =
        LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap();
    let dt_secs = 30.0;
    let dt_ticks = (dt_secs * 1e9) as u64;
    let players = 10u64;

    let mut queue = Queue::default();
    let mut world = World::new(SimRng::from_seed(0));
    let mut rng = SimRng::from_seed(1);
    let mut waits: Vec<u64> = Vec::new();
    let mut pairs: Vec<(u64, u64)> = Vec::new();

    for i in 0..players {
        let arrive = SimTime::from_secs(i as f64 * dt_secs);
        queue.enqueue(queue_entry(i, arrive, 1000.0));
        world
            .observations
            .insert(PlayerId(i), observation(i, 1000.0));

        let matches = mm.find_matches(&queue, &world, 1, arrive, &mut rng);
        assert!(matches.len() <= 1, "at most one 1v1 match per cadence");
        for m in matches {
            let ids: Vec<u64> = m
                .team_a
                .iter()
                .chain(m.team_b.iter())
                .map(|p| p.0)
                .collect();
            assert_eq!(ids.len(), 2);
            pairs.push((ids[0], ids[1]));
            let max_wait = ids
                .iter()
                .filter_map(|pid| queue.waiting_time(PlayerId(*pid), arrive))
                .map(|t| t.ticks())
                .max()
                .unwrap();
            waits.push(max_wait);
            queue.remove_batch(
                &m.team_a
                    .iter()
                    .chain(&m.team_b)
                    .copied()
                    .collect::<Vec<_>>(),
            );
        }
    }

    // Exactly one match per two arrivals, pairing oldest-unmatched players.
    assert_eq!(pairs.len(), players as usize / 2);
    for (k, (a, b)) in pairs.iter().enumerate() {
        assert_eq!(*a, (2 * k) as u64, "match {k} older partner");
        assert_eq!(*b, (2 * k + 1) as u64, "match {k} newer partner");
    }
    // Analytic wait for every formed match: Δt exactly, in ticks.
    assert!(!waits.is_empty());
    for (k, &w) in waits.iter().enumerate() {
        assert_eq!(w, dt_ticks, "match {k} wait must be exactly Δt");
    }
}
