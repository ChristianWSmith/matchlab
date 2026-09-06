//! Lua-native matchmakers.
//!
//! `LuaMatchmaker` implements the `Matchmaker` trait by delegating to a
//! script's `find_matches` function. The queue is snapshotted into a Lua table
//! (observations only — never `PlayerReality`); randomness flows through
//! `matchlab.rng_*` when a script wants it.

use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use mlua::{Lua, Table, Value};

use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

/// A matchmaker whose algorithm lives entirely in a Lua script.
pub struct LuaMatchmaker {
    vm: LuaVm,
}

impl LuaMatchmaker {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["find_matches"])?;
        Ok(Self { vm })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

/// Snapshot the queue into a Lua array of entries (observations only).
fn queue_to_table(lua: &Lua, queue: &Queue, now: SimTime) -> Result<Value, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for (i, entry) in queue.entries().iter().enumerate() {
        let row = lua.create_table().map_err(|e| e.to_string())?;
        row.set("idx", i).map_err(|e| e.to_string())?;
        row.set("player_id", entry.player_id.0)
            .map_err(|e| e.to_string())?;
        row.set("rating", entry.observation.rating)
            .map_err(|e| e.to_string())?;
        row.set("rating", entry.observation.rating)
            .map_err(|e| e.to_string())?;
        row.set("rating_deviation", entry.observation.rating_deviation)
            .map_err(|e| e.to_string())?;
        row.set("games_played", entry.observation.games_played)
            .map_err(|e| e.to_string())?;
        row.set("win_rate", entry.observation.win_rate)
            .map_err(|e| e.to_string())?;
        row.set("joined_at_secs", entry.joined_at.as_secs_f64())
            .map_err(|e| e.to_string())?;
        row.set(
            "wait_secs",
            now.duration_since(entry.joined_at).as_secs_f64(),
        )
        .map_err(|e| e.to_string())?;
        row.set("region", convert::region_str(entry.region))
            .map_err(|e| e.to_string())?;
        match entry.party_id {
            Some(pid) => row.set("party_id", pid).map_err(|e| e.to_string())?,
            None => row.set("party_id", Value::Nil).map_err(|e| e.to_string())?,
        }
        row.set("latency_ms", entry.latency_ms)
            .map_err(|e| e.to_string())?;
        row.set("game_mode", entry.game_mode.as_str())
            .map_err(|e| e.to_string())?;
        t.set(i + 1, row).map_err(|e| e.to_string())?;
    }
    Ok(Value::Table(t))
}

fn ids_from_table(t: &Table) -> Vec<PlayerId> {
    t.pairs::<mlua::Value, u64>()
        .map(|p| p.map(|(_, id)| PlayerId(id)).unwrap_or(PlayerId(0)))
        .collect()
}

impl Matchmaker for LuaMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let queue_val = self
            .vm
            .with_lua(|lua| queue_to_table(lua, queue, now))
            .expect("build queue table");

        let matches_tbl: Table = self.vm.with_rng(rng, |vm| {
            vm.call_with_context(
                "find_matches",
                &[
                    queue_val,
                    Value::Integer(team_size as i64),
                    Value::Number(now.as_secs_f64()),
                ],
            )
            .expect("matchmaker find_matches failed")
        });

        let mut matches = Vec::new();
        for pair in matches_tbl.pairs::<mlua::Value, Table>() {
            let (_, row) = pair.expect("iterate proposed matches");
            let team_a = ids_from_table(&row.get::<Table>("team_a").expect("match team_a"));
            let team_b = ids_from_table(&row.get::<Table>("team_b").expect("match team_b"));
            if team_a.is_empty() || team_b.is_empty() {
                continue;
            }
            let quality = match row.get::<mlua::Value>("quality_score") {
                Ok(Value::Number(n)) => n,
                _ => ProposedMatch::match_quality(&team_a, &team_b, world),
            };
            matches.push(ProposedMatch {
                team_a,
                team_b,
                quality_score: quality,
            });
        }
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{PlayerObservation, Region, SkillVector, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
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
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
        }
    }

    fn entry(id: u64, joined_at: SimTime, rating: f64, region: Region) -> crate::queue::QueueEntry {
        crate::queue::QueueEntry {
            player_id: PlayerId(id),
            joined_at,
            observation: obs(id, rating),
            region,
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

    fn batch() -> LuaMatchmaker {
        LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap()
    }

    #[test]
    fn batch_forms_balanced_teams() {
        let mut queue = Queue::default();
        for (id, rating) in [
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
        ] {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), rating, Region::NA));
        }
        let world = build_world(&[
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
        let mut rng = SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].quality_score >= 0.7,
            "balanced pairing quality was {}",
            matches[0].quality_score
        );
    }

    #[test]
    fn batch_fifo_tiebreak_by_join_order() {
        let mut queue = Queue::default();
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
            queue.enqueue(entry(id, SimTime::from_secs(t), 1000.0, Region::NA));
        }
        let world = build_world(&(1..=10u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());
        let mm = batch();
        let mut rng = SimRng::from_seed(7);
        let matches = mm.find_matches(&queue, &world, 5, SimTime::ZERO, &mut rng);
        assert_eq!(matches.len(), 1);
        // Longest-waiting players alternate teams (rating tie → FIFO).
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
    fn expanding_window_widens_with_wait() {
        let mm = LuaMatchmaker::load(
            "plugins/matchmaking/expanding_window.lua",
            &serde_yaml::from_str(
                "tiers: [[5.0, 25.0], [10.0, 50.0], [20.0, 100.0], [30.0, 200.0]]\nmax_window: 400.0",
            )
            .unwrap(),
        )
        .unwrap();
        // Two players 40 rating apart: at t=0 the 5s tier (25 diff) rejects
        // them; at t=30s the 30s tier (200 diff) accepts.
        let mut queue = Queue::default();
        queue.enqueue(entry(1, SimTime::from_secs(0.0), 1000.0, Region::NA));
        queue.enqueue(entry(2, SimTime::from_secs(0.0), 1040.0, Region::NA));
        let world = build_world(&[(1, 1000.0), (2, 1040.0)]);

        let mut rng = SimRng::from_seed(1);
        let early = mm.find_matches(&queue, &world, 1, SimTime::from_secs(2.0), &mut rng);
        assert!(early.is_empty(), "2s wait should use the 5s tier (25 diff)");

        let mut rng = SimRng::from_seed(1);
        let late = mm.find_matches(&queue, &world, 1, SimTime::from_secs(30.0), &mut rng);
        assert_eq!(late.len(), 1, "30s wait should match within 200 diff");
    }

    #[test]
    fn strict_rejects_outliers() {
        let mm = LuaMatchmaker::load(
            "plugins/matchmaking/strict.lua",
            &serde_yaml::from_str("max_skill_diff: 50.0").unwrap(),
        )
        .unwrap();
        let mut queue = Queue::default();
        queue.enqueue(entry(1, SimTime::from_secs(0.0), 1000.0, Region::NA));
        queue.enqueue(entry(2, SimTime::from_secs(0.0), 1100.0, Region::NA));
        let world = build_world(&[(1, 1000.0), (2, 1100.0)]);
        let mut rng = SimRng::from_seed(1);
        let matches = mm.find_matches(&queue, &world, 1, SimTime::ZERO, &mut rng);
        assert!(matches.is_empty(), "100 diff > 50 max_skill_diff");
    }

    fn random_mm() -> LuaMatchmaker {
        LuaMatchmaker::load("plugins/matchmaking/random.lua", &serde_yaml::Value::Null).unwrap()
    }

    fn fingerprint(matches: &[ProposedMatch]) -> Vec<Vec<u64>> {
        let mut out: Vec<Vec<u64>> = matches
            .iter()
            .map(|m| {
                let mut ids: Vec<u64> = m
                    .team_a
                    .iter()
                    .chain(m.team_b.iter())
                    .map(|p| p.0)
                    .collect();
                ids.sort();
                ids
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn random_forms_full_matches_deterministically() {
        let mut queue = Queue::default();
        for id in 1..=100u64 {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0, Region::NA));
        }
        let world = build_world(&(1..=100u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());
        let mm = random_mm();

        let mut rng = SimRng::from_seed(99);
        let first = mm.find_matches(&queue, &world, 2, SimTime::ZERO, &mut rng);
        let mut rng = SimRng::from_seed(99);
        let second = mm.find_matches(&queue, &world, 2, SimTime::ZERO, &mut rng);
        assert_eq!(fingerprint(&first), fingerprint(&second));

        // Every player used exactly once: 100 players / (2*2 per match) = 25.
        assert_eq!(first.len(), 25);
        let mut ids: Vec<u64> = first
            .iter()
            .flat_map(|m| m.team_a.iter().chain(m.team_b.iter()))
            .map(|p| p.0)
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 100);

        // A different seed draws a different composition.
        let mut rng = SimRng::from_seed(100);
        let other = mm.find_matches(&queue, &world, 2, SimTime::ZERO, &mut rng);
        assert_ne!(fingerprint(&first), fingerprint(&other));
    }

    #[test]
    fn hub_spoke_partitions_by_region() {
        let mm = LuaMatchmaker::load(
            "plugins/matchmaking/hub_spoke.lua",
            &serde_yaml::from_str("spoke_capacity: 100").unwrap(),
        )
        .unwrap();
        let mut queue = Queue::default();
        for (id, region) in [
            (1, Region::NA),
            (2, Region::NA),
            (3, Region::EU),
            (4, Region::EU),
        ] {
            queue.enqueue(entry(id, SimTime::from_secs(id as f64), 1000.0, region));
        }
        let world = build_world(&[(1, 1000.0), (2, 1000.0), (3, 1000.0), (4, 1000.0)]);
        let mut rng = SimRng::from_seed(1);
        let matches = mm.find_matches(&queue, &world, 1, SimTime::ZERO, &mut rng);
        // Two matches: one NA (1v2), one EU (3v4) — same-region players kept
        // together.
        assert_eq!(matches.len(), 2);
        let mut pairs: Vec<Vec<u64>> = matches
            .iter()
            .map(|m| {
                let mut ids: Vec<u64> = m
                    .team_a
                    .iter()
                    .chain(m.team_b.iter())
                    .map(|p| p.0)
                    .collect();
                ids.sort();
                ids
            })
            .collect();
        pairs.sort();
        assert_eq!(pairs, vec![vec![1, 2], vec![3, 4]]);
    }
}
