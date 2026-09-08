//! Statistical sanity tests: synthetic-data tests with known
//! statistical truth that verify the estimation pipeline recovers intended
//! quantities and doesn't silently misbehave on edge cases.
use matchlab_analysis::effect::{CiMethod, bootstrap_ci, effect_sizes, student_t_ci};
use matchlab_analysis::estimand::Estimand;
use matchlab_analysis::result::StatisticalResult;
use matchlab_core::rng::SimRng;
use matchlab_experiments::ExperimentResult;
use matchlab_experiments::replicate::{ArmResult, ReplicateResult};
use matchlab_metrics::MetricResult;
use std::collections::BTreeMap;
fn metric_result(mean: f64) -> MetricResult {
    MetricResult::Summary {
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
fn arm_scalars(arm: &ArmResult) -> Vec<matchlab_analysis::hierarchy::ReplicationScalar> {
    use matchlab_analysis::hierarchy::replication_scalars;
    replication_scalars(arm, "rating_accuracy")
        .iter()
        .filter_map(|o| o.replication_scalar())
        .collect()
}
#[test]
fn zero_effect_paired_and_independent() {
    let means = vec![100.0, 102.0, 101.0, 103.0, 100.0, 104.0, 101.0, 102.0];
    let control = fake_arm("a", &means);
    let treatment = fake_arm("b", &means);
    let c = arm_scalars(&control);
    let t = arm_scalars(&treatment);
    let es_paired = effect_sizes(&c, &t, true, 0.95, 42).expect("paired");
    assert!(
        es_paired.mean_delta.abs() < 1e-12,
        "zero effect: mean_delta = {}",
        es_paired.mean_delta
    );
    assert!(
        es_paired.ci_lo <= 0.0 && es_paired.ci_hi >= 0.0,
        "zero effect: CI must contain 0"
    );
    assert!(es_paired.ci_method == CiMethod::PairedBootstrap);
    let es_ind = effect_sizes(&c, &t, false, 0.95, 42).expect("independent");
    assert!(es_ind.mean_delta.abs() < 1e-12);
    assert!(es_ind.ci_lo <= 0.0 && es_ind.ci_hi >= 0.0);
    assert!(
        es_ind.ci_method == CiMethod::Bootstrap,
        "n=8 → bootstrap for independent, got {:?}",
        es_ind.ci_method
    );
    let es2 = effect_sizes(&c, &t, true, 0.95, 42).expect("determinism");
    assert_eq!(es_paired.ci_lo, es2.ci_lo);
    assert_eq!(es_paired.ci_hi, es2.ci_hi);
}
#[test]
fn known_effect_constant_shift() {
    let control: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
    let treatment: Vec<f64> = control.iter().map(|v| v + 10.0).collect();
    let ca = fake_arm("ctrl", &control);
    let ta = fake_arm("trt", &treatment);
    let c = arm_scalars(&ca);
    let t = arm_scalars(&ta);
    let es = effect_sizes(&c, &t, true, 0.95, 42).expect("effect");
    assert!(
        (es.mean_delta - 10.0).abs() < 1e-9,
        "estimate = {}",
        es.mean_delta
    );
    assert!(
        es.ci_lo > 9.0 && es.ci_hi < 11.0,
        "tight CI about 10: [{}, {}]",
        es.ci_lo,
        es.ci_hi
    );
    assert!(es.ci_lo > 0.0, "CI excludes 0");
    assert!(es.relative_diff > 0.0, "positive relative diff");
    assert!(es.percent_improvement > 0.0, "positive percent improvement");
    assert!(es.d_z.is_none(), "constant shift → d_z undefined");
    let sr = StatisticalResult::new(
        Estimand::MeanPairedDifference(matchlab_analysis::estimand::EstimandDef {
            outcome: "rating_accuracy".to_string(),
            treatment: "trt".to_string(),
            control: "ctrl".to_string(),
        }),
        es,
        20,
        (20, 20),
    );
    assert_eq!(sr.sample.replications, 20);
    assert!((sr.estimate - 10.0).abs() < 1e-9);
}
#[test]
fn ci_width_non_increasing_with_n() {
    let generate = |n: usize, rng_seed: u64| -> (Vec<f64>, Vec<f64>) {
        let mut rng = SimRng::from_seed(rng_seed);
        let c: Vec<f64> = (0..n).map(|_| rng.sample_normal(100.0, 10.0)).collect();
        let t: Vec<f64> = c.iter().map(|v| v + 5.0).collect();
        (c, t)
    };
    let ns = [10, 30, 50, 100];
    let mut widths = Vec::new();
    for &n in &ns {
        let (c, t) = generate(n, 777);
        let ci = student_t_ci(&c, 0.95);
        let ci_t = student_t_ci(&t, 0.95);
        widths.push((ci.upper - ci.lower + ci_t.upper - ci_t.lower) / 2.0);
    }
    let mut violations = 0;
    for w in widths.windows(2) {
        if w[1] > w[0] + 0.1 {
            violations += 1;
        }
    }
    assert!(
        violations <= 1,
        "CI width should generally decrease with N: {:?}",
        widths
    );
}
#[test]
fn heavy_tail_bootstrap_finite_and_sane() {
    let mut rng = SimRng::from_seed(42);
    let samples: Vec<f64> = (0..40)
        .map(|_| (rng.sample_normal(0.0, 0.8)).exp() * 10.0)
        .collect();
    let ci = bootstrap_ci(&samples, 0.95, 99);
    assert!(ci.lower.is_finite(), "lower bound finite");
    assert!(ci.upper.is_finite(), "upper bound finite");
    assert!(ci.lower < ci.upper, "lower < upper");
    assert!(ci.method == CiMethod::Bootstrap);
    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = (sorted[19] + sorted[20]) / 2.0;
    assert!(median > 0.0, "median of log-normal is positive");
}
#[test]
fn paired_ci_substantially_tighter_on_correlated_arms() {
    let base_rng_seed = 55;
    let n = 50;
    let mut rng = SimRng::from_seed(base_rng_seed);
    let base: Vec<f64> = (0..n).map(|_| rng.sample_normal(100.0, 15.0)).collect();
    let noise: Vec<f64> = (0..n).map(|_| rng.sample_normal(0.0, 2.0)).collect();
    let treatment: Vec<f64> = base.iter().zip(&noise).map(|(b, e)| b + e + 8.0).collect();
    let ca = fake_arm("ctrl", &base);
    let ta = fake_arm("trt", &treatment);
    let c = arm_scalars(&ca);
    let t = arm_scalars(&ta);
    let paired = effect_sizes(&c, &t, true, 0.95, 42).expect("paired");
    let independent = effect_sizes(&c, &t, false, 0.95, 42).expect("independent");
    let paired_width = paired.ci_hi - paired.ci_lo;
    let ind_width = independent.ci_hi - independent.ci_lo;
    assert!(
        ind_width > paired_width * 2.0,
        "paired CI ({paired_width:.3}) must be ≥ 2× tighter than independent ({ind_width:.3})"
    );
    assert!(paired.ci_method == CiMethod::PairedBootstrap);
    assert!(
        independent.ci_method == CiMethod::WelchT,
        "n=50 → Welch for independent, got {:?}",
        independent.ci_method
    );
    assert!(paired.ci_lo > 0.0, "paired CI excludes 0");
    assert!(independent.ci_lo > 0.0, "independent CI excludes 0");
}
#[test]
fn paired_design_refuses_unequal_arm_lengths() {
    let control = vec![
        matchlab_analysis::hierarchy::per_replication::from_metric(&MetricResult::Scalar(1.0))
            .unwrap(),
    ];
    let treatment = vec![
        matchlab_analysis::hierarchy::per_replication::from_metric(&MetricResult::Scalar(2.0))
            .unwrap(),
        matchlab_analysis::hierarchy::per_replication::from_metric(&MetricResult::Scalar(3.0))
            .unwrap(),
    ];
    let err = effect_sizes(&control, &treatment, true, 0.95, 1).expect_err("must fail");
    assert!(err.contains("equal arm lengths"));
}
#[test]
fn determinism_same_seed_same_result() {
    let means_a = vec![100.0, 102.0, 101.0, 103.0, 100.0];
    let means_b = vec![105.0, 107.0, 106.0, 108.0, 105.0];
    let ca = fake_arm("a", &means_a);
    let ta = fake_arm("b", &means_b);
    let c = arm_scalars(&ca);
    let t = arm_scalars(&ta);
    let es1 = effect_sizes(&c, &t, true, 0.95, 42).expect("first");
    let es2 = effect_sizes(&c, &t, true, 0.95, 42).expect("second");
    assert_eq!(es1.mean_delta, es2.mean_delta);
    assert_eq!(es1.ci_lo, es2.ci_lo);
    assert_eq!(es1.ci_hi, es2.ci_hi);
    assert_eq!(es1.cohen_d, es2.cohen_d);
    assert_eq!(es1.ci_method, es2.ci_method);
    let es3 = effect_sizes(&c, &t, false, 0.95, 42).expect("third");
    let es4 = effect_sizes(&c, &t, false, 0.95, 42).expect("fourth");
    assert_eq!(es3.ci_lo, es4.ci_lo);
    assert_eq!(es3.ci_hi, es4.ci_hi);
    let ci1 = bootstrap_ci(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.95, 99);
    let ci2 = bootstrap_ci(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.95, 99);
    assert_eq!(ci1.lower, ci2.lower);
    assert_eq!(ci1.upper, ci2.upper);
    let ci3 = student_t_ci(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.95);
    let ci4 = student_t_ci(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.95);
    assert_eq!(ci3.lower, ci4.lower);
    assert_eq!(ci3.upper, ci4.upper);
}
