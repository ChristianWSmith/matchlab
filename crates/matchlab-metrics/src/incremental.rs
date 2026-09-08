//! Incremental result aggregation: streaming/incremental
//! aggregation for appropriate statistics, allowing large studies whose
//! total output exceeds available memory.
/// An aggregator that can be updated incrementally with new values.
pub trait IncrementalAggregator: Send + Sync {
    fn update(&mut self, value: f64);
    fn result(&self) -> AggregatedValue;
    fn merge(&mut self, other: &Self);
}
/// The result of an incremental aggregation.
#[derive(Debug, Clone)]
pub enum AggregatedValue {
    Mean(f64),
    Variance { mean: f64, m2: f64, count: u64 },
    Count(u64),
}
/// Online mean using Welford's algorithm.
#[derive(Debug, Clone)]
pub struct WelfordMean {
    mean: f64,
    count: u64,
}
impl Default for WelfordMean {
    fn default() -> Self {
        Self {
            mean: 0.0,
            count: 0,
        }
    }
}
impl IncrementalAggregator for WelfordMean {
    fn update(&mut self, value: f64) {
        self.count += 1;
        self.mean += (value - self.mean) / self.count as f64;
    }
    fn result(&self) -> AggregatedValue {
        AggregatedValue::Mean(self.mean)
    }
    fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        let total = self.count + other.count;
        if total == 0 {
            return;
        }
        let delta = other.mean - self.mean;
        self.mean += delta * other.count as f64 / total as f64;
        self.count = total;
    }
}
/// Online variance using Welford's algorithm.
#[derive(Debug, Clone)]
pub struct WelfordVariance {
    mean: f64,
    m2: f64,
    count: u64,
}
impl Default for WelfordVariance {
    fn default() -> Self {
        Self {
            mean: 0.0,
            m2: 0.0,
            count: 0,
        }
    }
}
impl IncrementalAggregator for WelfordVariance {
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }
    fn result(&self) -> AggregatedValue {
        AggregatedValue::Variance {
            mean: self.mean,
            m2: self.m2,
            count: self.count,
        }
    }
    fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        let total = self.count + other.count;
        if total == 0 {
            return;
        }
        let delta = other.mean - self.mean;
        self.m2 += other.m2 + delta * delta * self.count as f64 * other.count as f64 / total as f64;
        self.mean += delta * other.count as f64 / total as f64;
        self.count = total;
    }
}
/// Simple count aggregator.
#[derive(Debug, Clone, Default)]
pub struct CountAggregator {
    count: u64,
}
impl IncrementalAggregator for CountAggregator {
    fn update(&mut self, _value: f64) {
        self.count += 1;
    }
    fn result(&self) -> AggregatedValue {
        AggregatedValue::Count(self.count)
    }
    fn merge(&mut self, other: &Self) {
        self.count += other.count;
    }
}
/// Configuration for per-metric aggregation strategy.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationStrategy {
    /// Incremental: use Welford's algorithm, no retained observations.
    Incremental,
    /// Full: retain all observations (required for certain statistics).
    Full,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn welford_mean_matches_exact() {
        let mut agg = WelfordMean::default();
        for i in 0..100 {
            agg.update(i as f64);
        }
        let result = agg.result();
        match result {
            AggregatedValue::Mean(m) => {
                assert!((m - 49.5).abs() < 1e-9, "mean should be 49.5, got {m}");
            }
            _ => panic!("expected Mean"),
        }
    }
    #[test]
    fn welford_variance_matches_exact() {
        let mut agg = WelfordVariance::default();
        for i in 0..100 {
            agg.update(i as f64);
        }
        let result = agg.result();
        match result {
            AggregatedValue::Variance { mean, m2, count } => {
                assert_eq!(count, 100);
                assert!((mean - 49.5).abs() < 1e-9);
                let variance = m2 / (count - 1) as f64;
                assert!(
                    (variance - 841.6666666666666).abs() < 0.01,
                    "sample variance should be ~841.67, got {variance}"
                );
            }
            _ => panic!("expected Variance"),
        }
    }
    #[test]
    fn merge_produces_correct_combined_result() {
        let mut a = WelfordMean::default();
        let mut b = WelfordMean::default();
        for i in 0..50 {
            a.update(i as f64);
        }
        for i in 50..100 {
            b.update(i as f64);
        }
        a.merge(&b);
        match a.result() {
            AggregatedValue::Mean(m) => {
                assert!(
                    (m - 49.5).abs() < 1e-9,
                    "merged mean should be 49.5, got {m}"
                );
            }
            _ => panic!("expected Mean"),
        }
    }
    #[test]
    fn count_aggregator_counts() {
        let mut agg = CountAggregator::default();
        for _ in 0..42 {
            agg.update(0.0);
        }
        match agg.result() {
            AggregatedValue::Count(c) => assert_eq!(c, 42),
            _ => panic!("expected Count"),
        }
    }
}
