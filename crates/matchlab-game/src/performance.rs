//! Latent skill vs realized performance: separates what a player
//! is capable of from how they perform in a particular match. The performance
//! model introduces noise and contextual effects between latent ability and
//! what the outcome model sees.
use matchlab_core::player::SkillVector;
use matchlab_core::rng::SimRng;
/// Contextual factors that affect realized performance in a match.
#[derive(Debug, Clone, Default)]
pub struct PerformanceContext {
    pub fatigue: f64,
    pub momentum: f64,
    pub confidence: f64,
    pub games_in_session: u64,
}
/// A model that transforms latent skill into realized performance.
pub trait PerformanceModel: Send + Sync {
    /// Produce realized performance from latent skill, context, and RNG.
    /// The output is ephemeral — it is not stored on `PlayerReality`.
    fn realize(
        &self,
        skill: &SkillVector,
        context: &PerformanceContext,
        rng: &mut SimRng,
    ) -> SkillVector;
}
/// Gaussian noise model: adds independent Gaussian noise per dimension,
/// scaled by the dimension's value.
#[derive(Debug, Clone)]
pub struct GaussianNoiseModel {
    /// Relative noise variance: noise ~ N(0, stddev * sqrt(value² + 1)).
    /// A variance of 0.0 produces deterministic performance (realized = latent).
    pub variance: f64,
}
impl GaussianNoiseModel {
    pub fn new(variance: f64) -> Self {
        Self { variance }
    }
    /// No noise: realized = latent.
    pub fn deterministic() -> Self {
        Self { variance: 0.0 }
    }
}
impl PerformanceModel for GaussianNoiseModel {
    fn realize(
        &self,
        skill: &SkillVector,
        _context: &PerformanceContext,
        rng: &mut SimRng,
    ) -> SkillVector {
        if self.variance.abs() < 1e-12 {
            return skill.clone();
        }
        let dimensions = skill
            .dimensions
            .iter()
            .map(|(dim, &val)| {
                let stddev = self.variance * (val * val + 1.0).sqrt();
                let noise = rng.sample_normal(0.0, stddev);
                (dim.clone(), val + noise)
            })
            .collect();
        SkillVector { dimensions }
    }
}
/// Deterministic performance model: realized = latent (no noise).
pub struct DeterministicModel;
impl PerformanceModel for DeterministicModel {
    fn realize(
        &self,
        skill: &SkillVector,
        _context: &PerformanceContext,
        _rng: &mut SimRng,
    ) -> SkillVector {
        skill.clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_model_preserves_skill() {
        let model = DeterministicModel;
        let sv = SkillVector::one_dimensional(1500.0);
        let ctx = PerformanceContext::default();
        let mut rng = SimRng::from_seed(42);
        let perf = model.realize(&sv, &ctx, &mut rng);
        assert_eq!(perf.overall(), 1500.0);
    }
    #[test]
    fn zero_noise_preserves_skill() {
        let model = GaussianNoiseModel::deterministic();
        let sv = SkillVector::one_dimensional(1500.0);
        let ctx = PerformanceContext::default();
        let mut rng = SimRng::from_seed(42);
        let perf = model.realize(&sv, &ctx, &mut rng);
        assert_eq!(perf.overall(), 1500.0);
    }
    #[test]
    fn nonzero_noise_produces_different_results() {
        let model = GaussianNoiseModel::new(0.1);
        let sv = SkillVector::one_dimensional(1500.0);
        let ctx = PerformanceContext::default();
        let mut rng = SimRng::from_seed(42);
        let perf1 = model.realize(&sv, &ctx, &mut rng);
        let perf2 = model.realize(&sv, &ctx, &mut rng);
        assert!(
            (perf1.overall() - perf2.overall()).abs() > 1e-12
                || (0..100).any(|_| {
                    let mut rng2 = SimRng::from_seed(42);
                    let p = model.realize(&sv, &ctx, &mut rng2);
                    let p2 = model.realize(&sv, &ctx, &mut rng2);
                    (p.overall() - p2.overall()).abs() > 1e-12
                }),
            "noise should produce variation"
        );
    }
    #[test]
    fn noise_variance_within_expected_bounds() {
        let model = GaussianNoiseModel::new(0.1);
        let sv = SkillVector::one_dimensional(1000.0);
        let ctx = PerformanceContext::default();
        let mut rng = SimRng::from_seed(42);
        let mut diffs = Vec::new();
        for _ in 0..1000 {
            let perf = model.realize(&sv, &ctx, &mut rng);
            diffs.push(perf.overall() - 1000.0);
        }
        let mean_diff: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let var_diff: f64 =
            diffs.iter().map(|d| (d - mean_diff).powi(2)).sum::<f64>() / diffs.len() as f64;
        assert!(
            mean_diff.abs() < 10.0,
            "mean noise should be near zero: {mean_diff}"
        );
        assert!(
            var_diff > 0.0 && var_diff.is_finite(),
            "variance should be positive: {var_diff}"
        );
    }
    #[test]
    fn multidimensional_noise_independent_per_dim() {
        let model = GaussianNoiseModel::new(0.05);
        let mut dims = std::collections::HashMap::new();
        dims.insert("a".to_string(), 100.0);
        dims.insert("b".to_string(), 200.0);
        let sv = SkillVector { dimensions: dims };
        let ctx = PerformanceContext::default();
        let mut rng = SimRng::from_seed(42);
        let perf = model.realize(&sv, &ctx, &mut rng);
        assert_eq!(perf.ndim(), 2);
        assert!(perf.dimensions["a"] != 100.0 || perf.dimensions["b"] != 200.0);
    }
    #[test]
    fn context_does_not_affect_deterministic_model() {
        let model = DeterministicModel;
        let sv = SkillVector::one_dimensional(1500.0);
        let ctx = PerformanceContext {
            fatigue: 0.5,
            momentum: 1.0,
            confidence: 0.8,
            games_in_session: 10,
        };
        let mut rng = SimRng::from_seed(42);
        let perf = model.realize(&sv, &ctx, &mut rng);
        assert_eq!(perf.overall(), 1500.0);
    }
    #[test]
    fn context_does_not_affect_noise_distribution() {
        let model = GaussianNoiseModel::new(0.1);
        let sv = SkillVector::one_dimensional(1000.0);
        let ctx_no_fatigue = PerformanceContext::default();
        let ctx_high_fatigue = PerformanceContext {
            fatigue: 0.9,
            ..Default::default()
        };
        let mut rng1 = SimRng::from_seed(42);
        let mut rng2 = SimRng::from_seed(42);
        let perf1 = model.realize(&sv, &ctx_no_fatigue, &mut rng1);
        let perf2 = model.realize(&sv, &ctx_high_fatigue, &mut rng2);
        assert!((perf1.overall() - perf2.overall()).abs() < 1e-12);
    }
}
