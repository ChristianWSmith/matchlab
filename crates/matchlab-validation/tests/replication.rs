//! replication-pipeline validation: the analytical baselines for the
//! v0.2 research unit (a replicated study) beyond what the loop-level suites
//! already prove.
//!
//! These pin the research pipeline itself — replication determinism, CRN
//! pairing honesty, counterfactual replay identity, the per-stream RNG
//! guarantee in study context, and the CI-width contract — with fixed seeds so
//! every assertion is deterministic.
use matchlab_analysis::effect::extract_scalar;
use matchlab_analysis::study::{StudyReportConfig, compute_study_stats, write_study_result_json};
use matchlab_core::match_::TeamComposition;
use matchlab_core::rng::StreamSeeds;
use matchlab_core::time::SimTime;
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::replicate::{
    ArmConfig, ReplicationRunner, ReplicationSpec, SeedStrategy, StudyResult,
};
use matchlab_experiments::runner::ExperimentRunner;
use matchlab_loop::LoopConfig;
use matchlab_metrics::MetricResult;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_metrics::lua::LuaMetricCollector;
/// Fast cold-start elo manifest: a small 1v1 population so replicate counts can
/// be meaningful without a minutes-long CI budget.
fn elo_manifest(name: &str, matches: u64) -> ExperimentConfig {
    serde_yaml::from_str(&format!(
        r#"
experiment:
  name: {name}
  seed: 1
  population:
    size: 40
    seed: 1
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: {{ type: normal, mean: 1000, stddev: 250 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    teams: {{ a: 1, b: 1 }}
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
  metrics: [match_quality, rating_accuracy]
  cohorts: []
  duration:
    matches: {matches}
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        name = name,
        matches = matches
    ))
    .expect("valid elo manifest")
}
/// Same skeleton with the Glicko-2 script: cold start (initial_rd 350) so its
/// convergence trail is distinct from Elo's.
fn glicko_manifest(name: &str, matches: u64) -> ExperimentConfig {
    serde_yaml::from_str(&format!(
        r#"
experiment:
  name: {name}
  seed: 1
  population:
    size: 40
    seed: 1
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: {{ type: normal, mean: 1000, stddev: 250 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    teams: {{ a: 1, b: 1 }}
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - script: plugins/rating/glicko2.lua
        initial_rating: 1000.0
        initial_rd: 350.0
        initial_volatility: 0.06
        tau: 0.5
  metrics: [match_quality, rating_accuracy]
  cohorts: []
  duration:
    matches: {matches}
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        name = name,
        matches = matches
    ))
    .expect("valid glicko manifest")
}
fn elo_arms() -> Vec<ArmConfig> {
    vec![
        ArmConfig {
            name: "elo_a".to_string(),
            config: elo_manifest("elo_a", 60),
        },
        ArmConfig {
            name: "elo_b".to_string(),
            config: elo_manifest("elo_b", 60),
        },
    ]
}
fn elo_vs_glicko_arms() -> Vec<ArmConfig> {
    vec![
        ArmConfig {
            name: "elo".to_string(),
            config: elo_manifest("elo", 60),
        },
        ArmConfig {
            name: "glicko2".to_string(),
            config: glicko_manifest("glicko2", 60),
        },
    ]
}
fn run_study(arms: &[ArmConfig], strategy: SeedStrategy, count: u64) -> StudyResult {
    ReplicationRunner::run_arms(
        arms,
        &ReplicationSpec {
            count,
            strategy,
            base_seed: 42,
        },
    )
    .expect("study runs")
}
/// Per-replicate scalar of one metric for one arm.
fn arm_scalars(study: &StudyResult, arm_index: usize, metric: &str) -> Vec<f64> {
    study.arms[arm_index]
        .replicates
        .iter()
        .map(|repl| extract_scalar(&repl.result.metrics[metric]).expect("scalarizable metric"))
        .collect()
}
fn metric_mean(result: &MetricResult) -> f64 {
    match result {
        MetricResult::Summary { mean, .. } => *mean,
        other => panic!("expected summary, got {other:?}"),
    }
}
/// CRN paired honesty: identical (elo vs elo) arms share the replicate seed and
/// produce byte-identical runs — so the paired delta is provably zero and the
/// effect CI contains 0.
#[test]
fn crn_identical_arms_share_seeds_and_zero_effect() {
    let study = run_study(&elo_arms(), SeedStrategy::Crn, 5);
    assert_eq!(study.arms.len(), 2);
    for (a, b) in study.arms[0]
        .replicates
        .iter()
        .zip(&study.arms[1].replicates)
    {
        assert_eq!(a.seed, b.seed, "CRN arms share the replicate seed ");
        assert_eq!(
            a.replicate_index, b.replicate_index,
            "arms of a replicate share its index"
        );
        assert_eq!(
            arm_scalars(&study, 0, "rating_accuracy"),
            arm_scalars(&study, 1, "rating_accuracy"),
            "identical arms run identically under CRN"
        );
    }
    let stats = compute_study_stats(&study, &StudyReportConfig::default());
    let ra_stats: Vec<_> = stats
        .effects
        .iter()
        .filter(|e| e.metric == "rating_accuracy")
        .collect();
    assert_eq!(ra_stats.len(), 1, "one pairwise effect (elo_a vs elo_b)");
    let es = ra_stats[0];
    assert!(
        es.mean_delta.abs() < 1e-9,
        "identical arms must have zero mean delta, got {}",
        es.mean_delta
    );
    assert!(es.ci_lo <= 0.0 && es.ci_hi >= 0.0, "delta CI contains 0");
}
/// Negative control: independent identical arms diverge (different seeds →
/// different runs), proving the zero-delta above is caused by CRN pairing, not
/// by the config being the same.
#[test]
fn independent_identical_arms_diverge() {
    let study = run_study(&elo_arms(), SeedStrategy::Independent, 5);
    for (a, b) in study.arms[0]
        .replicates
        .iter()
        .zip(&study.arms[1].replicates)
    {
        assert_ne!(a.seed, b.seed, "independent arms derive distinct seeds");
        let a_ra = metric_mean(&a.result.metrics["rating_accuracy"]);
        let b_ra = metric_mean(&b.result.metrics["rating_accuracy"]);
        assert!(
            (a_ra - b_ra).abs() > 1e-9,
            "independent runs must diverge ({} vs {})",
            a_ra,
            b_ra
        );
    }
}
/// Counterfactual honesty: the replay arm reruns the identical recorded match
/// history through the same system, so its rating trajectories reproduce the
/// live arm's (bit-identical per-participant ratings; summary aggregates equal
/// within floating-point noise from the different fold order), and the
/// recording run is byte-identical to a plain run of the same seed.
#[test]
fn counterfactual_replay_equals_live_arm_with_same_system() {
    let study = run_study(&elo_arms(), SeedStrategy::Counterfactual, 5);
    assert_eq!(study.strategy, SeedStrategy::Counterfactual);
    for (live, replay) in study.arms[0]
        .replicates
        .iter()
        .zip(&study.arms[1].replicates)
    {
        assert_eq!(live.seed, replay.parent_seed);
        assert_ne!(replay.seed, replay.parent_seed);
        assert_eq!(
            live.result.matches_completed, replay.result.matches_completed,
            "replay runs the recorded match count"
        );
        for (la, ra) in arm_scalars(&study, 0, "rating_accuracy")
            .iter()
            .zip(arm_scalars(&study, 1, "rating_accuracy"))
        {
            assert!(
                (la - ra).abs() / la.abs() < 1e-9,
                "replay accuracy must track live: {la} vs {ra}"
            );
        }
        let mut plain = elo_manifest(&live.result.name, 60);
        plain.experiment.seed = live.seed;
        let mut recorded_result = live.result.clone();
        let plain_result = ExperimentRunner::run(&plain).expect("plain run");
        recorded_result.timestamp.clear();
        let mut plain_result = plain_result;
        plain_result.timestamp.clear();
        assert_eq!(
            recorded_result, plain_result,
            "recording must not perturb the simulated output"
        );
    }
}
/// Different rating systems under CRN produce disjoint accuracy distributions:
/// the cold-start Glicko-2 (initial_rd 350) trails Elo on MAE in the same
/// paired match stream.
#[test]
fn different_systems_have_disjoint_accuracy() {
    let study = run_study(&elo_vs_glicko_arms(), SeedStrategy::Crn, 5);
    let elo_ra = arm_scalars(&study, 0, "rating_accuracy");
    let glicko_ra = arm_scalars(&study, 1, "rating_accuracy");
    assert_eq!(elo_ra.len(), 5);
    let elo_max = elo_ra.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let glicko_min = glicko_ra.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(
        glicko_min > elo_max,
        "glicko2 MAE ({glicko_ra:?}) must exceed elo MAX ({elo_ra:?})"
    );
    let stats = compute_study_stats(&study, &StudyReportConfig::default());
    let es = stats
        .effects
        .iter()
        .find(|e| e.metric == "rating_accuracy")
        .expect("rating_accuracy effect");
    assert!(es.mean_delta > 0.0, "delta positive when glicko > elo");
    assert!(es.ci_lo > 0.0, "disjoint distributions exclude 0");
}
/// Study determinism: identical inputs produce byte-identical exported
/// `study_stats.json` (aggregates carry no wall-clock data) and structurally
/// identical `<study_id>.json` net of the timestamps.
#[test]
fn study_json_is_deterministic_on_rerun() {
    let dir_a = std::env::temp_dir().join("matchlab-replication-a");
    let dir_b = std::env::temp_dir().join("matchlab-replication-b");
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
    let a = run_study(&elo_vs_glicko_arms(), SeedStrategy::Crn, 3);
    let b = run_study(&elo_vs_glicko_arms(), SeedStrategy::Crn, 3);
    assert_eq!(a.study_id, b.study_id);
    assert_eq!(a.arms[0].name, "elo");
    assert_eq!(a.arms[1].name, "glicko2");
    for (x, y) in a.arms.iter().zip(&b.arms) {
        assert_eq!(x.name, y.name);
    }
    write_study_result_json(&a, dir_a.to_string_lossy().as_ref()).unwrap();
    write_study_result_json(&b, dir_b.to_string_lossy().as_ref()).unwrap();
    let stats_a = std::fs::read_to_string(dir_a.join("study_stats.json")).unwrap();
    let stats_b = std::fs::read_to_string(dir_b.join("study_stats.json")).unwrap();
    assert_eq!(stats_a, stats_b, "aggregate stats byte-identical");
    let mut ja: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir_a.join(format!("{}.json", a.study_id))).unwrap(),
    )
    .unwrap();
    let mut jb: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir_b.join(format!("{}.json", b.study_id))).unwrap(),
    )
    .unwrap();
    for j in [&mut ja, &mut jb] {
        for arm in j["arms"].as_array_mut().unwrap() {
            for repl in arm["replicates"].as_array_mut().unwrap() {
                repl["result"]["timestamp"] = serde_json::Value::Null;
            }
        }
    }
    assert_eq!(ja, jb, "study JSON identical net of timestamps");
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}
/// M1.4 stream guarantee in study context: with no adversarial agents or
/// satisfaction model configured, the `behavior` stream is never drawn, so
/// varying only the behavior RNG seed leaves the run byte-identical.
#[test]
fn behavior_stream_is_inert_without_adversarial_satisfaction() {
    use matchlab_core::rng::SimRng;
    use matchlab_game::lua::LuaOutcomeModel;
    use matchlab_matchmaking::lua::LuaMatchmaker;
    use matchlab_players::population::{PopulationConfig, PopulationGenerator};
    use matchlab_rating::registry;
    fn archetype() -> matchlab_players::archetype::ArchetypeConfig {
        serde_yaml::from_str(
            "name: stable
proportion: 1.0
skill_distribution: { type: normal, mean: 1000, stddev: 250 }
skill_volatility: 0.0
improvement_rate: 0.0
play_frequency: 0.8
session_length: 1800.0
quit_probability: 0.0
initial_rating: 1000.0
",
        )
        .unwrap()
    }
    let population_config = PopulationConfig {
        size: 40,
        archetypes: vec![archetype()],
    };
    let mut rng = SimRng::from_seed(7);
    let (realities, observations) = PopulationGenerator::generate(&population_config, &mut rng);
    let population: Vec<_> = realities.into_iter().zip(observations).collect();
    let build = |behavior_seed: u64| {
        let teams = TeamComposition {
            team_size_a: 1,
            team_size_b: 1,
            role_a: None,
            role_b: None,
        };
        let mut streams = StreamSeeds::from_seed(12345);
        streams.behavior = behavior_seed;
        let mut engine = MetricsEngine::new();
        engine.register(Box::new(
            LuaMetricCollector::load(
                "plugins/metrics/rating_accuracy.lua",
                &serde_yaml::Value::Null,
            )
            .unwrap(),
        ));
        let config = LoopConfig {
            teams,
            batch_interval_ticks: 10,
            rejoin_delay: SimTime::from_secs(30.0),
            max_matches: 60,
            skill_update_interval: None,
            stream_seeds: streams,
            record_history: false,
        };
        let rating = registry::from_name(
            "elo",
            &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
        )
        .unwrap();
        let outcome = LuaOutcomeModel::load(
            "plugins/game/logistic.lua",
            &serde_yaml::from_str("beta: 400.0\nnoise: 0.05").unwrap(),
        )
        .unwrap();
        let matchmaker =
            LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap();
        let mut loop_ = matchlab_loop::MatchLoop::new(
            population.clone(),
            rating,
            Box::new(outcome),
            Box::new(matchmaker),
            engine,
            config,
        );
        loop_.run_until(SimTime::from_secs(60_000.0));
        let (matches_completed, sim_time) = {
            let state = loop_.state.lock().unwrap();
            (state.matches_completed, loop_.world.time.as_secs_f64())
        };
        (loop_.finalize_metrics(), matches_completed, sim_time)
    };
    let (metrics_a, m_a, t_a) = build(404);
    let (metrics_b, m_b, t_b) = build(999_999);
    assert_eq!(m_a, m_b, "matches completed unaffected");
    assert_eq!(t_a, t_b, "sim time unaffected");
    assert_eq!(
        metrics_a["rating_accuracy"], metrics_b["rating_accuracy"],
        "behavior stream must not perturb a run without adversarial/satisfaction"
    );
}
/// The effect CI width contracts as the replicate count grows: same study
/// seed, 24 vs 96 replicates, paired deltas from identical match streams.
#[test]
fn effect_ci_width_shrinks_as_replicates_grow() {
    let width = |count: u64| {
        let study = run_study(&elo_vs_glicko_arms(), SeedStrategy::Crn, count);
        let stats = compute_study_stats(&study, &StudyReportConfig::default());
        let es = stats
            .effects
            .iter()
            .find(|e| e.metric == "rating_accuracy")
            .expect("rating_accuracy effect");
        es.ci_hi - es.ci_lo
    };
    let small = width(24);
    let large = width(96);
    assert!(
        large < small,
        "CI must tighten with more replicates ({large} vs {small})"
    );
}
