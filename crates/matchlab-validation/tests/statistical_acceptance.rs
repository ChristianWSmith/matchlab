//! Statistical acceptance suite: six synthetic studies with
//! known truth, testing the full chain from replication aggregation to report.
use matchlab_analysis::effect::{CiMethod, effect_sizes};
use matchlab_analysis::estimand::{Estimand, EstimandDef};
use matchlab_analysis::hierarchy::ReplicationScalar;
use matchlab_analysis::result::StatisticalResult;
use matchlab_analysis::robustness::run_robustness;
use matchlab_analysis::study::{StudyReportConfig, generate_research_report};
use matchlab_core::rng::SimRng;
use matchlab_experiments::ExperimentResult;
use matchlab_experiments::replicate::{ArmResult, ReplicateResult};
use std::collections::BTreeMap;
fn metric_result(mean: f64) -> matchlab_metrics::MetricResult {
    matchlab_metrics::MetricResult::Summary {
        mean,
        median: mean,
        p75: mean,
        p90: mean,
        p95: mean,
        p99: mean,
        stddev: 0.0,
    }
}
fn fake_result(name: &str, ra_mean: f64) -> ExperimentResult {
    let mut metrics = BTreeMap::new();
    metrics.insert("rating_accuracy".to_string(), metric_result(ra_mean));
    ExperimentResult {
        experiment_id: format!("{name}-x"),
        name: name.to_string(),
        config_hash: "hash".to_string(),
        git_commit: "abc".to_string(),
        timestamp: "t".to_string(),
        matches_completed: 10,
        matches_formed: 10,
        simulated_time_secs: 100.0,
        metrics,
        utility_score: None,
    }
}
fn fake_arm(name: &str, means: &[f64]) -> ArmResult {
    ArmResult {
        name: name.to_string(),
        condition_id: name.to_string(),
        replicates: means
            .iter()
            .enumerate()
            .map(|(i, m)| ReplicateResult {
                replicate_index: i as u64,
                seed: i as u64,
                parent_seed: 0,
                result: fake_result(name, *m),
            })
            .collect(),
    }
}
fn arm_scalars(arm: &ArmResult) -> Vec<ReplicationScalar> {
    use matchlab_analysis::hierarchy::replication_scalars;
    replication_scalars(arm, "rating_accuracy")
        .iter()
        .filter_map(|o| o.replication_scalar())
        .collect()
}
fn paired_study(_name: &str, control_means: &[f64], treatment_means: &[f64]) -> StatisticalResult {
    let ca = fake_arm("control", control_means);
    let ta = fake_arm("treatment", treatment_means);
    let c = arm_scalars(&ca);
    let t = arm_scalars(&ta);
    let es = effect_sizes(&c, &t, true, 0.95, 42).expect("effect");
    StatisticalResult::new(
        Estimand::MeanPairedDifference(EstimandDef {
            outcome: "rating_accuracy".to_string(),
            treatment: "treatment".to_string(),
            control: "control".to_string(),
        }),
        es,
        control_means.len(),
        (control_means.len(), treatment_means.len()),
    )
}
#[test]
fn study_a_zero_effect() {
    let means = vec![100.0, 102.0, 101.0, 103.0, 100.0, 104.0, 101.0, 102.0];
    let sr = paired_study("A", &means, &means);
    assert!(
        sr.estimate.abs() < 1e-9,
        "zero effect: estimate = {}",
        sr.estimate
    );
    assert!(
        sr.uncertainty.lower <= 0.0 && sr.uncertainty.upper >= 0.0,
        "zero effect: CI must contain 0"
    );
}
#[test]
fn study_b_known_small_effect() {
    let control: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
    let treatment: Vec<f64> = control.iter().map(|v| v + 5.0).collect();
    let sr = paired_study("B", &control, &treatment);
    assert!(
        (sr.estimate - 5.0).abs() < 1e-9,
        "B: estimate = {}",
        sr.estimate
    );
    assert!(sr.uncertainty.lower > 0.0, "B: CI excludes 0");
    assert!(
        sr.effect_size.standardized.is_some(),
        "B: has standardized effect"
    );
}
#[test]
fn study_c_known_large_effect() {
    let control: Vec<f64> = (0..30).map(|i| 100.0 + i as f64).collect();
    let treatment: Vec<f64> = control.iter().map(|v| v + 20.0).collect();
    let sr = paired_study("C", &control, &treatment);
    assert!(
        (sr.estimate - 20.0).abs() < 1e-9,
        "C: estimate = {}",
        sr.estimate
    );
    assert!(sr.uncertainty.lower > 0.0, "C: CI excludes 0");
    assert!(sr.effect_size.absolute > 19.0, "C: effect large");
}
#[test]
fn study_d_heterogeneous_effect() {
    use matchlab_analysis::cohorts::analyze_heterogeneity;
    let low_ctrl: Vec<ReplicationScalar> = (0..10)
        .map(|i| per_replication_scalar(100.0 + i as f64))
        .collect();
    let low_trt: Vec<ReplicationScalar> = low_ctrl.clone();
    let high_ctrl: Vec<ReplicationScalar> = (0..10)
        .map(|i| per_replication_scalar(100.0 + i as f64))
        .collect();
    let high_trt: Vec<ReplicationScalar> = (0..10)
        .map(|i| per_replication_scalar(120.0 + i as f64))
        .collect();
    let cohorts = vec![
        ("low_vol".to_string(), low_ctrl, low_trt),
        ("high_vol".to_string(), high_ctrl, high_trt),
    ];
    let results = analyze_heterogeneity(&cohorts, true, 0.95, 42);
    assert_eq!(results.len(), 2, "D: two cohorts");
    let low = results.iter().find(|r| r.cohort == "low_vol").unwrap();
    let high = results.iter().find(|r| r.cohort == "high_vol").unwrap();
    assert!(low.effect.mean_delta.abs() < 1e-9, "D: low vol ≈ 0");
    assert!(
        (high.effect.mean_delta - 20.0).abs() < 1e-9,
        "D: high vol ≈ 20"
    );
}
fn per_replication_scalar(v: f64) -> ReplicationScalar {
    use matchlab_analysis::hierarchy::per_replication;
    use matchlab_metrics::MetricResult;
    per_replication::from_metric(&MetricResult::Scalar(v)).unwrap()
}
#[test]
fn study_e_paired_benefit() {
    let base: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect();
    let noise: Vec<f64> = (0..50).map(|i| (i as f64 * 0.1).sin()).collect();
    let treatment: Vec<f64> = base.iter().zip(&noise).map(|(b, n)| b + n + 5.0).collect();
    let ca = fake_arm("control", &base);
    let ta = fake_arm("treatment", &treatment);
    let c = arm_scalars(&ca);
    let t = arm_scalars(&ta);
    let paired = effect_sizes(&c, &t, true, 0.95, 42).expect("paired");
    let independent = effect_sizes(&c, &t, false, 0.95, 42).expect("independent");
    assert!(
        (paired.ci_hi - paired.ci_lo) < (independent.ci_hi - independent.ci_lo),
        "E: paired CI narrower than independent"
    );
    assert!(paired.ci_method == CiMethod::PairedBootstrap);
}
#[test]
fn study_f_heavy_tailed_outcome() {
    use matchlab_analysis::robustness::RobustnessSpec;
    let mut rng = SimRng::from_seed(42);
    let control: Vec<f64> = (0..40)
        .map(|_| (rng.sample_normal(0.0, 1.0)).exp() * 10.0)
        .collect();
    let treatment: Vec<f64> = control.iter().map(|v| v * 1.1).collect();
    let ca = fake_arm("control", &control);
    let ta = fake_arm("treatment", &treatment);
    let c = arm_scalars(&ca);
    let t = arm_scalars(&ta);
    let specs = vec![RobustnessSpec::MeanVsMedian];
    let report = run_robustness(&c, &t, &specs, 0.95, 42);
    assert_eq!(report.checks.len(), 1, "F: robustness check runs");
    assert!(
        report.checks[0].primary_delta > 0.0,
        "F: mean effect positive"
    );
}
#[test]
fn study_g_report_contract() {
    use matchlab_experiments::{SeedStrategy, StudyResult};
    let means_a = vec![100.0, 102.0, 101.0, 103.0];
    let means_b = vec![105.0, 107.0, 106.0, 108.0];
    let ca = fake_arm("elo", &means_a);
    let ta = fake_arm("glicko", &means_b);
    let study = StudyResult {
        study_id: "test-g".to_string(),
        name: "acceptance_g".to_string(),
        config_hash: "deadbeef".to_string(),
        git_commit: "abc123".to_string(),
        strategy: SeedStrategy::Crn,
        design: matchlab_experiments::DesignType::Paired,
        experimental_design: matchlab_experiments::ExperimentalDesign::default(),
        replication_count: 4,
        arms: vec![ca, ta],
    };
    let cfg = StudyReportConfig::default();
    let report = generate_research_report(&study, &cfg);
    assert!(report.contains("## Study"));
    assert!(report.contains("## Design"));
    assert!(report.contains("## Effect Estimates"));
    assert!(report.contains("## Reproducibility"));
    assert!(report.contains("No causal claim is made"));
    let report2 = generate_research_report(&study, &cfg);
    assert_eq!(report, report2);
}
#[test]
fn all_studies_are_deterministic() {
    let means = vec![100.0, 102.0, 101.0, 103.0, 100.0];
    let sr1 = paired_study("det", &means, &means);
    let sr2 = paired_study("det", &means, &means);
    assert_eq!(sr1.estimate, sr2.estimate);
    assert_eq!(sr1.uncertainty.lower, sr2.uncertainty.lower);
    assert_eq!(sr1.uncertainty.upper, sr2.uncertainty.upper);
}
