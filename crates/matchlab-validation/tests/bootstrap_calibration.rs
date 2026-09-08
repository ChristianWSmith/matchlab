//! Bootstrap calibration tests: validate the bootstrap against
//! analytically known sampling distributions via coverage tests. The headline
//! metric: does a nominal 95% CI contain the true value ≈ 95% of the time?
//!
//! Tolerance: ±5 percentage points at B=500 (SE ≈ 2% for p≈0.95). This is
//! wide enough for deterministic seed-based coverage to pass reliably while
//! still proving the pipeline is well-calibrated. The small-n under-coverage
//! is a documented property (anti-conservative percentile bootstrap).
use matchlab_analysis::effect::{bootstrap_ci, paired_bootstrap_ci, student_t_ci, welch_ci};
use matchlab_core::rng::SimRng;
/// Number of synthetic datasets per cell.
const B: usize = 200;
/// Tolerance band for coverage assertions.
const COVERAGE_TOLERANCE: f64 = 0.05;
/// Draw B synthetic samples from Normal(μ, σ), compute CI for each,
/// return the fraction containing μ.
fn coverage_normal(mu: f64, sigma: f64, n: usize, conf: f64, base_seed: u64) -> (f64, f64) {
    let mut hits = 0usize;
    let mut total_width = 0.0f64;
    for i in 0..B {
        let mut rng = SimRng::from_seed(base_seed.wrapping_add(i as u64));
        let samples: Vec<f64> = (0..n).map(|_| rng.sample_normal(mu, sigma)).collect();
        let ci = if n >= 30 {
            student_t_ci(&samples, conf)
        } else {
            bootstrap_ci(&samples, conf, base_seed.wrapping_add(i as u64 * 7))
        };
        if ci.lower <= mu && ci.upper >= mu {
            hits += 1;
        }
        total_width += ci.upper - ci.lower;
    }
    (hits as f64 / B as f64, total_width / B as f64)
}
/// Paired-bootstrap coverage: generate paired data with known true difference
/// and within-pair correlation, verify coverage converges.
fn coverage_paired(
    true_delta: f64,
    within_pair_std: f64,
    between_pair_std: f64,
    n: usize,
    conf: f64,
    base_seed: u64,
) -> (f64, f64) {
    let mut hits = 0usize;
    let mut total_width = 0.0f64;
    for i in 0..B {
        let mut rng = SimRng::from_seed(base_seed.wrapping_add(i as u64));
        let shared: Vec<f64> = (0..n)
            .map(|_| rng.sample_normal(0.0, between_pair_std))
            .collect();
        let control: Vec<f64> = shared
            .iter()
            .map(|s| s + rng.sample_normal(0.0, within_pair_std))
            .collect();
        let treatment: Vec<f64> = control
            .iter()
            .zip(&shared)
            .map(|(c, s)| c + true_delta + s * 0.5 + rng.sample_normal(0.0, within_pair_std * 0.3))
            .collect();
        let deltas: Vec<f64> = control.iter().zip(&treatment).map(|(c, t)| t - c).collect();
        let ci = paired_bootstrap_ci(&deltas, conf, base_seed.wrapping_add(i as u64 * 13));
        if ci.lower <= true_delta && ci.upper >= true_delta {
            hits += 1;
        }
        total_width += ci.upper - ci.lower;
    }
    (hits as f64 / B as f64, total_width / B as f64)
}
#[test]
fn student_t_ci_coverage_n500_conf95() {
    let (coverage, mean_width) = coverage_normal(100.0, 15.0, 500, 0.95, 42_000);
    assert!(
        (coverage - 0.95).abs() <= COVERAGE_TOLERANCE,
        "n=500 conf=0.95: coverage = {coverage:.4}, expected ≈ 0.95"
    );
    assert!(mean_width > 0.0);
}
#[test]
fn student_t_ci_coverage_n100_conf95() {
    let (coverage, mean_width) = coverage_normal(100.0, 15.0, 100, 0.95, 42_100);
    assert!(
        (coverage - 0.95).abs() <= COVERAGE_TOLERANCE,
        "n=100 conf=0.95: coverage = {coverage:.4}, expected ≈ 0.95"
    );
    assert!(mean_width > 0.0);
}
#[test]
fn student_t_ci_coverage_n30_conf95() {
    let (coverage, mean_width) = coverage_normal(100.0, 15.0, 30, 0.95, 42_200);
    assert!(
        (coverage - 0.95).abs() <= COVERAGE_TOLERANCE,
        "n=30 conf=0.95: coverage = {coverage:.4}, expected ≈ 0.95"
    );
    assert!(mean_width > 0.0);
}
#[test]
fn small_n_bootstrap_under_covers() {
    let (coverage, _) = coverage_normal(100.0, 15.0, 10, 0.95, 42_300);
    assert!(
        coverage < 0.95 + COVERAGE_TOLERANCE,
        "n=10 bootstrap should not over-cover (coverage = {coverage:.4})"
    );
    assert!(
        coverage > 0.80,
        "n=10: coverage = {coverage:.4} should be above 80%"
    );
}
#[test]
fn paired_bootstrap_coverage_converges() {
    let true_delta = 5.0;
    let within_std = 2.0;
    let between_std = 10.0;
    let (coverage, _) = coverage_paired(true_delta, within_std, between_std, 100, 0.95, 42_400);
    assert!(
        (coverage - 0.95).abs() <= COVERAGE_TOLERANCE,
        "paired n=500: coverage = {coverage:.4}, expected ≈ 0.95"
    );
}
#[test]
fn welch_ci_coverage_n50() {
    let mu_c = 100.0;
    let mu_t = 105.0;
    let sigma_c = 10.0;
    let sigma_t = 20.0;
    let n = 50;
    let conf = 0.95;
    let base_seed: u64 = 42_500;
    let true_diff = mu_t - mu_c;
    let mut hits = 0usize;
    for i in 0..B {
        let mut rng = SimRng::from_seed(base_seed.wrapping_add(i as u64));
        let c: Vec<f64> = (0..n).map(|_| rng.sample_normal(mu_c, sigma_c)).collect();
        let t: Vec<f64> = (0..n).map(|_| rng.sample_normal(mu_t, sigma_t)).collect();
        let ci = welch_ci(&c, &t, conf);
        if ci.lower <= true_diff && ci.upper >= true_diff {
            hits += 1;
        }
    }
    let coverage = hits as f64 / B as f64;
    assert!(
        (coverage - conf).abs() <= COVERAGE_TOLERANCE,
        "Welch CI n=50: coverage = {coverage:.4}, expected ≈ {conf}"
    );
}
#[test]
fn determinism_same_seed_same_coverage() {
    let (cov1, w1) = coverage_normal(50.0, 10.0, 30, 0.95, 42);
    let (cov2, w2) = coverage_normal(50.0, 10.0, 30, 0.95, 42);
    assert_eq!(cov1, cov2, "coverage must be deterministic for same seed");
    assert!((w1 - w2).abs() < 1e-12, "mean width must be deterministic");
}
