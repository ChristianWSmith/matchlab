//! Skill dynamics: controlled models for skill evolution:
//! improvement, decline, volatility, experience-dependent learning, and
//! inactivity effects.
use matchlab_core::player::SkillVector;
use matchlab_core::rng::SimRng;
/// Contextual information for skill dynamics.
#[derive(Debug, Clone, Default)]
pub struct DynamicsContext {
    pub games_played: u64,
    pub account_age: u64,
    pub time_inactive: u64,
    pub current_skill: f64,
}
/// A model for how skill evolves over time.
pub trait SkillDynamics: Send + Sync {
    /// Advance skill by one time step given the current state.
    fn advance(
        &self,
        skill: &SkillVector,
        context: &DynamicsContext,
        rng: &mut SimRng,
    ) -> SkillVector;
}
/// Linear dynamics: skill += improvement_rate + N(0, volatility).
/// This is the current `SkillProcess::advance()` formalized.
#[derive(Debug, Clone)]
pub struct LinearDynamics {
    pub improvement_rate: f64,
    pub decline_rate: f64,
    pub volatility: f64,
}
impl SkillDynamics for LinearDynamics {
    fn advance(
        &self,
        skill: &SkillVector,
        _context: &DynamicsContext,
        rng: &mut SimRng,
    ) -> SkillVector {
        let dimensions = skill
            .dimensions
            .iter()
            .map(|(dim, &val)| {
                let drift = self.improvement_rate - self.decline_rate;
                let noise = if self.volatility > 0.0 {
                    rng.sample_normal(0.0, self.volatility)
                } else {
                    0.0
                };
                (dim.clone(), val + drift + noise)
            })
            .collect();
        SkillVector { dimensions }
    }
}
/// Experience-dependent dynamics: improvement rate decreases with games played,
/// converging to a plateau.
#[derive(Debug, Clone)]
pub struct ExperienceDynamics {
    pub learning_rate: f64,
    pub plateau_games: u64,
    pub volatility: f64,
}
impl SkillDynamics for ExperienceDynamics {
    fn advance(
        &self,
        skill: &SkillVector,
        context: &DynamicsContext,
        rng: &mut SimRng,
    ) -> SkillVector {
        let ratio = (context.games_played as f64 / self.plateau_games as f64).min(1.0);
        let effective_rate = self.learning_rate * (1.0 - ratio);
        let dimensions = skill
            .dimensions
            .iter()
            .map(|(dim, &val)| {
                let noise = if self.volatility > 0.0 {
                    rng.sample_normal(0.0, self.volatility)
                } else {
                    0.0
                };
                (dim.clone(), val + effective_rate + noise)
            })
            .collect();
        SkillVector { dimensions }
    }
}
/// Decay dynamics: skill declines after periods of inactivity.
#[derive(Debug, Clone)]
pub struct DecayDynamics {
    /// Fraction of skill lost per inactive time step (relative to current skill).
    pub decay_rate: f64,
    /// Number of time steps before decay begins.
    pub inactive_threshold: u64,
    /// Floor value to prevent skill from decaying to zero.
    pub min_skill: f64,
}
impl SkillDynamics for DecayDynamics {
    fn advance(
        &self,
        skill: &SkillVector,
        context: &DynamicsContext,
        _rng: &mut SimRng,
    ) -> SkillVector {
        if context.time_inactive <= self.inactive_threshold {
            return skill.clone();
        }
        let inactive_steps = context.time_inactive - self.inactive_threshold;
        let decay_factor = (1.0 - self.decay_rate).powi(inactive_steps as i32);
        let dimensions = skill
            .dimensions
            .iter()
            .map(|(dim, &val)| {
                let decayed = val * decay_factor;
                (dim.clone(), decayed.max(self.min_skill))
            })
            .collect();
        SkillVector { dimensions }
    }
}
/// No-op dynamics: skill remains stationary.
pub struct StationaryDynamics;
impl SkillDynamics for StationaryDynamics {
    fn advance(
        &self,
        skill: &SkillVector,
        _context: &DynamicsContext,
        _rng: &mut SimRng,
    ) -> SkillVector {
        skill.clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_dynamics_adds_improvement_rate() {
        let dyn_model = LinearDynamics {
            improvement_rate: 5.0,
            decline_rate: 0.0,
            volatility: 0.0,
        };
        let sv = SkillVector::one_dimensional(100.0);
        let ctx = DynamicsContext::default();
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert!((new_sv.overall() - 105.0).abs() < 1e-9);
    }
    #[test]
    fn linear_dynamics_subtracts_decline_rate() {
        let dyn_model = LinearDynamics {
            improvement_rate: 0.0,
            decline_rate: 3.0,
            volatility: 0.0,
        };
        let sv = SkillVector::one_dimensional(100.0);
        let ctx = DynamicsContext::default();
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert!((new_sv.overall() - 97.0).abs() < 1e-9);
    }
    #[test]
    fn linear_dynamics_zero_volatility_deterministic() {
        let dyn_model = LinearDynamics {
            improvement_rate: 1.0,
            decline_rate: 0.0,
            volatility: 0.0,
        };
        let sv = SkillVector::one_dimensional(100.0);
        let ctx = DynamicsContext::default();
        let mut rng1 = SimRng::from_seed(42);
        let mut rng2 = SimRng::from_seed(42);
        let new1 = dyn_model.advance(&sv, &ctx, &mut rng1);
        let new2 = dyn_model.advance(&sv, &ctx, &mut rng2);
        assert_eq!(new1.overall(), new2.overall());
    }
    #[test]
    fn experience_dynamics_converges_to_plateau() {
        let dyn_model = ExperienceDynamics {
            learning_rate: 10.0,
            plateau_games: 100,
            volatility: 0.0,
        };
        let sv = SkillVector::one_dimensional(100.0);
        let mut rng = SimRng::from_seed(42);
        let mut skill = sv.clone();
        for i in 0..150 {
            let ctx = DynamicsContext {
                games_played: i,
                ..Default::default()
            };
            skill = dyn_model.advance(&skill, &ctx, &mut rng);
        }
        assert!(
            skill.overall() > 500.0,
            "should converge toward plateau: {}",
            skill.overall()
        );
    }
    #[test]
    fn experience_dynamics_zero_games_full_rate() {
        let dyn_model = ExperienceDynamics {
            learning_rate: 5.0,
            plateau_games: 100,
            volatility: 0.0,
        };
        let sv = SkillVector::one_dimensional(100.0);
        let ctx = DynamicsContext {
            games_played: 0,
            ..Default::default()
        };
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert!((new_sv.overall() - 105.0).abs() < 1e-9);
    }
    #[test]
    fn decay_dynamics_no_decay_below_threshold() {
        let dyn_model = DecayDynamics {
            decay_rate: 0.1,
            inactive_threshold: 5,
            min_skill: 100.0,
        };
        let sv = SkillVector::one_dimensional(1000.0);
        let ctx = DynamicsContext {
            time_inactive: 3,
            ..Default::default()
        };
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert!((new_sv.overall() - 1000.0).abs() < 1e-9);
    }
    #[test]
    fn decay_dynamics_reduces_after_threshold() {
        let dyn_model = DecayDynamics {
            decay_rate: 0.5,
            inactive_threshold: 0,
            min_skill: 100.0,
        };
        let sv = SkillVector::one_dimensional(1000.0);
        let ctx = DynamicsContext {
            time_inactive: 1,
            ..Default::default()
        };
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert!((new_sv.overall() - 500.0).abs() < 1e-9);
    }
    #[test]
    fn decay_dynamics_respects_min_skill() {
        let dyn_model = DecayDynamics {
            decay_rate: 0.9,
            inactive_threshold: 0,
            min_skill: 500.0,
        };
        let sv = SkillVector::one_dimensional(1000.0);
        let ctx = DynamicsContext {
            time_inactive: 10,
            ..Default::default()
        };
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert!((new_sv.overall() - 500.0).abs() < 1e-9);
    }
    #[test]
    fn stationary_dynamics_preserves_skill() {
        let dyn_model = StationaryDynamics;
        let sv = SkillVector::one_dimensional(1500.0);
        let ctx = DynamicsContext::default();
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        assert_eq!(new_sv.overall(), 1500.0);
    }
    #[test]
    fn linear_dynamics_reproduces_current_skill_process() {
        let dyn_model = LinearDynamics {
            improvement_rate: 0.01,
            decline_rate: 0.0,
            volatility: 0.5,
        };
        let sv = SkillVector::one_dimensional(1200.0);
        let ctx = DynamicsContext::default();
        let mut rng = SimRng::from_seed(42);
        let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
        let diff = new_sv.overall() - 1200.0;
        assert!(
            diff > -5.0 && diff < 5.0,
            "diff should be reasonable: {diff}"
        );
    }
}
