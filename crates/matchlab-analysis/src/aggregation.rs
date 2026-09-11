//! Replication-level observation table (spec §14.9,): the
//! canonical pipeline from raw observations to per-replication scalars.
//!
//! ```text
//! raw observations
//!    ↓
//! metric per replication
//!    ↓
//! replication distribution
//!    ↓
//! estimand
//!    ↓
//! sampling uncertainty
//! ```
//!
//! The statistical unit is the **replication**. This module builds a
//! [`ReplicationTable`] — a per-`StudyResult` observation matrix indexed by
//! `(condition_id, metric)` — that every downstream stage (estimand, bootstrap
//! CI, effect size, reports) consumes.
//!
//! Aggregation semantics:
//! - `Scalar` → value
//! - `Summary` → **mean** (v0.2 behavior; `median`/`stddev` available for
//!   )
//! - `TimeSeries` → mean-of-bucket-means
//! - `Distribution` → mean-of-sample (deliberate extension over v0.2;
//!   `queue_time`-style metrics now produce a replication-level scalar
//!   instead of being silently dropped)
//! - `Histogram` → `None` (no built-in producer in v0.3; aggregation
//!   semantics for bucketed data are deferred)
use crate::hierarchy::ReplicationScalar;
use crate::hierarchy::per_replication;
use matchlab_experiments::StudyResult;
use matchlab_metrics::MetricResult;
use std::collections::BTreeMap;
use tracing;
/// One metric's value within one replication, with full provenance
/// (condition, replication index, metric name) and the aggregation result.
pub struct ReplicationObservation {
    pub condition_id: String,
    pub replicate_index: u64,
    pub metric: String,
    pub statistic: MetricResult,
    /// The replication-level scalar, if the metric is aggregatable. Uses the
    /// documented [`per_replication`] constructors to preserve the type
    /// barrier: `Distribution` is collapsed via
    /// [`per_replication::from_distribution`]; all other scalarizable shapes
    /// via [`per_replication::from_metric`].
    pub scalar: Option<ReplicationScalar>,
}
impl ReplicationObservation {
    /// The barrier-typed scalar, or `None` for non-scalarizable metrics.
    pub fn replication_scalar(&self) -> Option<ReplicationScalar> {
        self.scalar
    }
    /// The bare f64 value, for callers outside the statistical core.
    pub fn value(&self) -> Option<f64> {
        self.scalar.map(|s| s.value())
    }
}
/// Per-(condition, metric) column of the observation table. Sorted by
/// `replicate_index` within each column for deterministic iteration.
pub type ReplicationTable = BTreeMap<(String, String), Vec<ReplicationObservation>>;
/// Aggregate any [`MetricResult`] to a replication-level scalar. Returns
/// `None` for non-scalarizable shapes (`Histogram` in v0.3).
///
/// This is the canonical per-replication aggregation function. For the
/// type-barriered constructors used internally by the table, see
/// [`per_replication::from_metric`] and
/// [`per_replication::from_distribution`].
pub fn replication_scalar(result: &MetricResult) -> Option<f64> {
    match result {
        MetricResult::Scalar(v) => Some(*v),
        MetricResult::Summary { mean, .. } => Some(*mean),
        MetricResult::TimeSeries { bucket_means } => {
            if bucket_means.is_empty() {
                None
            } else {
                let sum: f64 = bucket_means.iter().sum();
                Some(sum / bucket_means.len() as f64)
            }
        }
        MetricResult::Distribution(samples) => {
            if samples.is_empty() {
                None
            } else {
                let sum: f64 = samples.iter().sum();
                Some(sum / samples.len() as f64)
            }
        }
        MetricResult::Histogram { .. } => None,
    }
}
/// Build the canonical replication observation table from a [`StudyResult`].
///
/// Each arm × replicate × metric triple becomes one [`ReplicationObservation`]
/// with its provenance attached. Columns are sorted by `replicate_index` for
/// deterministic iteration. A metric absent from a replicate does not appear
/// in the column (distinguishing "missing" from "non-scalarizable").
///
/// Aggregation routing:
/// - `Distribution` → [`per_replication::from_distribution`] (the named,
///   documented collapse step for per-match samples)
/// - Everything else → [`per_replication::from_metric`] (Scalar, Summary,
///   TimeSeries; Histogram returns `None`)
pub fn build_replication_table(study: &StudyResult) -> ReplicationTable {
    tracing::debug!("building replication table from study results");
    let mut table: ReplicationTable = BTreeMap::new();
    for arm in &study.arms {
        let condition_id = arm.condition_id.clone();
        for repl in &arm.replicates {
            for (metric, statistic) in &repl.result.metrics {
                let scalar = match statistic {
                    MetricResult::Distribution(_) => per_replication::from_distribution(statistic),
                    _ => per_replication::from_metric(statistic),
                };
                table
                    .entry((condition_id.clone(), metric.clone()))
                    .or_default()
                    .push(ReplicationObservation {
                        condition_id: condition_id.clone(),
                        replicate_index: repl.replicate_index,
                        metric: metric.clone(),
                        statistic: statistic.clone(),
                        scalar,
                    });
            }
        }
    }
    for col in table.values_mut() {
        col.sort_by_key(|o| o.replicate_index);
    }
    table
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_experiments::{
        ArmResult, ExperimentResult, ReplicateResult, SeedStrategy, StudyResult,
    };
    use matchlab_metrics::MetricResult;
    fn fake_result(name: &str, metrics: Vec<(&str, MetricResult)>) -> ExperimentResult {
        ExperimentResult {
            experiment_id: format!("{name}-x"),
            name: name.to_string(),
            config_hash: "hash".to_string(),
            git_commit: "abc".to_string(),
            timestamp: "t".to_string(),
            matches_completed: 1,
            matches_formed: 1,
            simulated_time_secs: 1.0,
            metrics: metrics
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            utility_score: None,
        }
    }
    fn arm(name: &str, rows: Vec<Vec<(&str, MetricResult)>>) -> ArmResult {
        ArmResult {
            name: name.to_string(),
            condition_id: name.to_string(),
            replicates: rows
                .into_iter()
                .enumerate()
                .map(|(i, metrics)| ReplicateResult {
                    replicate_index: i as u64,
                    seed: i as u64,
                    parent_seed: 0,
                    result: fake_result(name, metrics),
                })
                .collect(),
        }
    }
    fn summary(mean: f64) -> MetricResult {
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
    fn study(arms: Vec<ArmResult>) -> StudyResult {
        StudyResult {
            study_id: "test-study".to_string(),
            name: "test".to_string(),
            config_hash: "dead".to_string(),
            git_commit: "beef".to_string(),
            strategy: SeedStrategy::Crn,
            design: matchlab_experiments::DesignType::Paired,
            experimental_design: matchlab_experiments::ExperimentalDesign::default(),
            replication_count: arms[0].replicates.len() as u64,
            arms,
        }
    }
    #[test]
    fn table_is_deterministic_for_a_given_study() {
        let s = study(vec![
            arm(
                "a",
                vec![vec![("m", summary(1.0))], vec![("m", summary(2.0))]],
            ),
            arm(
                "b",
                vec![vec![("m", summary(3.0))], vec![("m", summary(4.0))]],
            ),
        ]);
        let t1 = build_replication_table(&s);
        let t2 = build_replication_table(&s);
        assert_eq!(t1.len(), t2.len());
        for (k, col1) in &t1 {
            let col2 = t2.get(k).expect("same keys");
            assert_eq!(col1.len(), col2.len());
            for (o1, o2) in col1.iter().zip(col2) {
                assert_eq!(o1.replicate_index, o2.replicate_index);
                assert_eq!(o1.scalar, o2.scalar);
            }
        }
    }
    #[test]
    fn summary_metric_has_scalar_via_table() {
        let s = study(vec![arm("a", vec![vec![("ra", summary(100.0))]])]);
        let table = build_replication_table(&s);
        let col = table.get(&("a".to_string(), "ra".to_string())).unwrap();
        assert_eq!(col.len(), 1);
        assert_eq!(col[0].value(), Some(100.0));
        assert!(col[0].replication_scalar().is_some());
    }
    #[test]
    fn distribution_metric_now_aggregates_to_scalar() {
        let s = study(vec![arm(
            "a",
            vec![vec![(
                "queue_time",
                MetricResult::Distribution(vec![10.0, 20.0, 30.0]),
            )]],
        )]);
        let table = build_replication_table(&s);
        let col = table
            .get(&("a".to_string(), "queue_time".to_string()))
            .unwrap();
        assert_eq!(col.len(), 1);
        let scalar = col[0].value().expect("Distribution aggregates to scalar");
        assert!((scalar - 20.0).abs() < 1e-12, "mean of [10,20,30]");
        assert!(col[0].replication_scalar().is_some());
    }
    #[test]
    fn missing_metric_is_absent_from_column() {
        let s = study(vec![arm("a", vec![vec![("ra", summary(1.0))]])]);
        let table = build_replication_table(&s);
        assert!(
            !table.contains_key(&("a".to_string(), "queue_time".to_string())),
            "metric absent from replicate is absent from table"
        );
    }
    #[test]
    fn empty_distribution_yields_none() {
        let s = study(vec![arm(
            "a",
            vec![vec![("q", MetricResult::Distribution(vec![]))]],
        )]);
        let table = build_replication_table(&s);
        let col = table.get(&("a".to_string(), "q".to_string())).unwrap();
        assert!(col[0].scalar.is_none(), "empty Distribution → None");
    }
    #[test]
    fn histogram_yields_none() {
        let s = study(vec![arm(
            "a",
            vec![vec![(
                "h",
                MetricResult::Histogram {
                    buckets: vec![(1.0, 5), (2.0, 3)],
                },
            )]],
        )]);
        let table = build_replication_table(&s);
        let col = table.get(&("a".to_string(), "h".to_string())).unwrap();
        assert!(col[0].scalar.is_none(), "Histogram → None");
    }
    #[test]
    fn columns_are_sorted_by_replicate_index() {
        let s = study(vec![arm(
            "a",
            vec![
                vec![("m", summary(10.0))],
                vec![],
                vec![("m", summary(30.0))],
            ],
        )]);
        let table = build_replication_table(&s);
        let col = table.get(&("a".to_string(), "m".to_string())).unwrap();
        assert_eq!(col.len(), 2);
        assert_eq!(col[0].replicate_index, 0);
        assert_eq!(col[1].replicate_index, 2);
    }
    #[test]
    fn provenance_is_attached_to_every_observation() {
        let s = study(vec![arm("cond-a", vec![vec![("metric-x", summary(7.0))]])]);
        let table = build_replication_table(&s);
        let obs = &table[&("cond-a".to_string(), "metric-x".to_string())][0];
        assert_eq!(obs.condition_id, "cond-a");
        assert_eq!(obs.metric, "metric-x");
        assert_eq!(obs.replicate_index, 0);
    }
    #[test]
    fn replication_scalar_external_fn_agrees_with_table_scalar_for_documented_cases() {
        let results = vec![
            MetricResult::Scalar(5.0),
            summary(3.0),
            MetricResult::TimeSeries {
                bucket_means: vec![1.0, 2.0, 3.0],
            },
        ];
        for r in &results {
            let ext = replication_scalar(r);
            let from_table = match r {
                MetricResult::Distribution(_) => per_replication::from_distribution(r),
                _ => per_replication::from_metric(r),
            };
            assert_eq!(ext, from_table.map(|s| s.value()));
        }
    }
}
