//! Strategic equilibria and stability metrics: analysis
//! primitives for studying whether strategic populations stabilize,
//! measuring strategy prevalence, population composition, utility,
//! convergence, oscillation, divergence, and intervention rates over time.
use matchlab_core::time::SimTime;
/// Population snapshot at a point in time.
#[derive(Debug, Clone)]
pub struct PopulationSnapshot {
    pub total_players: usize,
    pub active_players: usize,
    pub party_count: usize,
    pub average_rating: f64,
    pub strategy_distribution: std::collections::HashMap<String, f64>,
}
/// Collected stability metrics over time.
#[derive(Debug, Clone, Default)]
pub struct StabilityMetrics {
    pub strategy_prevalence: std::collections::HashMap<String, Vec<(SimTime, f64)>>,
    pub population_over_time: Vec<(SimTime, PopulationSnapshot)>,
    pub utility_over_time: Vec<(SimTime, f64)>,
    pub queue_time_convergence: Vec<(SimTime, f64)>,
    pub match_quality_convergence: Vec<(SimTime, f64)>,
    pub intervention_count: usize,
    pub adversarial_prevalence: f64,
}
impl StabilityMetrics {
    /// Record a population snapshot.
    pub fn record_population(&mut self, time: SimTime, snapshot: PopulationSnapshot) {
        self.population_over_time.push((time, snapshot));
    }
    /// Record a utility value.
    pub fn record_utility(&mut self, time: SimTime, utility: f64) {
        self.utility_over_time.push((time, utility));
    }
    /// Record a queue time observation.
    pub fn record_queue_time(&mut self, time: SimTime, queue_time: f64) {
        self.queue_time_convergence.push((time, queue_time));
    }
    /// Record a match quality observation.
    pub fn record_match_quality(&mut self, time: SimTime, quality: f64) {
        self.match_quality_convergence.push((time, quality));
    }
    /// Record an intervention.
    pub fn record_intervention(&mut self) {
        self.intervention_count += 1;
    }
    /// Record strategy prevalence at a point in time.
    pub fn record_strategy_prevalence(&mut self, time: SimTime, strategy: &str, prevalence: f64) {
        self.strategy_prevalence
            .entry(strategy.to_string())
            .or_default()
            .push((time, prevalence));
    }
}
/// Verdict on population stability.
#[derive(Debug, Clone)]
pub enum StabilityVerdict {
    /// Population has stabilized.
    Stable { confidence: f64 },
    /// Population is oscillating with a detectable frequency.
    Oscillating { frequency: f64 },
    /// Population is diverging.
    Diverging { rate: f64 },
    /// Population was transient but settled.
    Transient { settled_at: SimTime },
}
/// Analyze stability metrics to produce a verdict.
pub fn compute_stability(metrics: &StabilityMetrics) -> StabilityVerdict {
    if metrics.utility_over_time.len() < 2 {
        return StabilityVerdict::Transient {
            settled_at: SimTime::ZERO,
        };
    }
    let recent: Vec<f64> = metrics
        .utility_over_time
        .iter()
        .rev()
        .take(10)
        .map(|(_, u)| *u)
        .collect();
    if recent.len() >= 2 {
        let mean = recent.iter().sum::<f64>() / recent.len() as f64;
        let variance = recent.iter().map(|u| (u - mean).powi(2)).sum::<f64>() / recent.len() as f64;
        let cv = if mean.abs() > 1e-12 {
            variance.sqrt() / mean.abs()
        } else {
            0.0
        };
        if cv < 0.05 {
            return StabilityVerdict::Stable { confidence: 0.9 };
        }
        if cv > 0.5 {
            return StabilityVerdict::Diverging { rate: cv };
        }
    }
    if recent.len() >= 4 {
        let mut sign_changes = 0;
        for w in recent.windows(2) {
            let d = w[1] - w[0];
            if d.abs() > 0.01 {}
        }
        for w in recent.windows(3) {
            let d1 = w[1] - w[0];
            let d2 = w[2] - w[1];
            if d1 * d2 < 0.0 {
                sign_changes += 1;
            }
        }
        if sign_changes >= 2 {
            return StabilityVerdict::Oscillating { frequency: 1.0 };
        }
    }
    StabilityVerdict::Transient {
        settled_at: metrics
            .utility_over_time
            .last()
            .map(|(t, _)| *t)
            .unwrap_or(SimTime::ZERO),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_metrics_produce_stable_verdict() {
        let mut metrics = StabilityMetrics::default();
        for i in 0..20 {
            metrics.record_utility(SimTime::from_secs(i as f64), 100.0);
        }
        let verdict = compute_stability(&metrics);
        assert!(matches!(verdict, StabilityVerdict::Stable { .. }));
    }
    #[test]
    fn empty_metrics_produce_transient() {
        let metrics = StabilityMetrics::default();
        let verdict = compute_stability(&metrics);
        assert!(matches!(verdict, StabilityVerdict::Transient { .. }));
    }
    #[test]
    fn oscillating_metrics_detected() {
        let mut metrics = StabilityMetrics::default();
        for i in 0..20 {
            let utility = if i % 2 == 0 { 100.0 } else { 50.0 };
            metrics.record_utility(SimTime::from_secs(i as f64), utility);
        }
        let verdict = compute_stability(&metrics);
        assert!(
            matches!(verdict, StabilityVerdict::Oscillating { .. }),
            "alternating pattern should be detected as oscillating"
        );
    }
    #[test]
    fn metrics_are_deterministic() {
        let mut m1 = StabilityMetrics::default();
        let mut m2 = StabilityMetrics::default();
        for i in 0..10 {
            m1.record_utility(SimTime::from_secs(i as f64), 100.0);
            m2.record_utility(SimTime::from_secs(i as f64), 100.0);
        }
        let v1 = compute_stability(&m1);
        let v2 = compute_stability(&m2);
        assert!(format!("{:?}", v1) == format!("{:?}", v2));
    }
}
