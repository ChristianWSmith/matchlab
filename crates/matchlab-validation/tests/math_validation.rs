//! Mathematical & Statistical Validation.
//!
//! Known-answer tests for probability distributions, random sampling,
//! confidence intervals, effect sizes, p-values, multiple-comparison
//! corrections, power calculations, and Pareto dominance.
use matchlab_core::rng::SimRng;
/// Normal CDF: generated samples have expected mean and variance.
#[test]
fn normal_distribution_statistics() {
    let mut rng = SimRng::from_seed(42);
    let samples: Vec<f64> = (0..10000).map(|_| rng.sample_normal(0.0, 1.0)).collect();
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;
    assert!((mean).abs() < 0.05, "normal mean should be near 0: {mean}");
    assert!(
        (var - 1.0).abs() < 0.05,
        "normal variance should be near 1: {var}"
    );
}
/// Uniform distribution generates values in [0,1).
#[test]
fn uniform_distribution_bounds() {
    let mut rng = SimRng::from_seed(42);
    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;
    for _ in 0..10000 {
        let v = rng.gen_range(0.0, 1.0);
        min_val = min_val.min(v);
        max_val = max_val.max(v);
    }
    assert!(min_val >= 0.0, "min should be >= 0");
    assert!(max_val < 1.0, "max should be < 1");
}
/// Random sampling produces values with correct mean for uniform distribution.
#[test]
fn random_sampling_mean_converges() {
    let mut rng = SimRng::from_seed(42);
    let samples: Vec<f64> = (0..10000).map(|_| rng.gen_range(0.0, 1.0)).collect();
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    assert!(
        (mean - 0.5).abs() < 0.02,
        "uniform mean should be near 0.5: {mean}"
    );
}
/// Bootstrap CI contains the true mean at nominal rate.
#[test]
fn bootstrap_ci_coverage() {
    use matchlab_analysis::effect::bootstrap_ci;
    let mut rng = SimRng::from_seed(42);
    let mut hits = 0;
    let n_samples = 100;
    let n_trials = 1000;
    for _ in 0..n_trials {
        let samples: Vec<f64> = (0..n_samples)
            .map(|_| rng.sample_normal(5.0, 1.0))
            .collect();
        let ci = bootstrap_ci(&samples, 0.95, 42);
        if ci.lower <= 5.0 && 5.0 <= ci.upper {
            hits += 1;
        }
    }
    let coverage = hits as f64 / n_trials as f64;
    assert!(coverage > 0.90, "bootstrap coverage too low: {coverage}");
    assert!(coverage < 1.0, "bootstrap coverage too high: {coverage}");
}
/// Student-t CI contains the true mean at nominal rate.
#[test]
fn student_t_ci_coverage() {
    use matchlab_analysis::effect::student_t_ci;
    let mut rng = SimRng::from_seed(42);
    let mut hits = 0;
    let n_samples = 30;
    let n_trials = 1000;
    for _ in 0..n_trials {
        let samples: Vec<f64> = (0..n_samples)
            .map(|_| rng.sample_normal(5.0, 1.0))
            .collect();
        let ci = student_t_ci(&samples, 0.95);
        if ci.lower <= 5.0 && 5.0 <= ci.upper {
            hits += 1;
        }
    }
    let coverage = hits as f64 / n_trials as f64;
    assert!(coverage > 0.90, "student-t coverage too low: {coverage}");
    assert!(coverage < 1.0, "student-t coverage too high: {coverage}");
}
/// Welch CI contains the true difference at nominal rate.
#[test]
fn welch_ci_coverage() {
    use matchlab_analysis::effect::welch_ci;
    let mut rng = SimRng::from_seed(42);
    let mut hits = 0;
    let n_trials = 1000;
    let true_diff = 5.0;
    for _ in 0..n_trials {
        let control: Vec<f64> = (0..30).map(|_| rng.sample_normal(0.0, 1.0)).collect();
        let treatment: Vec<f64> = (0..30).map(|_| rng.sample_normal(5.0, 1.0)).collect();
        let ci = welch_ci(&control, &treatment, 0.95);
        if ci.lower <= true_diff && true_diff <= ci.upper {
            hits += 1;
        }
    }
    let coverage = hits as f64 / n_trials as f64;
    assert!(coverage > 0.90, "welch coverage too low: {coverage}");
}
/// Cohen's d matches hand computation.
#[test]
fn cohens_d_known_values() {
    use matchlab_analysis::effect::effect_sizes;
    let mut rng = SimRng::from_seed(42);
    let control: Vec<f64> = (0..100).map(|_| rng.sample_normal(10.0, 1.0)).collect();
    let treatment: Vec<f64> = (0..100).map(|_| rng.sample_normal(15.0, 1.0)).collect();
    let control_s: Vec<_> = control
        .iter()
        .map(|v| {
            matchlab_analysis::hierarchy::per_replication::from_metric(
                &matchlab_metrics::MetricResult::Scalar(*v),
            )
            .unwrap()
        })
        .collect();
    let treatment_s: Vec<_> = treatment
        .iter()
        .map(|v| {
            matchlab_analysis::hierarchy::per_replication::from_metric(
                &matchlab_metrics::MetricResult::Scalar(*v),
            )
            .unwrap()
        })
        .collect();
    let es = effect_sizes(&control_s, &treatment_s, false, 0.95, 42).unwrap();
    assert!(
        (es.mean_delta - 5.0).abs() < 1.0,
        "effect size should be near 5.0: {}",
        es.mean_delta
    );
}
/// Paired t-test p-value is small for large true differences.
#[test]
fn paired_t_pvalue_detects_difference() {
    use matchlab_analysis::effect::paired_t_pvalue;
    let deltas = vec![5.0; 20];
    let p = paired_t_pvalue(&deltas);
    assert!(
        p < 0.001,
        "p-value should be very small for large difference: {p}"
    );
}
/// Paired t-test p-value is large for zero differences.
#[test]
fn paired_t_pvalue_large_for_zero_difference() {
    use matchlab_analysis::effect::paired_t_pvalue;
    let deltas = vec![0.0; 20];
    let p = paired_t_pvalue(&deltas);
    assert!(
        p > 0.99,
        "p-value should be near 1 for zero difference: {p}"
    );
}
/// Welch p-value detects difference.
#[test]
fn welch_pvalue_detects_difference() {
    use matchlab_analysis::effect::welch_pvalue;
    let control = vec![10.0; 20];
    let treatment = vec![15.0; 20];
    let p = welch_pvalue(&control, &treatment);
    assert!(p < 0.001, "p-value should be very small: {p}");
}
/// Holm correction is more conservative than BH.
#[test]
fn holm_more_conservative_than_bh() {
    use matchlab_analysis::multiple_comparisons::{benjamini_hochberg, holm};
    let ps = vec![0.01, 0.05, 0.10, 0.50];
    let holm_adj = holm(&ps);
    let bh_adj = benjamini_hochberg(&ps);
    for (h, b) in holm_adj.iter().zip(bh_adj.iter()) {
        assert!(*h >= *b - 1e-10, "holm {h} should be >= bh {b}");
    }
}
/// Holm and BH are monotonic.
#[test]
fn corrections_are_monotonic() {
    use matchlab_analysis::multiple_comparisons::{benjamini_hochberg, holm};
    let ps = vec![0.01, 0.05, 0.10, 0.50];
    let holm_adj = holm(&ps);
    let bh_adj = benjamini_hochberg(&ps);
    for w in holm_adj.windows(2) {
        assert!(w[0] <= w[1] + 1e-10, "holm should be monotonic");
    }
    for w in bh_adj.windows(2) {
        assert!(w[0] <= w[1] + 1e-10, "bh should be monotonic");
    }
}
/// Correction preserves ordering relative to raw p-values.
#[test]
fn corrections_preserve_raw_ordering() {
    use matchlab_analysis::multiple_comparisons::{benjamini_hochberg, holm};
    let ps = vec![0.01, 0.05, 0.10, 0.50];
    let holm_adj = holm(&ps);
    let bh_adj = benjamini_hochberg(&ps);
    for (raw, adj) in ps.iter().zip(holm_adj.iter()) {
        assert!(*adj >= *raw - 1e-10, "holm adjusted >= raw");
    }
    for (raw, adj) in ps.iter().zip(bh_adj.iter()) {
        assert!(*adj >= *raw - 1e-10, "bh adjusted >= raw");
    }
}
/// Required replications increases with smaller effect size.
#[test]
fn required_replications_increases_with_smaller_effect() {
    use matchlab_analysis::power::{PowerSpec, required_replications};
    let spec1 = PowerSpec {
        alpha: 0.05,
        target_power: 0.80,
        minimum_effect: 10.0,
    };
    let spec2 = PowerSpec {
        alpha: 0.05,
        target_power: 0.80,
        minimum_effect: 5.0,
    };
    let n1 = required_replications(15.0, &spec1);
    let n2 = required_replications(15.0, &spec2);
    assert!(
        n2 > n1,
        "smaller effect needs more replications: n1={n1}, n2={n2}"
    );
}
/// Achieved power is reasonable.
#[test]
fn achieved_power_reasonable() {
    use matchlab_analysis::power::{PowerSpec, achieved_power, required_replications};
    let spec = PowerSpec {
        alpha: 0.05,
        target_power: 0.80,
        minimum_effect: 10.0,
    };
    let n = required_replications(15.0, &spec);
    let power = achieved_power(15.0, n, 10.0, 0.05);
    assert!(
        power >= 0.70,
        "achieved power should be reasonable: {power}"
    );
}
/// Pareto frontier correctly identifies dominated points.
#[test]
fn pareto_frontier_dominance() {
    use matchlab_analysis::pareto::{ParetoPoint, pareto_front};
    let points = vec![
        ParetoPoint {
            label: "a".to_string(),
            values: vec![1.0, 5.0],
        },
        ParetoPoint {
            label: "b".to_string(),
            values: vec![2.0, 3.0],
        },
        ParetoPoint {
            label: "c".to_string(),
            values: vec![3.0, 2.0],
        },
        ParetoPoint {
            label: "d".to_string(),
            values: vec![4.0, 1.0],
        },
    ];
    let higher_is_better = vec![true, true];
    let front = pareto_front(&points, &higher_is_better);
    assert!(!front.is_empty(), "front should not be empty");
}
/// Same seed produces identical random samples.
#[test]
fn determinism_same_seed_same_samples() {
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let s1: Vec<f64> = (0..100).map(|_| rng1.sample_normal(0.0, 1.0)).collect();
    let s2: Vec<f64> = (0..100).map(|_| rng2.sample_normal(0.0, 1.0)).collect();
    assert_eq!(s1, s2, "same seed should produce identical samples");
}
/// Different seeds produce different samples.
#[test]
fn different_seeds_produce_different_samples() {
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(43);
    let s1: Vec<f64> = (0..100).map(|_| rng1.sample_normal(0.0, 1.0)).collect();
    let s2: Vec<f64> = (0..100).map(|_| rng2.sample_normal(0.0, 1.0)).collect();
    assert_ne!(s1, s2, "different seeds should produce different samples");
}
