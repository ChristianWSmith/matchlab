//! Statistical result object (spec §14.9,): the atomic,
//! serializable unit consumed by reports, CLI, JSON export, and downstream
//! analysis. Ties together estimand, point estimate, uncertainty, effect size,
//! and sample provenance into one first-class type.
use crate::effect::{CiMethod, EffectSize};
use crate::estimand::Estimand;
/// How a per-replication metric scalar was aggregated from the raw metric
/// result .
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationKind {
    Mean,
    Median,
    DistributionMean,
}
/// Full statistical provenance : everything needed to reproduce a result
/// from the file alone.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Provenance {
    pub study_id: String,
    pub config_hash: String,
    pub git_commit: String,
    pub engine_version: String,
    pub condition_ids: (String, String),
    pub metric: String,
    pub design: matchlab_experiments::DesignType,
    pub aggregation: AggregationKind,
    pub method: CiMethod,
    pub confidence: f64,
    pub bootstrap_iterations: Option<usize>,
    pub seed: u64,
    pub n_replications: usize,
}
/// Uncertainty quantification for a point estimate .
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Uncertainty {
    pub method: CiMethod,
    pub confidence: f64,
    pub lower: f64,
    pub upper: f64,
    pub bootstrap_iterations: Option<usize>,
    pub seed: Option<u64>,
}
/// Summary of an effect size across its components .
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EffectSizeSummary {
    pub absolute: f64,
    pub relative: f64,
    pub percent_change: f64,
    /// Standardized effect: `d_z` for paired designs, `cohen_d` for
    /// independent. `None` when the standardized effect is undefined (e.g.
    /// constant deltas with zero variance).
    pub standardized: Option<f64>,
}
/// Sample provenance for a statistical result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SampleSummary {
    /// Number of independent replications (the statistical unit,).
    pub replications: usize,
    /// Per-arm sample sizes: `(control_n, treatment_n)`.
    pub n_per_arm: (usize, usize),
}
/// The atomic statistical result — one row per (pair of arms, metric).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct StatisticalResult {
    pub estimand: Estimand,
    pub estimate: f64,
    pub uncertainty: Uncertainty,
    pub effect_size: EffectSizeSummary,
    pub sample: SampleSummary,
    /// Optional family grouping for multiple-comparisons correction .
    pub family: Option<String>,
    /// Raw two-sided p-value from the pair .
    pub p_value: Option<f64>,
    /// Adjusted p-value after Holm/BH correction . `None` when no
    /// correction has been applied.
    pub adjusted_p: Option<f64>,
    /// Full provenance : everything needed to reproduce this result.
    pub provenance: Provenance,
}
impl StatisticalResult {
    /// Construct a `StatisticalResult` from an estimand, effect size, and
    /// sample provenance.
    pub fn new(
        estimand: Estimand,
        es: EffectSize,
        n_pairs: usize,
        n_per_arm: (usize, usize),
    ) -> Self {
        Self {
            estimand,
            estimate: es.mean_delta,
            uncertainty: Uncertainty {
                method: es.ci_method,
                confidence: 0.95,
                lower: es.ci_lo,
                upper: es.ci_hi,
                bootstrap_iterations: Some(10_000),
                seed: None,
            },
            effect_size: EffectSizeSummary {
                absolute: es.mean_delta,
                relative: es.relative_diff,
                percent_change: es.percent_improvement,
                standardized: es.d_z.or(Some(es.cohen_d)),
            },
            sample: SampleSummary {
                replications: n_pairs,
                n_per_arm,
            },
            family: None,
            p_value: None,
            adjusted_p: None,
            provenance: Provenance {
                study_id: String::new(),
                config_hash: String::new(),
                git_commit: String::new(),
                engine_version: env!("CARGO_PKG_VERSION").to_string(),
                condition_ids: (String::new(), String::new()),
                metric: String::new(),
                design: matchlab_experiments::DesignType::Paired,
                aggregation: AggregationKind::Mean,
                method: es.ci_method,
                confidence: 0.95,
                bootstrap_iterations: Some(10_000),
                seed: 0,
                n_replications: n_pairs,
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimand::EstimandDef;
    use crate::hierarchy::{ReplicationScalar, per_replication};
    use matchlab_metrics::MetricResult;
    fn sample_result() -> StatisticalResult {
        StatisticalResult {
            estimand: Estimand::MeanPairedDifference(EstimandDef {
                outcome: "rating_accuracy".to_string(),
                treatment: "glicko2".to_string(),
                control: "elo".to_string(),
            }),
            estimate: -14.23,
            uncertainty: Uncertainty {
                method: CiMethod::PairedBootstrap,
                confidence: 0.95,
                lower: -16.11,
                upper: -12.42,
                bootstrap_iterations: Some(10_000),
                seed: Some(42),
            },
            effect_size: EffectSizeSummary {
                absolute: -14.23,
                relative: -0.095,
                percent_change: -9.5,
                standardized: Some(-0.82),
            },
            sample: SampleSummary {
                replications: 500,
                n_per_arm: (500, 500),
            },
            family: None,
            p_value: Some(0.001),
            adjusted_p: None,
            provenance: Provenance {
                study_id: "test-study".to_string(),
                config_hash: "deadbeef".to_string(),
                git_commit: "abc123".to_string(),
                engine_version: "0.1.0".to_string(),
                condition_ids: ("elo".to_string(), "glicko2".to_string()),
                metric: "rating_accuracy".to_string(),
                design: matchlab_experiments::DesignType::Paired,
                aggregation: AggregationKind::Mean,
                method: CiMethod::PairedBootstrap,
                confidence: 0.95,
                bootstrap_iterations: Some(10_000),
                seed: 42,
                n_replications: 500,
            },
        }
    }
    #[test]
    fn statistical_result_round_trips_json() {
        let sr = sample_result();
        let json = serde_json::to_string(&sr).expect("serialize");
        let back: StatisticalResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sr.estimate, back.estimate);
        assert_eq!(sr.uncertainty, back.uncertainty);
        assert_eq!(sr.effect_size, back.effect_size);
        assert_eq!(sr.sample, back.sample);
        assert_eq!(sr.estimand.name(), back.estimand.name());
    }
    #[test]
    fn statistical_result_round_trips_yaml() {
        let sr = sample_result();
        let yaml = serde_yaml::to_string(&sr).expect("serialize yaml");
        let back: StatisticalResult = serde_yaml::from_str(&yaml).expect("deserialize yaml");
        assert_eq!(sr.estimate, back.estimate);
        assert_eq!(sr.uncertainty, back.uncertainty);
        assert_eq!(sr.effect_size, back.effect_size);
        assert_eq!(sr.sample, back.sample);
        assert_eq!(sr.estimand.name(), back.estimand.name());
    }
    #[test]
    fn proposal_sketch_parses_verbatim() {
        let yaml = r#"
estimand: mean_paired_difference
estimate: -14.23
uncertainty:
  method: paired_bootstrap
  confidence: 0.95
  lower: -16.11
  upper: -12.42
  bootstrap_iterations: 10000
  seed: 42
effect_size:
  absolute: -14.23
  relative: -0.095
  percent_change: -9.5
  standardized: -0.82
sample:
  replications: 500
  n_per_arm:
  - 500
  - 500
family: null
p_value: 0.001
adjusted_p: null
provenance:
  study_id: test
  config_hash: abc
  git_commit: def
  engine_version: "0.1.0"
  condition_ids:
  - elo
  - glicko2
  metric: rating_accuracy
  design: paired
  aggregation: mean
  method: paired_bootstrap
  confidence: 0.95
  bootstrap_iterations: 10000
  seed: 42
  n_replications: 500
"#;
        let sr: StatisticalResult = serde_yaml::from_str(yaml).expect("proposal sketch parses");
        assert_eq!(sr.estimate, -14.23);
        assert_eq!(sr.uncertainty.lower, -16.11);
        assert_eq!(sr.sample.replications, 500);
    }
    #[test]
    fn compute_statistical_results_on_fixture() {
        let to_scalar = |v: f64| -> ReplicationScalar {
            per_replication::from_metric(&MetricResult::Scalar(v)).expect("scalar")
        };
        let control_s = [157.0, 160.0, 159.0];
        let treatment_s = [144.0, 142.0, 145.0];
        let control: Vec<ReplicationScalar> = control_s.iter().copied().map(to_scalar).collect();
        let treatment: Vec<ReplicationScalar> =
            treatment_s.iter().copied().map(to_scalar).collect();
        let es = crate::effect::effect_sizes(&control, &treatment, true, 0.95, 42).expect("effect");
        let sr = StatisticalResult::new(
            Estimand::MeanPairedDifference(EstimandDef {
                outcome: "rating_accuracy".to_string(),
                treatment: "glicko".to_string(),
                control: "elo".to_string(),
            }),
            es,
            3,
            (3, 3),
        );
        assert_eq!(sr.sample.replications, 3);
        assert!((sr.estimate - (-15.0)).abs() < 1.0, "estimate ≈ Δ");
        assert!(sr.uncertainty.lower < sr.estimate);
        assert!(sr.uncertainty.upper > sr.estimate);
        assert_eq!(sr.uncertainty.method, CiMethod::PairedBootstrap);
    }
    #[test]
    fn provenance_is_present_on_fixture() {
        let sr = sample_result();
        assert_eq!(sr.provenance.study_id, "test-study");
        assert_eq!(sr.provenance.engine_version, "0.1.0");
        assert_eq!(sr.provenance.method, CiMethod::PairedBootstrap);
        assert_eq!(sr.provenance.aggregation, AggregationKind::Mean);
        assert_eq!(sr.provenance.seed, 42);
        assert_eq!(sr.provenance.n_replications, 500);
    }
    #[test]
    fn provenance_round_trips_json() {
        let sr = sample_result();
        let json = serde_json::to_string(&sr).unwrap();
        let back: StatisticalResult = serde_json::from_str(&json).unwrap();
        assert_eq!(sr.provenance, back.provenance);
    }
}
