use matchlab_core::event::{Event, MatchEndEvent, PlayerQueueEvent};
use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{
    PlayerId, PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
};
use matchlab_core::rng::{SimRng, StreamSeeds};
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use matchlab_matchmaking::queue::{Queue, QueueEntry};
use std::collections::HashMap;
use std::time::Instant;

const ITERS: u64 = 10_000;

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
        recent_performances: vec![0.5, 0.6, 0.4, 0.7, 0.3],
        queue_joined_at: None,
        is_online: true,
        party_id: None,
        tilt_level: 0.0,
        game_mode: "ranked".into(),
        skill_vector: SkillVector::one_dimensional(rating),
        detection_flags: Vec::new(),
        role: None,
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
        region: Region::NA,
        location: matchlab_core::player::GeoLocation::new(Region::NA, 0.0, 0.0),
        account_age: 0,
        games_played: 0,
        fatigue: 0.0,
        tilt: 0.0,
        experience: 0,
        is_online: true,
        archetype: "stable".into(),
        role: None,
    }
}

fn sample_match_result(n: usize) -> MatchResult {
    let team_a: Vec<PlayerId> = (1..=n as u64).map(PlayerId).collect();
    let team_b: Vec<PlayerId> = ((n as u64 + 1)..=2 * n as u64).map(PlayerId).collect();
    let perfs: Vec<PlayerPerformance> = (1..=2 * n as u64)
        .map(|id| PlayerPerformance {
            player_id: PlayerId(id),
            stats: {
                let mut s = std::collections::HashMap::new();
                s.insert("kills".to_string(), id as f64 * 2.0);
                s.insert("deaths".to_string(), id as f64);
                s.insert("assists".to_string(), id as f64 * 0.5);
                s.insert("impact".to_string(), 0.5 + id as f64 * 0.05);
                s
            },
            variance: 0.1,
        })
        .collect();
    MatchResult {
        match_id: MatchId(1),
        winner: Team::A,
        team_a,
        team_b,
        team_a_score: 13.0,
        team_b_score: 5.0,
        player_performances: perfs,
        duration: SimTime::from_secs(1800.0),
        disconnected: false,
        forfeited: false,
        variance: 0.05,
        unexpected_events: Vec::new(),
    }
}

fn bench<F: FnMut()>(name: &str, iters: u64, mut f: F) {
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_secs_f64() / iters as f64 * 1e9;
    eprintln!("{name}: {iters} iters in {elapsed:.2?} ({per_op:.0} ns/op)");
}

#[test]
fn bench_lua_vm_creation() {
    let params =
        serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0").unwrap();
    bench("lua_vm_creation", 100, || {
        let _vm = LuaVm::load_with_plugin_dir(
            "plugins/rating/elo.lua",
            &params,
            &["initialize", "predict", "update"],
            "plugins/rating",
        )
        .unwrap();
    });
}

#[test]
fn bench_lua_call_overhead() {
    let params =
        serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0").unwrap();
    let vm = LuaVm::load_with_plugin_dir(
        "plugins/rating/elo.lua",
        &params,
        &["initialize", "predict", "update"],
        "plugins/rating",
    )
    .unwrap();
    let mut rng = SimRng::from_seed(42);
    for _ in 0..100 {
        vm.with_rng(&mut rng, |vm| {
            let data = vm
                .with_lua(|lua| {
                    let t = lua.create_table().unwrap();
                    t.set("player_id", 1u64).unwrap();
                    Ok(mlua::Value::Table(t))
                })
                .unwrap();
            let _: mlua::Table = vm.call_init("initialize", &[data]).unwrap();
        });
    }
    bench("lua_call_overhead", ITERS, || {
        vm.with_rng(&mut rng, |vm| {
            let data = vm
                .with_lua(|lua| {
                    let t = lua.create_table().unwrap();
                    t.set("player_id", 1u64).unwrap();
                    Ok(mlua::Value::Table(t))
                })
                .unwrap();
            let _: mlua::Table = vm.call_init("initialize", &[data]).unwrap();
        });
    });
}

#[test]
fn bench_observation_to_table() {
    let lua = mlua::Lua::new();
    let o = obs(1, 1500.0);
    for _ in 0..100 {
        let _ = convert::observation_to_table(&lua, &o).unwrap();
    }
    bench("observation_to_table", ITERS, || {
        let _ = convert::observation_to_table(&lua, &o).unwrap();
    });
}

#[test]
fn bench_match_result_to_table() {
    let lua = mlua::Lua::new();
    let mr = sample_match_result(5);
    for _ in 0..100 {
        let _ = convert::match_result_to_table(&lua, &mr).unwrap();
    }
    bench("match_result_to_table (5v5)", ITERS, || {
        let _ = convert::match_result_to_table(&lua, &mr).unwrap();
    });
}

#[test]
fn bench_observations_to_map() {
    let lua = mlua::Lua::new();
    let obs_list: Vec<PlayerObservation> = (1..=10)
        .map(|id| obs(id, 1000.0 + id as f64 * 50.0))
        .collect();
    for _ in 0..100 {
        let _ = convert::observations_to_map(&lua, &obs_list).unwrap();
    }
    bench("observations_to_map (10 obs)", ITERS, || {
        let _ = convert::observations_to_map(&lua, &obs_list).unwrap();
    });
}

#[test]
fn bench_metric_snapshot() {
    let lua = mlua::Lua::new();
    let mut world = World::new(SimRng::from_seed(1));
    for id in 1..=10u64 {
        world.add_player(reality(id, 1500.0), obs(id, 1200.0));
    }
    let mr = sample_match_result(5);
    let mr_table = convert::match_result_to_table(&lua, &mr).unwrap();
    let mr_val = mlua::Value::Table(mr_table);
    for _ in 0..100 {
        let _ = convert::metric_snapshot_with_table(&lua, mr_val.clone(), &mr, &world).unwrap();
    }
    bench("metric_snapshot_with_table (10 players)", ITERS, || {
        let _ = convert::metric_snapshot_with_table(&lua, mr_val.clone(), &mr, &world).unwrap();
    });
}

#[test]
fn bench_queue_operations() {
    let entries: Vec<QueueEntry> = (0..1000)
        .map(|id| QueueEntry {
            player_id: PlayerId(id),
            joined_at: SimTime::ZERO,
            observation: matchlab_matchmaking::queue::LeanObservation {
                rating: 1000.0,
                rating_deviation: 350.0,
                games_played: 0,
                win_rate: 0.5,
            },
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".into(),
            role: None,
            latency_ms: 30.0,
        })
        .collect();

    bench("queue_enqueue (1000 entries × 100)", 100, || {
        let mut q = Queue::default();
        for e in &entries {
            q.enqueue(e.clone());
        }
    });

    let remove_ids: Vec<PlayerId> = (0..500).map(PlayerId).collect();
    let mut q = Queue::default();
    for e in &entries {
        q.enqueue(e.clone());
    }
    bench("queue_remove_batch (500 from 1000 × 100)", 100, || {
        q.remove_batch(&remove_ids);
    });
}

#[test]
fn bench_skill_vector() {
    let sv = SkillVector::one_dimensional(1500.0);
    bench("skill_vector::overall (1D, 1M)", 1_000_000, || {
        let _ = sv.overall();
    });
    bench("skill_vector::clone (1D, 1M)", 1_000_000, || {
        let _ = sv.clone();
    });
}

#[test]
fn bench_event_allocation() {
    let iters = 100_000u64;
    bench("event_alloc_box_dyn (100K)", iters, || {
        let evt: Box<dyn Event> = Box::new(MatchEndEvent {
            time: SimTime::ZERO,
            match_id: MatchId(1),
        });
        std::hint::black_box(evt);
    });
    bench("event_alloc_box_concrete (100K)", iters, || {
        let evt = Box::new(MatchEndEvent {
            time: SimTime::ZERO,
            match_id: MatchId(1),
        });
        std::hint::black_box(evt);
    });
    bench("event_alloc_player_queue_dyn (100K)", iters, || {
        let evt: Box<dyn Event> = Box::new(PlayerQueueEvent {
            time: SimTime::ZERO,
            player_id: PlayerId(1),
        });
        std::hint::black_box(evt);
    });
}

#[test]
fn bench_clone_costs() {
    let o = obs(1, 1500.0);
    bench("observation_clone (10K)", ITERS, || {
        let _ = o.clone();
    });
    let r = reality(1, 1500.0);
    bench("reality_clone (10K)", ITERS, || {
        let _ = r.clone();
    });
    let e = QueueEntry {
        player_id: PlayerId(1),
        joined_at: SimTime::ZERO,
        observation: matchlab_matchmaking::queue::LeanObservation {
            rating: 1000.0,
            rating_deviation: 350.0,
            games_played: 0,
            win_rate: 0.5,
        },
        region: Region::NA,
        party_id: None,
        game_mode: "ranked".into(),
        role: None,
        latency_ms: 30.0,
    };
    bench("queue_entry_clone (10K)", ITERS, || {
        let _ = e.clone();
    });
}

#[test]
fn bench_hashmap_lookup() {
    let n = 100u64;
    let map: HashMap<PlayerId, PlayerObservation> =
        (0..n).map(|id| (PlayerId(id), obs(id, 1000.0))).collect();
    let keys: Vec<PlayerId> = (0..n).map(PlayerId).collect();
    let rounds = 1000u64;
    let total = n * rounds;
    let mut acc = 0u64;
    bench(
        &format!("hashmap_lookup ({n} keys x {rounds})"),
        total,
        || {
            for k in &keys {
                if map.contains_key(k) {
                    acc += 1;
                }
            }
        },
    );
    std::hint::black_box(acc);
}

#[test]
fn bench_lua_full_rating_update() {
    use matchlab_rating::lua::LuaRatingSystem;
    use matchlab_rating::system::RatingSystem;
    let sys = LuaRatingSystem::load(
        "plugins/rating/elo.lua",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0").unwrap(),
    )
    .unwrap();
    let mr = sample_match_result(5);
    let obs_map: HashMap<PlayerId, PlayerObservation> = (1..=10u64)
        .map(|id| (PlayerId(id), obs(id, 1000.0 + id as f64 * 50.0)))
        .collect();
    for _ in 0..100 {
        let _ = sys.update(&mr, &obs_map);
    }
    bench("lua_full_rating_update (10 players, 5K)", 5000, || {
        let _ = sys.update(&mr, &obs_map);
    });
}

#[test]
fn bench_full_simulation_loop() {
    use matchlab_core::match_::TeamComposition;
    use matchlab_loop::MatchLoop;
    use matchlab_loop::machine::LoopConfig;
    use matchlab_lua::GlobalSubscription;
    use matchlab_metrics::MetricsEngine;
    use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
    use matchlab_players::population::{PopulationConfig, PopulationGenerator};

    let archetype = ArchetypeConfig {
        name: "stable".into(),
        proportion: 1.0,
        skill_distribution: DistributionConfig::Normal {
            mean: 1000.0,
            stddev: 150.0,
        },
        skill_volatility: 5.0,
        improvement_rate: 0.0,
        play_frequency: 0.8,
        session_length: 1800.0,
        quit_probability: 0.01,
        initial_rating: None,
        role: None,
        skill_dimensions: None,
        correlation: None,
        dynamics: None,
    };
    let config = PopulationConfig {
        size: 1000,
        archetypes: vec![archetype],
    };
    let mut rng = SimRng::from_seed(42);
    let (realities, obs_list) = PopulationGenerator::generate(&config, &mut rng);
    let pop: Vec<(PlayerReality, PlayerObservation)> =
        realities.into_iter().zip(obs_list).collect();

    let params =
        serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0").unwrap();
    let rating = || {
        matchlab_rating::plugins::registry::from_script("plugins/rating/elo.lua", &params).unwrap()
    };
    let outcome_params = serde_yaml::from_str("beta: 400.0\nnoise: 0.1").unwrap();
    let outcome = || {
        Box::new(
            matchlab_game::lua::LuaOutcomeModel::load("plugins/game/logistic.lua", &outcome_params)
                .unwrap(),
        )
    };
    let matchmaker = || {
        Box::new(
            matchlab_matchmaking::lua::LuaMatchmaker::load(
                "plugins/matchmaking/batch.lua",
                &serde_yaml::Value::Null,
            )
            .unwrap(),
        )
    };

    let cfg = LoopConfig {
        teams: TeamComposition::default(),
        batch_interval_ticks: 60,
        rejoin_delay: SimTime::from_secs(30.0),
        max_matches: 500,
        skill_update_interval: None,
        stream_seeds: StreamSeeds::from_seed(1234),
        record_history: false,
    };

    let start = Instant::now();
    let mut loop_ = MatchLoop::new(
        pop.clone(),
        rating(),
        outcome(),
        matchmaker(),
        MetricsEngine::new(),
        cfg.clone(),
        GlobalSubscription::default(),
    );
    loop_.run();
    let elapsed = start.elapsed();
    let completed = loop_.state.lock().unwrap().matches_completed;
    eprintln!(
        "full_simulation_loop (cold): {completed} matches in {elapsed:.2?} ({:.1} ms/match)",
        elapsed.as_millis() as f64 / completed as f64
    );

    let start = Instant::now();
    let mut loop_ = MatchLoop::new(
        pop,
        rating(),
        outcome(),
        matchmaker(),
        MetricsEngine::new(),
        cfg,
        GlobalSubscription::default(),
    );
    loop_.run();
    let elapsed = start.elapsed();
    let completed = loop_.state.lock().unwrap().matches_completed;
    eprintln!(
        "full_simulation_loop (warm): {completed} matches in {elapsed:.2?} ({:.1} ms/match)",
        elapsed.as_millis() as f64 / completed as f64
    );
}
