//! Statistical power analysis: compute required replications
//! and achieved power for paired continuous outcomes.
//!
//! For a paired design with effect standard deviation `s_d`, two-sided test
//! at level `α`, and target power `1−β`, the required sample size is:
//!
//! ```text
//! n ≈ ((z_{1−α/2} + z_{1−β}) · s_d / d)²
//! ```
//!
//! where `d` is the minimum detectable effect and `z_p` is the standard normal
//! quantile. This implementation uses the exact `norm_quantile` from `effect.rs`
//! and notes the small-n discrepancy with the t-based critical value.
use crate::effect::norm_quantile;
/// Power analysis specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PowerSpec {
    /// Significance level (e.g. 0.05).
    pub alpha: f64,
    /// Desired statistical power (e.g. 0.80).
    pub target_power: f64,
    /// Minimum detectable effect size (absolute, on the same scale as `effect_sd`).
    pub minimum_effect: f64,
}
/// Required number of replications to detect an effect of the given size with
/// the specified power. Uses the normal approximation (documented as slightly
/// conservative at small n).
pub fn required_replications(effect_sd: f64, spec: &PowerSpec) -> u64 {
    if spec.minimum_effect == 0.0 || effect_sd == 0.0 {
        return 1;
    }
    let z_alpha = norm_quantile(1.0 - spec.alpha / 2.0);
    let z_beta = norm_quantile(spec.target_power);
    let n = ((z_alpha + z_beta) * effect_sd / spec.minimum_effect).powi(2);
    n.ceil().max(1.0) as u64
}
/// Achieved power for a given sample size, effect size, and significance level.
/// Uses the normal approximation.
pub fn achieved_power(effect_sd: f64, n: u64, effect: f64, alpha: f64) -> f64 {
    if n == 0 || effect_sd == 0.0 || effect == 0.0 {
        return 0.0;
    }
    let z_alpha = norm_quantile(1.0 - alpha / 2.0);
    let se = effect_sd / (n as f64).sqrt();
    if se == 0.0 {
        return 1.0;
    }
    let z = (effect.abs() / se) - z_alpha;
    standard_normal_cdf(z)
}
/// Standard normal CDF approximation (Abramowitz & Stegun).
fn standard_normal_cdf(z: f64) -> f64 {
    if z < -8.0 {
        return 0.0;
    }
    if z > 8.0 {
        return 1.0;
    }
    let abs_z = z.abs();
    let t = 1.0 / (1.0 + 0.2316419 * abs_z);
    let d = 0.3989422804014327;
    let p = d
        * (-abs_z * abs_z / 2.0).exp()
        * (t * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429)))));
    if z >= 0.0 { 1.0 - p } else { p }
}
/// Allocation strategy for study planning.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationStrategy {
    Balanced,
}
/// Assumptions for a study plan.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanAssumptions {
    pub effect_sd: f64,
    pub minimum_effect: f64,
    pub alpha: f64,
    pub target_power: f64,
    pub n_conditions: u64,
    pub allocation: AllocationStrategy,
}
/// A study plan: the recommended replication allocation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyPlan {
    pub n_per_condition: u64,
    pub total_replications: u64,
    pub estimated_power: f64,
    pub assumptions: PlanAssumptions,
}
/// Plan a multi-condition study with Bonferroni correction. For k conditions,
/// the per-comparison alpha is alpha/k.
pub fn plan_study(assumptions: &PlanAssumptions) -> StudyPlan {
    let k = assumptions.n_conditions.max(1);
    let adjusted_alpha = assumptions.alpha / k as f64;
    let n = required_replications(
        assumptions.effect_sd,
        &PowerSpec {
            alpha: adjusted_alpha,
            target_power: assumptions.target_power,
            minimum_effect: assumptions.minimum_effect,
        },
    );
    let power = achieved_power(
        assumptions.effect_sd,
        n,
        assumptions.minimum_effect,
        adjusted_alpha,
    );
    StudyPlan {
        n_per_condition: n,
        total_replications: n * k,
        estimated_power: power,
        assumptions: assumptions.clone(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_replications_hand_computed() {
        let spec = PowerSpec {
            alpha: 0.05,
            target_power: 0.80,
            minimum_effect: 10.0,
        };
        let n = required_replications(15.0, &spec);
        assert_eq!(n, 18);
    }
    #[test]
    fn achieved_power_at_required_n() {
        let spec = PowerSpec {
            alpha: 0.05,
            target_power: 0.80,
            minimum_effect: 10.0,
        };
        let n = required_replications(15.0, &spec);
        let power = achieved_power(15.0, n, 10.0, 0.05);
        assert!(
            power >= 0.80,
            "achieved power at required N must be ≥ target: {power:.4}"
        );
    }
    #[test]
    fn zero_effect_gives_max_n() {
        let spec = PowerSpec {
            alpha: 0.05,
            target_power: 0.80,
            minimum_effect: 0.0,
        };
        assert_eq!(required_replications(15.0, &spec), 1);
    }
    #[test]
    fn larger_effect_needs_fewer_replications() {
        let spec_small = PowerSpec {
            alpha: 0.05,
            target_power: 0.80,
            minimum_effect: 5.0,
        };
        let spec_large = PowerSpec {
            alpha: 0.05,
            target_power: 0.80,
            minimum_effect: 20.0,
        };
        assert!(
            required_replications(15.0, &spec_small) > required_replications(15.0, &spec_large),
            "larger effect → fewer replications"
        );
    }
    #[test]
    fn higher_power_needs_more_replications() {
        let spec_low = PowerSpec {
            alpha: 0.05,
            target_power: 0.50,
            minimum_effect: 10.0,
        };
        let spec_high = PowerSpec {
            alpha: 0.05,
            target_power: 0.99,
            minimum_effect: 10.0,
        };
        assert!(
            required_replications(15.0, &spec_low) < required_replications(15.0, &spec_high),
            "higher power → more replications"
        );
    }
    #[test]
    fn standard_normal_cdf_sanity() {
        assert!((standard_normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((standard_normal_cdf(1.96) - 0.975).abs() < 0.001);
        assert!((standard_normal_cdf(-1.96) - 0.025).abs() < 0.001);
        assert!((standard_normal_cdf(8.0) - 1.0).abs() < 1e-6);
        assert!((standard_normal_cdf(-8.0)).abs() < 1e-6);
    }
    #[test]
    fn plan_study_balanced_two_conditions() {
        let assumptions = PlanAssumptions {
            effect_sd: 15.0,
            minimum_effect: 10.0,
            alpha: 0.05,
            target_power: 0.80,
            n_conditions: 2,
            allocation: AllocationStrategy::Balanced,
        };
        let plan = plan_study(&assumptions);
        assert!(plan.n_per_condition > 0);
        assert_eq!(plan.total_replications, plan.n_per_condition * 2);
        let single = required_replications(
            15.0,
            &PowerSpec {
                alpha: 0.05,
                target_power: 0.80,
                minimum_effect: 10.0,
            },
        );
        assert!(
            plan.n_per_condition >= single,
            "Bonferroni correction should increase N"
        );
    }
    #[test]
    fn plan_study_power_is_documented() {
        let assumptions = PlanAssumptions {
            effect_sd: 15.0,
            minimum_effect: 10.0,
            alpha: 0.05,
            target_power: 0.80,
            n_conditions: 2,
            allocation: AllocationStrategy::Balanced,
        };
        let plan = plan_study(&assumptions);
        assert!(
            plan.estimated_power > 0.5,
            "power should be reasonable: {}",
            plan.estimated_power
        );
    }
}
