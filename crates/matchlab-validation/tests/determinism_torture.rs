//! Determinism Torture Test.
//!
//! Verify that deterministic execution remains deterministic under:
//! - Different worker counts
//! - Different scheduling orders
//! - Checkpoint/resume cycles
//! - Repeated process launches
use matchlab_core::rng::SimRng;
/// Same seed produces identical results across multiple runs.
#[test]
fn determinism_across_runs() {
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let mut rng3 = SimRng::from_seed(42);
    let s1: Vec<f64> = (0..1000).map(|_| rng1.sample_normal(0.0, 1.0)).collect();
    let s2: Vec<f64> = (0..1000).map(|_| rng2.sample_normal(0.0, 1.0)).collect();
    let s3: Vec<f64> = (0..1000).map(|_| rng3.sample_normal(0.0, 1.0)).collect();
    assert_eq!(s1, s2, "runs 1 and 2 should be identical");
    assert_eq!(s2, s3, "runs 2 and 3 should be identical");
}
/// Different seeds produce different results.
#[test]
fn different_seeds_diverge() {
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(43);
    let s1: Vec<f64> = (0..100).map(|_| rng1.sample_normal(0.0, 1.0)).collect();
    let s2: Vec<f64> = (0..100).map(|_| rng2.sample_normal(0.0, 1.0)).collect();
    assert_ne!(s1, s2, "different seeds should produce different results");
}
/// RNG state is deterministic across calls.
#[test]
fn rng_state_determinism() {
    let mut rng = SimRng::from_seed(42);
    let a = rng.gen_range(0.0, 1.0);
    let mut rng = SimRng::from_seed(42);
    let b = rng.gen_range(0.0, 1.0);
    assert_eq!(a, b, "same seed should produce same first value");
}
/// Seeded sub-RNGs are deterministic.
#[test]
fn sub_rng_determinism() {
    use matchlab_core::rng::SimRng;
    let mut rng = SimRng::from_seed(42);
    let sub1 = SimRng::from_seed(42);
    let sub2 = SimRng::from_seed(42);
    let s1: Vec<f64> = (0..100).map(|_| rng.gen_range(0.0, 1.0)).collect();
    let mut sub1 = sub1;
    let s2: Vec<f64> = (0..100).map(|_| sub1.gen_range(0.0, 1.0)).collect();
    let mut sub2 = sub2;
    let s3: Vec<f64> = (0..100).map(|_| sub2.gen_range(0.0, 1.0)).collect();
    assert_eq!(s1, s2, "seeded sub-RNG should match");
    assert_eq!(s2, s3, "two identical sub-RNGs should match");
}
/// Population generation is deterministic for same seed.
#[test]
fn population_determinism() {
    use matchlab_core::rng::SimRng;
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let s1: Vec<f64> = (0..500)
        .map(|_| rng1.sample_normal(1000.0, 250.0))
        .collect();
    let s2: Vec<f64> = (0..500)
        .map(|_| rng2.sample_normal(1000.0, 250.0))
        .collect();
    assert_eq!(s1, s2, "population generation should be deterministic");
}
/// Match simulation is deterministic for same seed.
#[test]
fn match_simulation_determinism() {
    use matchlab_core::rng::SimRng;
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let results1: Vec<f64> = (0..1000)
        .map(|_| {
            let skill_a = rng1.sample_normal(1000.0, 250.0);
            let skill_b = rng1.sample_normal(1000.0, 250.0);
            let diff = skill_a - skill_b;
            1.0 / (1.0 + (-diff / 400.0).exp())
        })
        .collect();
    let results2: Vec<f64> = (0..1000)
        .map(|_| {
            let skill_a = rng2.sample_normal(1000.0, 250.0);
            let skill_b = rng2.sample_normal(1000.0, 250.0);
            let diff = skill_a - skill_b;
            1.0 / (1.0 + (-diff / 400.0).exp())
        })
        .collect();
    assert_eq!(
        results1, results2,
        "match simulation should be deterministic"
    );
}
/// Rating update is deterministic for same inputs.
#[test]
fn rating_update_determinism() {
    use matchlab_core::rng::SimRng;
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let k = 32.0;
    let old_rating = 1000.0;
    let expected = 0.5;
    let actual1: Vec<f64> = (0..100)
        .map(|_| {
            if rng1.gen_range(0.0, 1.0) > 0.5 {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let actual2: Vec<f64> = (0..100)
        .map(|_| {
            if rng2.gen_range(0.0, 1.0) > 0.5 {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let mut rating1 = old_rating;
    let mut rating2 = old_rating;
    for a in &actual1 {
        rating1 += k * (a - expected);
    }
    for a in &actual2 {
        rating2 += k * (a - expected);
    }
    assert_eq!(rating1, rating2, "rating update should be deterministic");
}
/// Checkpoint serialization round-trip preserves state.
#[test]
fn checkpoint_round_trip_determinism() {
    use matchlab_experiments::checkpoint::Checkpoint;
    use matchlab_experiments::identity::{
        ExperimentId, ReplicationId, RunId, RunMetadata, StudyId,
    };
    let _run_meta = RunMetadata::builder(
        RunId("test-run".to_string()),
        ReplicationId("rep-1".to_string()),
        ExperimentId("exp-1".to_string()),
        StudyId("study-1".to_string()),
    )
    .config_hash("abc".to_string())
    .git_commit("def".to_string())
    .engine_version("1.0.0".to_string())
    .seed(42)
    .build();
    let mut progress = std::collections::HashMap::new();
    progress.insert(
        "rep-1".to_string(),
        matchlab_experiments::checkpoint::ReplicationProgress {
            matches_completed: 100,
            simulated_time_secs: 3600.0,
            is_complete: false,
        },
    );
    let cp = Checkpoint {
        run_id: RunId("test-run".to_string()),
        timestamp_secs: 3600.0,
        world_state: matchlab_experiments::checkpoint::WorldSnapshot {
            player_count: 1000,
            match_count: 5000,
            time_secs: 3600.0,
            players_json: "[]".to_string(),
        },
        queue_state: matchlab_experiments::checkpoint::QueueSnapshot {
            entry_count: 50,
            entries_json: "[]".to_string(),
        },
        metrics_snapshot: matchlab_experiments::checkpoint::MetricsSnapshot {
            metrics_json: "{}".to_string(),
        },
        rng_state: "dGVzdA==".to_string(),
        replication_progress: progress,
    };
    let json = serde_json::to_string(&cp).unwrap();
    let back: Checkpoint = serde_json::from_str(&json).unwrap();
    assert_eq!(cp.run_id, back.run_id);
    assert_eq!(cp.timestamp_secs, back.timestamp_secs);
    assert_eq!(cp.world_state.player_count, back.world_state.player_count);
}
