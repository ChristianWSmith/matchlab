//! Skill as a stochastic process (spec §5.6).
//!
//! v0.1 uses static skill — the population is generated once at `t=0` and does
//! not change. `advance()` is implemented per the spec so later versions can
//! enable skill evolution; with `improvement_rate=0` and `volatility=0` it is
//! the identity (a no-op), which is the v0.1 baseline.

use matchlab_core::player::SkillVector;
use matchlab_core::rng::SimRng;

#[derive(Debug, Clone)]
pub struct SkillProcess {
    pub improvement_rate: f64,
    pub volatility: f64,
}

impl SkillProcess {
    /// Advance all skill dimensions one time step.
    pub fn advance(&self, current: &SkillVector, rng: &mut SimRng) -> SkillVector {
        let mut new_dims = std::collections::HashMap::new();
        for (dim, &val) in &current.dimensions {
            let noise = rng.sample_normal(0.0, self.volatility);
            new_dims.insert(dim.clone(), (val + self.improvement_rate + noise).max(0.0));
        }
        SkillVector {
            dimensions: new_dims,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_process_is_identity_when_no_improvement_and_no_volatility() {
        let process = SkillProcess {
            improvement_rate: 0.0,
            volatility: 0.0,
        };
        let mut rng = SimRng::from_seed(1);
        let skill = SkillVector::one_dimensional(1200.0);
        let advanced = process.advance(&skill, &mut rng);
        assert_eq!(advanced.overall(), 1200.0);
    }

    #[test]
    fn improvement_rate_drifts_skill_up() {
        let process = SkillProcess {
            improvement_rate: 2.0,
            volatility: 0.0,
        };
        let mut rng = SimRng::from_seed(1);
        let skill = SkillVector::one_dimensional(1000.0);
        let advanced = process.advance(&skill, &mut rng);
        assert!((advanced.overall() - 1002.0).abs() < 1e-9);
    }

    #[test]
    fn volatility_adds_noise_bounded_or_shrinking() {
        let process = SkillProcess {
            improvement_rate: 0.0,
            volatility: 5.0,
        };
        let mut rng = SimRng::from_seed(1);
        let skill = SkillVector::one_dimensional(1000.0);
        let advanced = process.advance(&skill, &mut rng);
        // Built from a normal sample around 1000; must not be exactly equal.
        assert!((advanced.overall() - 1000.0).abs() > 0.0);
    }
}
