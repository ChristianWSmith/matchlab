use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

/// A per-metric recorder that aggregates data across every match it is shown
/// (spec §11.2).
pub trait MetricCollector: Send + Sync {
    fn name(&self) -> &str;
    fn record_match(&mut self, match_result: &MatchResult, world: &World);
    fn compute(&self) -> MetricResult;
    /// Optional per-time-bucket series over the run's duration (e.g. bucketed
    /// MAE to show rating convergence). When present, the engine folds it into
    /// a `{name}_by_time` metric.
    fn time_buckets(&self) -> Option<Vec<f64>> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MetricResult {
    Scalar(f64),
    Distribution(Vec<f64>),
    Summary {
        mean: f64,
        median: f64,
        p75: f64,
        p90: f64,
        p95: f64,
        p99: f64,
        stddev: f64,
    },
    Histogram {
        buckets: Vec<(f64, u64)>,
    },
    /// Equal-duration bucket means sampled over the run, e.g. MAE by time
    /// window for demonstrating rating convergence.
    TimeSeries {
        bucket_means: Vec<f64>,
    },
}
