//! Statistical data hierarchy (spec §14.9,): the explicit,
//! typed nesting `Study → Condition → Replication → {Players, Matches,
//! Observations}`.
//!
//! The critical rule of the v0.3 statistical layer: **players and matches
//! within a replication are not independent statistical observations**, and the
//! default statistical unit is the replication, not the match. This module
//! makes that structural:
//!
//! - [`MetricObservation`] attaches provenance (`condition_id` +
//!   `replicate_index` + `metric`) to every per-replication value, so a metric
//!   result can always answer "which replication did this come from?".
//! - [`replication_scalars`] is the canonical extractor: one observation per
//!   replicate, with N/A replicates recorded ([`MetricValue::Missing`] /
//!   [`MetricValue::NotScalarizable`]) instead of being silently dropped.
//! - [`ReplicationScalar`] is the type barrier: estimators that must respect
//!   nesting consume `&[MetricObservation]` (or `Vec<ReplicationScalar>`)
//!   rather than bare `&[f64]`, so treating per-match samples as if they were
//!   per-replication requires an explicit, documented aggregation step.
use crate::effect::extract_scalar;
use matchlab_experiments::ArmResult;
use matchlab_metrics::MetricResult;
use serde::{Deserialize, Serialize};
/// Why a replicate's metric is not a usable replication-level value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MetricValue {
    /// A scalarable replication-level value.
    Scalar(f64),
    /// The metric is absent from this replicate entirely.
    Missing,
    /// The metric is present but not scalarizable into a replication unit
    /// (e.g. a `Distribution` of raw per-match samples).
    NotScalarizable,
}
/// One metric's value within one replication, with full provenance
/// : which condition, which replication, which metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricObservation {
    pub condition_id: String,
    pub replicate_index: u64,
    pub metric: String,
    pub value: MetricValue,
}
impl MetricObservation {
    /// The scalarable value, or `None` when this replicate is N/A.
    pub fn scalar(&self) -> Option<f64> {
        match self.value {
            MetricValue::Scalar(v) => Some(v),
            MetricValue::Missing | MetricValue::NotScalarizable => None,
        }
    }
    pub fn is_scalarable(&self) -> bool {
        matches!(self.value, MetricValue::Scalar(_))
    }
    /// The documented step from an observation to the estimator's barrier type.
    /// `None` for N/A replicates — a `Vec<ReplicationScalar>` can only be built
    /// through this (or [`per_replication::from_*`]) extraction, never by
    /// pairing per-match samples directly.
    pub fn replication_scalar(&self) -> Option<ReplicationScalar> {
        match self.value {
            MetricValue::Scalar(v) => Some(ReplicationScalar(v)),
            MetricValue::Missing | MetricValue::NotScalarizable => None,
        }
    }
}
/// The type barrier against flattening nested observations : a
/// replication-level scalar with a private field. There is no public
/// `From<f64>` — the only documented constructors are
/// [`per_replication::from_metric`] (per-replication aggregation) and
/// [`per_replication::from_distribution`] (explicit collapse of per-match
/// samples). A bare `f64` can never silently become one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplicationScalar(f64);
/// Documented per-replication aggregation steps. These are the only public
/// ways to obtain a [`ReplicationScalar`]; estimators that respect nesting take
/// `Vec<ReplicationScalar>` (or `&[MetricObservation]`) and refuse raw `f64`.
pub mod per_replication {
    use super::*;
    /// Aggregate one replicate's metric result into a replication-level scalar
    /// using the documented v0.2 rule (`Scalar` → value, `Summary` → mean,
    /// `TimeSeries` → mean-of-bucket-means). A raw `Distribution`/`Histogram`
    /// of per-match samples is *not* accepted here: collapsing it into a
    /// replication unit requires an explicit decision, so use
    /// [`per_replication::from_distribution`].
    pub fn from_metric(result: &MetricResult) -> Option<ReplicationScalar> {
        extract_scalar(result).map(ReplicationScalar)
    }
    /// Explicit, named collapse of a per-match `Distribution` into a
    /// replication-level scalar (the sample mean). Reaching a
    /// `Vec<ReplicationScalar>` from per-match samples always passes through
    /// this (or an equivalent documented) step — never automatically.
    pub fn from_distribution(result: &MetricResult) -> Option<ReplicationScalar> {
        match result {
            MetricResult::Distribution(samples) if !samples.is_empty() => Some(ReplicationScalar(
                samples.iter().sum::<f64>() / samples.len() as f64,
            )),
            _ => None,
        }
    }
}
impl ReplicationScalar {
    /// The wrapped value. Copying a scalar out of its barrier is always
    /// allowed; only *constructing* one requires the documented step.
    pub fn value(&self) -> f64 {
        self.0
    }
}
/// The canonical per-replication extractor (promoted out of `effect.rs`
/// 's private `arm_scalars`): one [`MetricObservation`] per replicate, in
/// ascending `replicate_index` order. Replicates whose metric is absent or
/// non-scalarizable are recorded ([`MetricValue::Missing`] /
/// [`MetricValue::NotScalarizable`]), never silently skipped, so a dropped
/// replicate is visible to downstream statistical layers.
pub fn replication_scalars(arm: &ArmResult, metric: &str) -> Vec<MetricObservation> {
    let condition_id = arm.condition().to_string();
    let mut observations: Vec<MetricObservation> = arm
        .replicates
        .iter()
        .map(|repl| MetricObservation {
            condition_id: condition_id.clone(),
            replicate_index: repl.replicate_index,
            metric: metric.to_string(),
            value: match repl.result.metrics.get(metric) {
                None => MetricValue::Missing,
                Some(result) => match per_replication::from_metric(result) {
                    Some(_) => MetricValue::Scalar(
                        extract_scalar(result).expect("just checked scalarable"),
                    ),
                    None => MetricValue::NotScalarizable,
                },
            },
        })
        .collect();
    observations.sort_by_key(|o| o.replicate_index);
    observations
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_experiments::replicate::ReplicateResult;
    use matchlab_metrics::MetricResult;
    fn arm_with_metrics(metrics: &[(MetricResult, Option<u64>)]) -> ArmResult {
        ArmResult {
            name: "arm".to_string(),
            condition_id: "cond-1".to_string(),
            replicates: metrics
                .iter()
                .enumerate()
                .map(|(i, (result, index))| ReplicateResult {
                    replicate_index: index.unwrap_or(i as u64),
                    seed: i as u64,
                    parent_seed: 0,
                    result: matchlab_experiments::ExperimentResult {
                        experiment_id: format!("arm-x{i}"),
                        name: "arm".to_string(),
                        config_hash: "hash".to_string(),
                        git_commit: "abc".to_string(),
                        timestamp: "t".to_string(),
                        matches_completed: 1,
                        matches_formed: 1,
                        simulated_time_secs: 1.0,
                        metrics: std::collections::BTreeMap::from([(
                            "m".to_string(),
                            result.clone(),
                        )]),
                        utility_score: None,
                    },
                })
                .collect(),
        }
    }
    fn sample_metrics() -> Vec<(MetricResult, Option<u64>)> {
        vec![
            (
                MetricResult::Summary {
                    mean: 1.0,
                    median: 1.0,
                    p75: 1.0,
                    p90: 1.0,
                    p95: 1.0,
                    p99: 1.0,
                    stddev: 0.0,
                },
                None,
            ),
            (MetricResult::Distribution(vec![0.9, 0.95, 0.97]), None),
            (MetricResult::Scalar(3.0), Some(99)),
        ]
    }
    #[test]
    fn metric_observation_round_trips_as_json_with_provenance() {
        let obs = MetricObservation {
            condition_id: "a/b".to_string(),
            replicate_index: 7,
            metric: "rating_accuracy".to_string(),
            value: MetricValue::Scalar(12.5),
        };
        let json = serde_json::to_string(&obs).expect("serialize");
        let back: MetricObservation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.condition_id, "a/b");
        assert_eq!(back.replicate_index, 7);
        assert_eq!(back.metric, "rating_accuracy");
        assert_eq!(back.value, MetricValue::Scalar(12.5));
    }
    #[test]
    fn replication_scalars_record_na_instead_of_dropping() {
        let arm = arm_with_metrics(&sample_metrics());
        let obs = replication_scalars(&arm, "m");
        assert_eq!(obs.len(), 3);
        assert!(obs[0].is_scalarable());
        assert_eq!(obs[0].scalar(), Some(1.0));
        assert_eq!(obs[1].value, MetricValue::NotScalarizable);
        assert_eq!(obs[1].scalar(), None);
        assert_eq!(obs[2].scalar(), Some(3.0));
        assert!(obs.iter().all(|o| o.condition_id == "cond-1"));
        assert_eq!(
            obs.iter().map(|o| o.replicate_index).collect::<Vec<_>>(),
            vec![0, 1, 99],
            "ascending replicate index order"
        );
    }
    #[test]
    fn missing_metric_is_visible_just_like_not_scalarizable() {
        let arm = ArmResult {
            name: "arm".to_string(),
            condition_id: "cond".to_string(),
            replicates: vec![ReplicateResult {
                replicate_index: 0,
                seed: 0,
                parent_seed: 0,
                result: matchlab_experiments::ExperimentResult {
                    experiment_id: "x".to_string(),
                    name: "arm".to_string(),
                    config_hash: "h".to_string(),
                    git_commit: "g".to_string(),
                    timestamp: "t".to_string(),
                    matches_completed: 0,
                    matches_formed: 0,
                    simulated_time_secs: 0.0,
                    metrics: std::collections::BTreeMap::new(),
                    utility_score: None,
                },
            }],
        };
        let obs = replication_scalars(&arm, "nope");
        assert_eq!(obs.len(), 1, "replicate present, metric absent");
        assert_eq!(obs[0].value, MetricValue::Missing);
        assert_eq!(obs[0].scalar(), None);
    }
    #[test]
    fn per_match_samples_need_an_explicit_replication_step() {
        let raw = MetricResult::Distribution(vec![0.9, 0.95, 0.97]);
        assert!(
            per_replication::from_metric(&raw).is_none(),
            "raw per-match samples rejected at the barrier"
        );
        let collapsed = per_replication::from_distribution(&raw).expect("explicit step");
        assert!((collapsed.value() - 0.94).abs() < 1e-12);
        let summary = MetricResult::Summary {
            mean: 2.0,
            median: 2.0,
            p75: 2.0,
            p90: 2.0,
            p95: 2.0,
            p99: 2.0,
            stddev: 0.0,
        };
        let s = per_replication::from_metric(&summary).expect("documented aggregation");
        assert_eq!(s.value(), 2.0);
    }
}
