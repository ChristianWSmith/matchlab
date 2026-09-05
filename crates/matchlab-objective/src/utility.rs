use std::collections::HashMap;

use matchlab_metrics::MetricResult;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObjectiveWeights {
    pub match_quality: f64,
    pub queue_time: f64,
    pub rating_accuracy: f64,
    pub convergence_speed: f64,
    pub smurf_damage: f64,
    pub false_positive_rate: f64,
    pub streak_frustration: f64,
}

impl Default for ObjectiveWeights {
    fn default() -> Self {
        Self {
            match_quality: 1.0,
            queue_time: 0.5,
            rating_accuracy: 1.0,
            convergence_speed: 0.8,
            smurf_damage: 2.0,
            false_positive_rate: 1.5,
            streak_frustration: 0.3,
        }
    }
}

pub struct ObjectiveFunction {
    pub weights: ObjectiveWeights,
}

impl ObjectiveFunction {
    pub fn new(weights: ObjectiveWeights) -> Self {
        Self { weights }
    }

    /// Compute aggregate utility from raw metrics. Returns the utility score
    /// AND the raw metrics — never discards raw values (§12.2).
    pub fn evaluate<'a>(
        &self,
        metrics: &'a HashMap<String, MetricResult>,
    ) -> (f64, &'a HashMap<String, MetricResult>) {
        let mut score = 0.0;

        if let Some(MetricResult::Summary { mean, .. }) = metrics.get("match_quality") {
            score += self.weights.match_quality * mean;
        }
        if let Some(MetricResult::Summary { mean, .. }) = metrics.get("queue_time") {
            score -= self.weights.queue_time * mean;
        }
        if let Some(MetricResult::Summary { mean, .. }) = metrics.get("rating_accuracy") {
            score -= self.weights.rating_accuracy * mean;
        }
        if let Some(MetricResult::Summary { mean, .. }) = metrics.get("convergence") {
            score -= self.weights.convergence_speed * mean;
        }
        if let Some(MetricResult::Distribution(d)) = metrics.get("smurf") {
            if let Some(&damage) = d.get(3) {
                score -= self.weights.smurf_damage * damage;
            }
            if let Some(&fp) = d.get(1) {
                score -= self.weights.false_positive_rate * fp;
            }
        }
        if let Some(MetricResult::Distribution(d)) = metrics.get("streaks") {
            if let Some(&p5) = d.first() {
                score -= self.weights.streak_frustration * p5;
            }
        }

        (score, metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn evaluate_all_zero_metrics_scores_zero() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert("match_quality".to_string(), summary(0.0));
        metrics.insert("queue_time".to_string(), summary(0.0));
        metrics.insert("rating_accuracy".to_string(), summary(0.0));
        metrics.insert("convergence".to_string(), summary(0.0));
        metrics.insert(
            "smurf".to_string(),
            MetricResult::Distribution(vec![0.0, 0.0, 0.0, 0.0]),
        );
        metrics.insert(
            "streaks".to_string(),
            MetricResult::Distribution(vec![0.0, 0.0, 0.0, 0.0]),
        );

        let (score, _) = func.evaluate(&metrics);
        assert!((score - 0.0).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn evaluate_high_match_quality_adds_score() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert("match_quality".to_string(), summary(0.9));

        let (score, _) = func.evaluate(&metrics);
        // weight 1.0 * 0.9 = 0.9
        assert!((score - 0.9).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn evaluate_high_queue_time_subtracts_score() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert("queue_time".to_string(), summary(30.0));

        let (score, _) = func.evaluate(&metrics);
        // weight 0.5 * 30 = 15 subtracted
        assert!((score + 15.0).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn evaluate_high_rating_error_subtracts_score() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert("rating_accuracy".to_string(), summary(200.0));

        let (score, _) = func.evaluate(&metrics);
        // weight 1.0 * 200 = 200 subtracted
        assert!((score + 200.0).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn evaluate_smurf_damage_subtracts_score() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert(
            "smurf".to_string(),
            MetricResult::Distribution(vec![0.5, 0.1, 0.5, 0.8]),
        );

        let (score, _) = func.evaluate(&metrics);
        // damage (index 3) = 0.8, weight 2.0 → 1.6 subtracted
        // fp (index 1) = 0.1, weight 1.5 → 0.15 subtracted
        assert!((score + 1.75).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn evaluate_streak_frustration_subtracts_score() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert(
            "streaks".to_string(),
            MetricResult::Distribution(vec![0.4, 0.2, 0.1, 0.05]),
        );

        let (score, _) = func.evaluate(&metrics);
        // p5 (index 0) = 0.4, weight 0.3 → 0.12 subtracted
        assert!((score + 0.12).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn evaluate_preserves_raw_metrics() {
        let func = ObjectiveFunction::new(ObjectiveWeights::default());
        let mut metrics = HashMap::new();
        metrics.insert("match_quality".to_string(), summary(0.85));

        let (_, returned) = func.evaluate(&metrics);
        assert_eq!(returned, &metrics);
        assert!(std::ptr::eq(returned, &metrics));
    }

    #[test]
    fn default_weights_match_spec_values() {
        let w = ObjectiveWeights::default();
        assert_eq!(w.match_quality, 1.0);
        assert_eq!(w.queue_time, 0.5);
        assert_eq!(w.rating_accuracy, 1.0);
        assert_eq!(w.convergence_speed, 0.8);
        assert_eq!(w.smurf_damage, 2.0);
        assert_eq!(w.false_positive_rate, 1.5);
        assert_eq!(w.streak_frustration, 0.3);
    }
}
