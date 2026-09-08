//! Serialization & Backward Compatibility tests.
//!
//! Verifies that the core experiment types survive a JSON round-trip and that
//! all `MetricResult` variants serialize correctly.
use matchlab_experiments::checkpoint::{
    Checkpoint, MetricsSnapshot, QueueSnapshot, ReplicationProgress, WorldSnapshot,
};
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::identity::RunId;
use matchlab_experiments::replicate::{
    ArmResult, DesignType, ReplicateResult, SeedStrategy, StudyResult,
};
use matchlab_experiments::runner::ExperimentResult;
use matchlab_metrics::MetricResult;
use std::collections::BTreeMap;
fn sample_config() -> ExperimentConfig {
    let yaml = r#"
experiment:
  name: round_trip_test
  description: "A config for round-trip testing"
  seed: 42
  population:
    size: 500
    seed: 42
    archetypes:
      - name: stable
        proportion: 0.7
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
        role: tank
      - name: carry
        proportion: 0.3
        skill_distribution: { type: log_normal, mean: 6.9, stddev: 0.2 }
        skill_volatility: 0.01
        improvement_rate: 0.1
        play_frequency: 0.9
        session_length: 3600.0
        quit_probability: 0.05
        initial_rating: 1200.0
  game:
    teams: { a: { size: 5, role: tank }, b: { size: 5 } }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics:
    - match_quality
    - queue_time
    - rating_accuracy
  cohorts:
    - name: all
      filter: { type: all }
  duration:
    matches: 1000000
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    serde_yaml::from_str(yaml).unwrap()
}
#[test]
fn experiment_config_round_trip_yaml() {
    let config = sample_config();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let back: ExperimentConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config.experiment.name, back.experiment.name);
    assert_eq!(config.experiment.seed, back.experiment.seed);
    assert_eq!(
        config.experiment.population.size,
        back.experiment.population.size
    );
    assert_eq!(
        config.experiment.game.teams.a.size(),
        back.experiment.game.teams.a.size()
    );
    assert_eq!(
        config.experiment.game.teams.b.size(),
        back.experiment.game.teams.b.size()
    );
    assert_eq!(config.experiment.metrics, back.experiment.metrics);
    assert_eq!(
        config.experiment.duration.matches,
        back.experiment.duration.matches
    );
    assert_eq!(
        config.experiment.duration.max_time,
        back.experiment.duration.max_time
    );
    assert_eq!(
        config.experiment.output.directory,
        back.experiment.output.directory
    );
    assert_eq!(
        config.experiment.rating.systems.len(),
        back.experiment.rating.systems.len()
    );
    assert_eq!(
        config.experiment.rating.systems[0].name,
        back.experiment.rating.systems[0].name
    );
    assert_eq!(
        config.experiment.matchmaking.script,
        back.experiment.matchmaking.script
    );
}
#[test]
fn experiment_config_round_trip_json() {
    let config = sample_config();
    let json = serde_json::to_string(&config).unwrap();
    let back: ExperimentConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.experiment.name, back.experiment.name);
    assert_eq!(config.experiment.seed, back.experiment.seed);
    assert_eq!(
        config.experiment.population.archetypes.len(),
        back.experiment.population.archetypes.len()
    );
    assert_eq!(
        config.experiment.population.archetypes[0].role,
        back.experiment.population.archetypes[0].role
    );
    assert_eq!(
        config.experiment.game.skill_update_interval_secs,
        back.experiment.game.skill_update_interval_secs
    );
}
#[test]
fn experiment_result_round_trip() {
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "rating_accuracy".to_string(),
        MetricResult::Summary {
            mean: 150.5,
            median: 145.0,
            p75: 180.0,
            p90: 200.0,
            p95: 220.0,
            p99: 250.0,
            stddev: 30.0,
        },
    );
    metrics.insert("match_quality".to_string(), MetricResult::Scalar(0.96));
    let result = ExperimentResult {
        experiment_id: "test-exp-42".to_string(),
        name: "test_experiment".to_string(),
        config_hash: "abc123def456".to_string(),
        git_commit: "deadbeef".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        matches_completed: 10000,
        matches_formed: 10050,
        simulated_time_secs: 3600.0,
        metrics,
        utility_score: Some(0.85),
    };
    let json = serde_json::to_string(&result).unwrap();
    let back: ExperimentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result.experiment_id, back.experiment_id);
    assert_eq!(result.name, back.name);
    assert_eq!(result.config_hash, back.config_hash);
    assert_eq!(result.git_commit, back.git_commit);
    assert_eq!(result.matches_completed, back.matches_completed);
    assert_eq!(result.matches_formed, back.matches_formed);
    assert_eq!(result.simulated_time_secs, back.simulated_time_secs);
    assert_eq!(result.utility_score, back.utility_score);
    assert_eq!(result.metrics, back.metrics);
}
#[test]
fn experiment_result_yaml_round_trip() {
    let mut metrics = BTreeMap::new();
    metrics.insert("queue_time".to_string(), MetricResult::Scalar(5.2));
    let result = ExperimentResult {
        experiment_id: "yaml-test".to_string(),
        name: "yaml_experiment".to_string(),
        config_hash: "hash123".to_string(),
        git_commit: "commit456".to_string(),
        timestamp: "2024-06-01T12:00:00Z".to_string(),
        matches_completed: 500,
        matches_formed: 510,
        simulated_time_secs: 300.0,
        metrics,
        utility_score: None,
    };
    let yaml = serde_yaml::to_string(&result).unwrap();
    let back: ExperimentResult = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(result.experiment_id, back.experiment_id);
    assert_eq!(result.utility_score, back.utility_score);
}
#[test]
fn metric_result_all_variants_serialize() {
    let variants = vec![
        MetricResult::Scalar(42.0),
        MetricResult::Distribution(vec![1.0, 2.0, 3.0]),
        MetricResult::Summary {
            mean: 100.0,
            median: 95.0,
            p75: 120.0,
            p90: 140.0,
            p95: 160.0,
            p99: 190.0,
            stddev: 25.0,
        },
        MetricResult::Histogram {
            buckets: vec![(0.0, 10), (50.0, 20), (100.0, 5)],
        },
        MetricResult::TimeSeries {
            bucket_means: vec![1.0, 2.0, 3.0, 4.0],
        },
    ];
    for variant in variants {
        let json = serde_json::to_string(&variant).unwrap();
        let back: MetricResult = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back, "round-trip failed for {json}");
    }
}
#[test]
fn metric_result_yaml_round_trip() {
    let variants = vec![
        MetricResult::Scalar(42.0),
        MetricResult::Distribution(vec![1.0, 2.0, 3.0]),
        MetricResult::Summary {
            mean: 100.0,
            median: 95.0,
            p75: 120.0,
            p90: 140.0,
            p95: 160.0,
            p99: 190.0,
            stddev: 25.0,
        },
        MetricResult::Histogram {
            buckets: vec![(0.0, 10), (50.0, 20)],
        },
        MetricResult::TimeSeries {
            bucket_means: vec![1.1, 2.2, 3.3],
        },
    ];
    for variant in variants {
        let yaml = serde_yaml::to_string(&variant).unwrap();
        let back: MetricResult = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(variant, back, "YAML round-trip failed for {yaml}");
    }
}
#[test]
fn checkpoint_round_trip() {
    let mut progress = std::collections::HashMap::new();
    progress.insert(
        "rep-0".to_string(),
        ReplicationProgress {
            matches_completed: 100,
            simulated_time_secs: 3600.0,
            is_complete: false,
        },
    );
    progress.insert(
        "rep-1".to_string(),
        ReplicationProgress {
            matches_completed: 200,
            simulated_time_secs: 7200.0,
            is_complete: true,
        },
    );
    let checkpoint = Checkpoint {
        run_id: RunId("run-42".to_string()),
        timestamp_secs: 3600.0,
        world_state: WorldSnapshot {
            player_count: 1000,
            match_count: 5000,
            time_secs: 3600.0,
            players_json: r#"[{"id":1,"rating":1000}]"#.to_string(),
        },
        queue_state: QueueSnapshot {
            entry_count: 50,
            entries_json: r#"[{"player_id":1}]"#.to_string(),
        },
        metrics_snapshot: MetricsSnapshot {
            metrics_json: r#"{"rating_accuracy":150.0}"#.to_string(),
        },
        rng_state: "dGVzdHN0YXRl".to_string(),
        replication_progress: progress,
    };
    let json = serde_json::to_string_pretty(&checkpoint).unwrap();
    let back: Checkpoint = serde_json::from_str(&json).unwrap();
    assert_eq!(checkpoint.run_id, back.run_id);
    assert_eq!(checkpoint.timestamp_secs, back.timestamp_secs);
    assert_eq!(
        checkpoint.world_state.player_count,
        back.world_state.player_count
    );
    assert_eq!(
        checkpoint.world_state.match_count,
        back.world_state.match_count
    );
    assert_eq!(checkpoint.world_state.time_secs, back.world_state.time_secs);
    assert_eq!(
        checkpoint.world_state.players_json,
        back.world_state.players_json
    );
    assert_eq!(
        checkpoint.queue_state.entry_count,
        back.queue_state.entry_count
    );
    assert_eq!(
        checkpoint.metrics_snapshot.metrics_json,
        back.metrics_snapshot.metrics_json
    );
    assert_eq!(checkpoint.rng_state, back.rng_state);
    assert_eq!(
        checkpoint.replication_progress.len(),
        back.replication_progress.len()
    );
    assert_eq!(
        checkpoint.replication_progress["rep-0"].matches_completed,
        back.replication_progress["rep-0"].matches_completed
    );
    assert_eq!(
        checkpoint.replication_progress["rep-1"].is_complete,
        back.replication_progress["rep-1"].is_complete
    );
}
#[test]
fn checkpoint_yaml_round_trip() {
    let checkpoint = Checkpoint {
        run_id: RunId("run-y".to_string()),
        timestamp_secs: 7200.0,
        world_state: WorldSnapshot {
            player_count: 200,
            match_count: 1000,
            time_secs: 7200.0,
            players_json: "[]".to_string(),
        },
        queue_state: QueueSnapshot {
            entry_count: 0,
            entries_json: "[]".to_string(),
        },
        metrics_snapshot: MetricsSnapshot {
            metrics_json: "{}".to_string(),
        },
        rng_state: "base64data".to_string(),
        replication_progress: std::collections::HashMap::new(),
    };
    let yaml = serde_yaml::to_string(&checkpoint).unwrap();
    let back: Checkpoint = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(checkpoint.run_id, back.run_id);
    assert_eq!(
        checkpoint.world_state.player_count,
        back.world_state.player_count
    );
}
fn sample_study_result() -> StudyResult {
    let mut metrics = BTreeMap::new();
    metrics.insert("rating_accuracy".to_string(), MetricResult::Scalar(140.0));
    StudyResult {
        study_id: "elo_vs_glicko-abc123-crn-r50".to_string(),
        name: "elo_vs_glicko".to_string(),
        config_hash: "abc123".to_string(),
        git_commit: "deadbeef".to_string(),
        strategy: SeedStrategy::Crn,
        design: DesignType::Paired,
        experimental_design: matchlab_experiments::design::ExperimentalDesign::Crn {
            pairing: matchlab_experiments::design::PairingKey {
                name: "replicate_seed".to_string(),
                description: "shared RNG stream".to_string(),
            },
        },
        replication_count: 50,
        arms: vec![
            ArmResult {
                name: "elo".to_string(),
                condition_id: "elo".to_string(),
                replicates: vec![ReplicateResult {
                    replicate_index: 0,
                    seed: 42,
                    parent_seed: 42,
                    result: ExperimentResult {
                        experiment_id: "elo-0".to_string(),
                        name: "elo".to_string(),
                        config_hash: "hash_elo".to_string(),
                        git_commit: "deadbeef".to_string(),
                        timestamp: "2024-01-01T00:00:00Z".to_string(),
                        matches_completed: 1000,
                        matches_formed: 1010,
                        simulated_time_secs: 3600.0,
                        metrics: metrics.clone(),
                        utility_score: None,
                    },
                }],
            },
            ArmResult {
                name: "glicko2".to_string(),
                condition_id: "glicko2".to_string(),
                replicates: vec![ReplicateResult {
                    replicate_index: 0,
                    seed: 42,
                    parent_seed: 42,
                    result: ExperimentResult {
                        experiment_id: "glicko2-0".to_string(),
                        name: "glicko2".to_string(),
                        config_hash: "hash_glicko".to_string(),
                        git_commit: "deadbeef".to_string(),
                        timestamp: "2024-01-01T00:00:00Z".to_string(),
                        matches_completed: 1000,
                        matches_formed: 1010,
                        simulated_time_secs: 3600.0,
                        metrics,
                        utility_score: None,
                    },
                }],
            },
        ],
    }
}
#[test]
fn study_result_round_trip_json() {
    let study = sample_study_result();
    let json = serde_json::to_string(&study).unwrap();
    let back: StudyResult = serde_json::from_str(&json).unwrap();
    assert_eq!(study.study_id, back.study_id);
    assert_eq!(study.name, back.name);
    assert_eq!(study.config_hash, back.config_hash);
    assert_eq!(study.git_commit, back.git_commit);
    assert_eq!(study.strategy, back.strategy);
    assert_eq!(study.design, back.design);
    assert_eq!(study.replication_count, back.replication_count);
    assert_eq!(study.arms.len(), back.arms.len());
    assert_eq!(study.arms[0].name, back.arms[0].name);
    assert_eq!(study.arms[0].condition_id, back.arms[0].condition_id);
    assert_eq!(
        study.arms[0].replicates.len(),
        back.arms[0].replicates.len()
    );
    assert_eq!(
        study.arms[0].replicates[0].seed,
        back.arms[0].replicates[0].seed
    );
    assert_eq!(
        study.arms[0].replicates[0].result.matches_completed,
        back.arms[0].replicates[0].result.matches_completed
    );
}
#[test]
fn study_result_round_trip_yaml() {
    let study = sample_study_result();
    let yaml = serde_yaml::to_string(&study).unwrap();
    let back: StudyResult = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(study.study_id, back.study_id);
    assert_eq!(study.arms.len(), back.arms.len());
    assert_eq!(
        study.arms[0].replicates[0].result.metrics,
        back.arms[0].replicates[0].result.metrics
    );
}
#[test]
fn experiment_config_optional_fields_default() {
    let yaml = r#"
experiment:
  name: minimal
  seed: 1
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 100 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.5
        session_length: 600.0
        quit_probability: 0.0
  game:
    script: plugins/game/logistic.lua
    beta: 400.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 60.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.experiment.description, None);
    assert!(config.experiment.detection.is_none());
    assert!(config.experiment.ranking.is_none());
    assert!(config.experiment.objectives.is_none());
    assert!(config.experiment.adversarial.is_none());
    assert!(config.experiment.satisfaction.is_none());
    assert!(config.experiment.replication.is_none());
    assert_eq!(config.experiment.game.skill_update_interval_secs, None);
}
#[test]
fn seed_strategy_and_design_type_serde() {
    let strategies = vec![
        SeedStrategy::Independent,
        SeedStrategy::Crn,
        SeedStrategy::Counterfactual,
    ];
    for s in strategies {
        let json = serde_json::to_string(&s).unwrap();
        let back: SeedStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
    let designs = vec![
        DesignType::Independent,
        DesignType::Paired,
        DesignType::Counterfactual,
    ];
    for d in designs {
        let json = serde_json::to_string(&d).unwrap();
        let back: DesignType = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
#[test]
fn nested_metric_result_in_btreemap() {
    let mut metrics = BTreeMap::new();
    metrics.insert("accuracy".to_string(), MetricResult::Scalar(1.0));
    metrics.insert(
        "quality".to_string(),
        MetricResult::Summary {
            mean: 0.95,
            median: 0.96,
            p75: 0.98,
            p90: 0.99,
            p95: 0.995,
            p99: 0.999,
            stddev: 0.02,
        },
    );
    metrics.insert(
        "timeseries".to_string(),
        MetricResult::TimeSeries {
            bucket_means: vec![10.0, 20.0, 30.0],
        },
    );
    let json = serde_json::to_string(&metrics).unwrap();
    let back: BTreeMap<String, MetricResult> = serde_json::from_str(&json).unwrap();
    assert_eq!(metrics, back);
    assert!(json.contains("\"accuracy\""));
    assert!(json.contains("\"quality\""));
    assert!(json.contains("\"timeseries\""));
}
