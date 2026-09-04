use crate::collector::MetricResult;

/// Statistical summary over a sample (spec §14.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub stddev: f64,
}

pub fn summary(values: &[f64]) -> Summary {
    if values.is_empty() {
        return Summary {
            n: 0,
            mean: 0.0,
            median: 0.0,
            p75: 0.0,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
            stddev: 0.0,
        };
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    Summary {
        n: values.len(),
        mean,
        median: percentile(&sorted, 50.0),
        p75: percentile(&sorted, 75.0),
        p90: percentile(&sorted, 90.0),
        p95: percentile(&sorted, 95.0),
        p99: percentile(&sorted, 99.0),
        stddev: var.sqrt(),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = (p / 100.0 * (sorted.len() - 1) as f64) as usize;
    sorted[idx]
}

/// Convert a `Summary` into a `MetricResult` for use by collectors. An empty
/// sample reports `Scalar(0.0)`.
pub fn summary_to_result(values: &[f64]) -> MetricResult {
    if values.is_empty() {
        return MetricResult::Scalar(0.0);
    }
    let s = summary(values);
    MetricResult::Summary {
        mean: s.mean,
        median: s.median,
        p75: s.p75,
        p90: s.p90,
        p95: s.p95,
        p99: s.p99,
        stddev: s.stddev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_slice_summarizes_to_zeros() {
        let s = summary(&[]);
        assert_eq!(s.n, 0);
        assert_eq!(s.mean, 0.0);
        assert_eq!(s.stddev, 0.0);
        assert_eq!(summary_to_result(&[]), MetricResult::Scalar(0.0));
    }

    #[test]
    fn summary_matches_known_values() {
        let s = summary(&[1.0, 2.0, 3.0, 4.0, 100.0]);
        assert_eq!(s.n, 5);
        assert!((s.mean - 22.0).abs() < 1e-9);
        // sorted = [1,2,3,4,100]; median idx=2 → 3; p75 idx=3 → 4; p99 idx=3 → 4
        assert_eq!(s.median, 3.0);
        assert_eq!(s.p75, 4.0);
        assert_eq!(s.p99, 4.0);
        // variance = 7610/5 = 1522 → stddev ≈ 39.01
        assert!((s.stddev - 39.01).abs() < 0.01);
    }

    #[test]
    fn summary_to_result_roundtrips_to_summary_variant() {
        let r = summary_to_result(&[1.0, 2.0]);
        match r {
            MetricResult::Summary { mean, .. } => assert!((mean - 1.5).abs() < 1e-9),
            other => panic!("expected Summary, got {other:?}"),
        }
    }
}
