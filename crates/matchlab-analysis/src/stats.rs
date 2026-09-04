//! Statistical summaries (spec §14.1).
//!
//! The canonical implementation lives in `matchlab-metrics` (so collectors can
//! stay on the metrics-only-core boundary); this module re-exports it so
//! consumers write `matchlab_analysis::stats::summary(...)`.

pub use matchlab_metrics::stats::{Summary, summary, summary_to_result};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_produces_required_percentiles_and_stddev() {
        let s = summary(&[1.0, 2.0, 3.0, 4.0, 100.0]);
        assert_eq!(s.n, 5);
        assert!((s.mean - 22.0).abs() < 1e-9);
        // sorted = [1,2,3,4,100]; median idx=2 → 3; p75 idx=3 → 4; p99 idx=3 → 4
        assert_eq!(s.median, 3.0);
        assert_eq!(s.p75, 4.0);
        assert_eq!(s.p95, 4.0);
        assert_eq!(s.p99, 4.0);
        // variance = 7610/5 = 1522 → stddev ≈ 39.01
        assert!((s.stddev - 39.01).abs() < 0.01);
    }

    #[test]
    fn summary_to_result_roundtrips() {
        let r = summary_to_result(&[1.0, 2.0]);
        match r {
            matchlab_metrics::MetricResult::Summary { mean, .. } => {
                assert!((mean - 1.5).abs() < 1e-9)
            }
            other => panic!("expected Summary, got {other:?}"),
        }
    }
}
