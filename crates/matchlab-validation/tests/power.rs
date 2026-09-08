//! Power sanity test: verify the analytic power formula against
//! empirical rejection rates on synthetic paired data.
use matchlab_analysis::power::{PowerSpec, achieved_power, required_replications};
use matchlab_core::rng::SimRng;
/// Empirically estimate power by rejecting H₀ (paired t-test) at `alpha`
/// on `B` synthetic paired datasets with true difference `true_delta` and
/// paired-std `s_d`.
fn empirical_power(true_delta: f64, s_d: f64, n: usize, alpha: f64, b: usize, seed: u64) -> f64 {
    use matchlab_analysis::effect::paired_t_pvalue;
    let mut rejections = 0usize;
    for i in 0..b {
        let mut rng = SimRng::from_seed(seed.wrapping_add(i as u64));
        let deltas: Vec<f64> = (0..n)
            .map(|_| true_delta + rng.sample_normal(0.0, s_d))
            .collect();
        let p = paired_t_pvalue(&deltas);
        if p < alpha {
            rejections += 1;
        }
    }
    rejections as f64 / b as f64
}
#[test]
fn analytic_power_matches_empirical_at_required_n() {
    let true_delta = 10.0;
    let s_d = 15.0;
    let alpha = 0.05;
    let spec = PowerSpec {
        alpha,
        target_power: 0.80,
        minimum_effect: true_delta,
    };
    let n = required_replications(s_d, &spec) as usize;
    let analytic = achieved_power(s_d, n as u64, true_delta, alpha);
    let empirical = empirical_power(true_delta, s_d, n, alpha, 2000, 33_000);
    assert!(
        (empirical - analytic).abs() < 0.10,
        "empirical power {empirical:.4} vs analytic {analytic:.4} at n={n} (tolerance ±0.10)"
    );
    assert!(
        empirical > 0.65,
        "empirical power should be reasonable at required n: {empirical:.4}"
    );
}
#[test]
fn larger_n_increases_power() {
    let true_delta = 10.0;
    let s_d = 15.0;
    let power_small = empirical_power(true_delta, s_d, 10, 0.05, 1000, 33_100);
    let power_large = empirical_power(true_delta, s_d, 100, 0.05, 1000, 33_200);
    assert!(
        power_large > power_small,
        "larger n → higher power: {power_large:.4} > {power_small:.4}"
    );
}
#[test]
fn zero_effect_gives_zero_power() {
    let power = achieved_power(15.0, 50, 0.0, 0.05);
    assert_eq!(power, 0.0);
}
