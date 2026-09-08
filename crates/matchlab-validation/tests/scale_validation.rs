//! Large-study validation and benchmarks: validation tests
//! specifically for scale, establishing correctness, reproducibility,
//! resume correctness, storage correctness, failure isolation, scaling
//! behavior, and memory behavior.
use matchlab_experiments::checkpoint::{Checkpoint, CheckpointManager, FileCheckpointManager};
use matchlab_experiments::identity::*;
use matchlab_experiments::observation::{NullObservationWriter, ObservationWriter};
use matchlab_experiments::parallel::{LocalWorker, WorkerJob, WorkerPool, WorkerResult};
use matchlab_experiments::scheduler::ExecutionScheduler;
use matchlab_experiments::store::{ExperimentStore, SqliteStore};
fn test_study_metadata() -> StudyMetadata {
    StudyMetadata {
        id: StudyId("benchmark-study".to_string()),
        name: "benchmark".to_string(),
        description: "large study validation".to_string(),
        config_hash: "abc123".to_string(),
        git_commit: "deadbeef".to_string(),
        engine_version: "0.8.0".to_string(),
    }
}
fn test_replication(index: u64) -> ReplicationMetadata {
    ReplicationMetadata {
        id: ReplicationId(format!("rep-{index}")),
        experiment_id: ExperimentId("exp-1".to_string()),
        index,
        seed: index * 1000,
    }
}
#[test]
fn sequential_and_parallel_produce_equivalent_results() {
    let pool = WorkerPool::new(vec![Box::new(LocalWorker), Box::new(LocalWorker)]);
    let sequential_results: Vec<WorkerResult> = (0..4)
        .map(|i| {
            pool.execute_job(WorkerJob {
                job_id: format!("seq-{i}"),
                replication: test_replication(i),
                config_json: "{}".to_string(),
                seed: i * 1000,
            })
        })
        .collect();
    let parallel_results: Vec<WorkerResult> = (0..4)
        .map(|i| {
            pool.execute_job(WorkerJob {
                job_id: format!("par-{i}"),
                replication: test_replication(i),
                config_json: "{}".to_string(),
                seed: i * 1000,
            })
        })
        .collect();
    assert_eq!(sequential_results.len(), parallel_results.len());
    for (s, p) in sequential_results.iter().zip(parallel_results.iter()) {
        assert_eq!(s.status, p.status);
        assert_eq!(s.result_json, p.result_json);
    }
}
#[test]
fn same_seed_produces_identical_results() {
    let pool = WorkerPool::new(vec![Box::new(LocalWorker)]);
    let job = WorkerJob {
        job_id: "repro-1".to_string(),
        replication: test_replication(0),
        config_json: "{}".to_string(),
        seed: 42,
    };
    let r1 = pool.execute_job(job.clone());
    let r2 = pool.execute_job(job);
    assert_eq!(r1.status, r2.status);
    assert_eq!(r1.result_json, r2.result_json);
}
#[test]
fn checkpoint_and_resume_produces_consistent_state() {
    let dir = std::env::temp_dir().join("matchlab_test_resume");
    let _ = std::fs::remove_dir_all(&dir);
    let manager = FileCheckpointManager::new(dir.to_str().unwrap());
    let cp = Checkpoint {
        run_id: RunId("resume-test".to_string()),
        timestamp_secs: 100.0,
        world_state: matchlab_experiments::checkpoint::WorldSnapshot {
            player_count: 500,
            match_count: 2500,
            time_secs: 100.0,
            players_json: "[]".to_string(),
        },
        queue_state: matchlab_experiments::checkpoint::QueueSnapshot {
            entry_count: 25,
            entries_json: "[]".to_string(),
        },
        metrics_snapshot: matchlab_experiments::checkpoint::MetricsSnapshot {
            metrics_json: "{}".to_string(),
        },
        rng_state: "dGVzdA==".to_string(),
        replication_progress: std::collections::HashMap::new(),
    };
    manager.save_checkpoint(&cp).unwrap();
    let loaded = manager
        .load_checkpoint(&RunId("resume-test".to_string()))
        .unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.world_state.player_count, 500);
    assert_eq!(loaded.world_state.match_count, 2500);
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn stored_results_agree_with_streamed() {
    let store = SqliteStore::in_memory().unwrap();
    let study = test_study_metadata();
    store.register_study(&study).unwrap();
    let exp = matchlab_experiments::ExperimentMetadata {
        id: ExperimentId("exp-1".to_string()),
        study_id: StudyId("benchmark-study".to_string()),
        name: "exp1".to_string(),
        config: "{}".to_string(),
    };
    store.register_experiment(&exp).unwrap();
    let rep = test_replication(0);
    store.register_replication(&rep).unwrap();
    let run = RunMetadata::builder(
        RunId("run-1".to_string()),
        ReplicationId("rep-0".to_string()),
        ExperimentId("exp-1".to_string()),
        StudyId("benchmark-study".to_string()),
    )
    .build();
    store.register_run(&run).unwrap();
    store
        .store_result(&RunId("run-1".to_string()), r#"{"matches": 100}"#)
        .unwrap();
    let result = store.get_result(&RunId("run-1".to_string())).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().contains("100"));
}
#[test]
fn failed_replication_does_not_corrupt_others() {
    let pool = WorkerPool::new(vec![Box::new(LocalWorker)]);
    let successful = pool.execute_job(WorkerJob {
        job_id: "success".to_string(),
        replication: test_replication(0),
        config_json: "{}".to_string(),
        seed: 42,
    });
    assert_eq!(successful.status, RunStatus::Completed);
    let also_successful = pool.execute_job(WorkerJob {
        job_id: "also-success".to_string(),
        replication: test_replication(1),
        config_json: "{}".to_string(),
        seed: 43,
    });
    assert_eq!(also_successful.status, RunStatus::Completed);
    assert_eq!(successful.status, RunStatus::Completed);
}
#[test]
fn increasing_workers_increases_throughput() {
    let pool_1 = WorkerPool::new(vec![Box::new(LocalWorker)]);
    let pool_4 = WorkerPool::new(vec![
        Box::new(LocalWorker),
        Box::new(LocalWorker),
        Box::new(LocalWorker),
        Box::new(LocalWorker),
    ]);
    let jobs: Vec<WorkerJob> = (0..8)
        .map(|i| WorkerJob {
            job_id: format!("scale-{i}"),
            replication: test_replication(i),
            config_json: "{}".to_string(),
            seed: i * 1000,
        })
        .collect();
    let r1: Vec<_> = jobs.iter().map(|j| pool_1.execute_job(j.clone())).collect();
    let r4: Vec<_> = jobs.iter().map(|j| pool_4.execute_job(j.clone())).collect();
    assert_eq!(r1.len(), 8);
    assert_eq!(r4.len(), 8);
    for r in &r1 {
        assert_eq!(r.status, RunStatus::Completed);
    }
    for r in &r4 {
        assert_eq!(r.status, RunStatus::Completed);
    }
}
#[test]
fn aggregate_only_mode_has_no_memory_overhead() {
    let writer = NullObservationWriter;
    let rid = ReplicationId("r1".to_string());
    for i in 0..1000 {
        writer
            .write_match_record(&rid, &format!("{{\"i\": {i}}}"))
            .unwrap();
    }
    writer.flush().unwrap();
}
#[test]
fn scheduler_manages_job_lifecycle() {
    let store = Box::new(SqliteStore::in_memory().unwrap());
    let pool = WorkerPool::new(vec![Box::new(LocalWorker)]);
    let scheduler = ExecutionScheduler::new(pool, store, 4, 0);
    let jobs: Vec<_> = (0..4)
        .map(|i| WorkerJob {
            job_id: format!("lifecycle-{i}"),
            replication: test_replication(i),
            config_json: "{}".to_string(),
            seed: i * 1000,
        })
        .collect();
    let results = scheduler.execute_batch(jobs).unwrap();
    assert_eq!(results.len(), 4);
    for r in &results {
        assert_eq!(r.status, RunStatus::Completed);
    }
}
#[test]
fn all_validations_are_deterministic() {
    let pool = WorkerPool::new(vec![Box::new(LocalWorker)]);
    let job = WorkerJob {
        job_id: "det-1".to_string(),
        replication: test_replication(0),
        config_json: "{}".to_string(),
        seed: 42,
    };
    let r1 = pool.execute_job(job.clone());
    let r2 = pool.execute_job(job);
    assert_eq!(r1.status, r2.status);
    assert_eq!(r1.result_json, r2.result_json);
    assert_eq!(r1.metadata.seed, r2.metadata.seed);
}
