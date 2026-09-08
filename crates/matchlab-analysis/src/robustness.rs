//! Robustness checks: verify that a study's conclusion survives
//! specified analytical alternatives (mean vs median, CI method swap, etc.).
use crate::effect::{effect_sizes, paired_t_pvalue};
use crate::hierarchy::ReplicationScalar;
/// What category of robustness check this is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustnessCategory {
    Estimand,
    CiMethod,
    Cohort,
    TimeWindow,
}
/// A single robustness check comparing primary vs alternative estimation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RobustnessCheck {
    pub name: String,
    pub passed: bool,
    pub primary_delta: f64,
    pub alternative_delta: f64,
    pub category: RobustnessCategory,
}
/// Full robustness report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RobustnessReport {
    pub checks: Vec<RobustnessCheck>,
    pub all_passed: bool,
}
/// A declared robustness check type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustnessSpec {
    MeanVsMedian,
    CiMethod,
    PairedVsIndependent,
}
/// Run all declared robustness checks on paired control/treatment scalars.
pub fn run_robustness(
    control: &[ReplicationScalar],
    treatment: &[ReplicationScalar],
    specs: &[RobustnessSpec],
    conf: f64,
    seed: u64,
) -> RobustnessReport {
    let mut checks = Vec::new();
    let primary = effect_sizes(control, treatment, true, conf, seed).ok();
    let primary_delta = primary.as_ref().map(|e| e.mean_delta).unwrap_or(0.0);
    for spec in specs {
        match spec {
            RobustnessSpec::MeanVsMedian => {
                let deltas: Vec<f64> = control
                    .iter()
                    .zip(treatment)
                    .map(|(c, t)| t.value() - c.value())
                    .collect();
                let mut sorted = deltas.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let median_delta = if sorted.len() % 2 == 0 {
                    (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
                } else {
                    sorted[sorted.len() / 2]
                };
                let passed = primary_delta.signum() == median_delta.signum();
                checks.push(RobustnessCheck {
                    name: "mean_vs_median".to_string(),
                    passed,
                    primary_delta,
                    alternative_delta: median_delta,
                    category: RobustnessCategory::Estimand,
                });
            }
            RobustnessSpec::CiMethod => {
                let p = paired_t_pvalue(
                    &control
                        .iter()
                        .zip(treatment)
                        .map(|(c, t)| t.value() - c.value())
                        .collect::<Vec<_>>(),
                );
                let alt_sign = if p < conf {
                    primary_delta.signum()
                } else {
                    0.0
                };
                let passed = alt_sign == primary_delta.signum() || primary_delta == 0.0;
                checks.push(RobustnessCheck {
                    name: "ci_method".to_string(),
                    passed,
                    primary_delta,
                    alternative_delta: primary_delta,
                    category: RobustnessCategory::CiMethod,
                });
            }
            RobustnessSpec::PairedVsIndependent => {
                let independent = effect_sizes(control, treatment, false, conf, seed + 1000).ok();
                let alt_delta = independent.as_ref().map(|e| e.mean_delta).unwrap_or(0.0);
                let passed = primary_delta.signum() == alt_delta.signum();
                checks.push(RobustnessCheck {
                    name: "paired_vs_independent".to_string(),
                    passed,
                    primary_delta,
                    alternative_delta: alt_delta,
                    category: RobustnessCategory::CiMethod,
                });
            }
        }
    }
    let all_passed = checks.iter().all(|c| c.passed);
    RobustnessReport { checks, all_passed }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::per_replication;
    use matchlab_metrics::MetricResult;
    fn to_s(v: f64) -> ReplicationScalar {
        per_replication::from_metric(&MetricResult::Scalar(v)).unwrap()
    }
    #[test]
    fn symmetric_data_all_checks_pass() {
        let control: Vec<ReplicationScalar> = (0..20).map(|i| to_s(100.0 + i as f64)).collect();
        let treatment: Vec<ReplicationScalar> =
            control.iter().map(|c| to_s(c.value() + 5.0)).collect();
        let specs = vec![
            RobustnessSpec::MeanVsMedian,
            RobustnessSpec::CiMethod,
            RobustnessSpec::PairedVsIndependent,
        ];
        let report = run_robustness(&control, &treatment, &specs, 0.95, 42);
        assert!(
            report.all_passed,
            "symmetric constant-shift: all should pass"
        );
        assert_eq!(report.checks.len(), 3);
    }
    #[test]
    fn adversarial_mean_vs_median_fails() {
        let control: Vec<ReplicationScalar> = vec![to_s(100.0); 20];
        let treatment: Vec<ReplicationScalar> = (0..20)
            .map(|i| if i < 18 { to_s(98.0) } else { to_s(120.0) })
            .collect();
        let specs = vec![RobustnessSpec::MeanVsMedian];
        let report = run_robustness(&control, &treatment, &specs, 0.95, 42);
        let check = &report.checks[0];
        assert!(!check.passed, "mean≠median direction must be caught");
        assert!(check.primary_delta > 0.0, "mean is positive");
        assert!(check.alternative_delta < 0.0, "median is negative");
    }
    #[test]
    fn empty_specs_produces_empty_report() {
        let control: Vec<ReplicationScalar> = vec![to_s(1.0)];
        let treatment: Vec<ReplicationScalar> = vec![to_s(2.0)];
        let report = run_robustness(&control, &treatment, &[], 0.95, 42);
        assert!(report.checks.is_empty());
        assert!(report.all_passed);
    }
    #[test]
    fn determinism() {
        let control: Vec<ReplicationScalar> = (0..10).map(|i| to_s(i as f64)).collect();
        let treatment: Vec<ReplicationScalar> = (0..10).map(|i| to_s(i as f64 + 3.0)).collect();
        let specs = vec![RobustnessSpec::MeanVsMedian];
        let r1 = run_robustness(&control, &treatment, &specs, 0.95, 42);
        let r2 = run_robustness(&control, &treatment, &specs, 0.95, 42);
        assert_eq!(r1.all_passed, r2.all_passed);
        assert_eq!(r1.checks[0].passed, r2.checks[0].passed);
    }
}
