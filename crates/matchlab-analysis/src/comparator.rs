//! Multi-experiment comparison (spec §14.6): side-by-side metric comparison
//! across experiments, with optional baseline and utility-score ranking.

use matchlab_experiments::ExperimentResult;
use std::collections::HashMap;

pub struct Comparator {
    pub results: Vec<ExperimentResult>,
    pub baseline: Option<usize>,
}

pub struct MetricComparison {
    pub experiment: String,
    pub value: matchlab_metrics::MetricResult,
}

impl Comparator {
    pub fn new(results: Vec<ExperimentResult>) -> Self {
        Self {
            results,
            baseline: None,
        }
    }

    pub fn set_baseline(&mut self, index: usize) {
        self.baseline = Some(index);
    }

    pub fn metric_comparison(&self) -> HashMap<String, Vec<MetricComparison>> {
        let mut out = HashMap::new();
        for result in &self.results {
            for (name, value) in &result.metrics {
                out.entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(MetricComparison {
                        experiment: result.name.clone(),
                        value: value.clone(),
                    });
            }
        }
        out
    }

    pub fn ranking(&self) -> Vec<(&ExperimentResult, f64)> {
        let mut ranked: Vec<_> = self
            .results
            .iter()
            .filter_map(|r| r.utility_score.map(|s| (r, s)))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_metrics::MetricResult;
    use std::collections::BTreeMap;

    fn result(name: &str, metric_value: f64, utility: Option<f64>) -> ExperimentResult {
        let mut metrics = BTreeMap::new();
        metrics.insert(
            "match_quality".to_string(),
            MetricResult::Summary {
                mean: metric_value,
                median: 0.0,
                p75: 0.0,
                p90: 0.0,
                p95: 0.0,
                p99: 0.0,
                stddev: 0.0,
            },
        );
        ExperimentResult {
            experiment_id: name.to_string(),
            name: name.to_string(),
            config_hash: "hash".to_string(),
            git_commit: "abc".to_string(),
            timestamp: "now".to_string(),
            matches_completed: 1,
            matches_formed: 1,
            simulated_time_secs: 1.0,
            metrics,
            utility_score: utility,
        }
    }

    #[test]
    fn metric_comparison_groups_by_metric() {
        let results = vec![result("a", 0.9, Some(1.0)), result("b", 0.8, Some(0.5))];
        let c = Comparator::new(results);
        let cmp = c.metric_comparison();
        let qualities = cmp.get("match_quality").expect("metric present");
        assert_eq!(qualities.len(), 2);
        assert_eq!(qualities[0].experiment, "a");
        assert_eq!(qualities[1].experiment, "b");
    }

    #[test]
    fn ranking_sorts_by_utility_descending() {
        let results = vec![
            result("low", 0.7, Some(0.2)),
            result("high", 0.9, Some(0.9)),
            result("mid", 0.8, Some(0.5)),
        ];
        let c = Comparator::new(results);
        let ranked = c.ranking();
        assert_eq!(ranked[0].0.name, "high");
        assert_eq!(ranked[1].0.name, "mid");
        assert_eq!(ranked[2].0.name, "low");
    }

    #[test]
    fn ranking_skips_results_without_utility() {
        let results = vec![result("a", 0.9, Some(1.0)), result("b", 0.8, None)];
        let c = Comparator::new(results);
        let ranked = c.ranking();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0.name, "a");
    }

    #[test]
    fn baseline_can_be_set() {
        let mut c = Comparator::new(vec![result("a", 0.9, None), result("b", 0.8, None)]);
        c.set_baseline(1);
        assert_eq!(c.baseline, Some(1));
    }

    #[test]
    fn result_json_roundtrip_preserves_all_metric_variants() {
        let mut metrics = BTreeMap::new();
        metrics.insert("scalar".to_string(), MetricResult::Scalar(0.5));
        metrics.insert(
            "dist".to_string(),
            MetricResult::Distribution(vec![1.0, 2.0, 3.0]),
        );
        metrics.insert(
            "summary".to_string(),
            MetricResult::Summary {
                mean: 0.9,
                median: 0.88,
                p75: 0.91,
                p90: 0.92,
                p95: 0.93,
                p99: 0.95,
                stddev: 0.01,
            },
        );
        metrics.insert(
            "hist".to_string(),
            MetricResult::Histogram {
                buckets: vec![(0.5, 10), (1.0, 20)],
            },
        );
        metrics.insert(
            "series".to_string(),
            MetricResult::TimeSeries {
                bucket_means: vec![199.8, 163.1],
            },
        );
        let original = ExperimentResult {
            experiment_id: "roundtrip-hash".to_string(),
            name: "roundtrip".to_string(),
            config_hash: "hash".to_string(),
            git_commit: "abc".to_string(),
            timestamp: "now".to_string(),
            matches_completed: 7,
            matches_formed: 7,
            simulated_time_secs: 210.0,
            metrics,
            utility_score: Some(0.875),
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ExperimentResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.config_hash, original.config_hash);
        assert_eq!(parsed.matches_completed, original.matches_completed);
        assert_eq!(parsed.metrics, original.metrics);
        assert_eq!(parsed.utility_score, original.utility_score);
    }
}
