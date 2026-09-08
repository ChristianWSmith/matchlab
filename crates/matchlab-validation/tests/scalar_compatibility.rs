//! Scalar compatibility and validation models: ensures every new
//! model has a deliberately simple limiting case that reproduces the behavior
//! of the previous scalar/stationary model.
use matchlab_core::player::SkillVector;
use matchlab_core::rng::SimRng;
/// Assert that a complex model and a simple model produce the same result
/// within the given tolerance.
pub fn assert_limiting_case_eq<F, G>(complex: F, simple: G, tolerance: f64)
where
    F: Fn() -> f64,
    G: Fn() -> f64,
{
    let c = complex();
    let s = simple();
    assert!(
        (c - s).abs() <= tolerance,
        "limiting case mismatch: complex={c}, simple={s}, tolerance={tolerance}"
    );
}
/// Assert that two skill vectors are equal within tolerance.
pub fn assert_skill_eq(a: &SkillVector, b: &SkillVector, tolerance: f64) {
    for dim in a.dimension_names() {
        let va = a.dimensions[dim];
        let vb = b.dimensions.get(dim).copied().unwrap_or(0.0);
        assert!(
            (va - vb).abs() <= tolerance,
            "dimension '{dim}': {va} vs {vb} (tolerance {tolerance})"
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::SkillVector;
    use matchlab_game::performance::PerformanceModel;
    use matchlab_game::team::TeamModel;
    use matchlab_players::distribution::*;
    use matchlab_players::dynamics::*;
    #[test]
    fn multidim_collapse_to_1d() {
        let mut dims = std::collections::HashMap::new();
        for d in &[
            "aim",
            "movement",
            "game_sense",
            "decision_making",
            "teamwork",
        ] {
            dims.insert(d.to_string(), 1200.0);
        }
        let multi = SkillVector { dimensions: dims };
        let single = SkillVector::one_dimensional(1200.0);
        assert_limiting_case_eq(|| multi.overall(), || single.overall(), 1e-9);
    }
    #[test]
    fn zero_noise_gives_deterministic_performance() {
        let model = matchlab_game::performance::GaussianNoiseModel::deterministic();
        let sv = SkillVector::one_dimensional(1500.0);
        let ctx = matchlab_game::performance::PerformanceContext::default();
        let mut rng = SimRng::from_seed(42);
        let perf1 = model.realize(&sv, &ctx, &mut rng);
        let mut rng = SimRng::from_seed(42);
        let perf2 = model.realize(&sv, &ctx, &mut rng);
        assert_skill_eq(&perf1, &perf2, 1e-12);
    }
    #[test]
    fn zero_learning_rate_gives_stationary_skill() {
        let linear = LinearDynamics {
            improvement_rate: 0.0,
            decline_rate: 0.0,
            volatility: 0.0,
        };
        let sv = SkillVector::one_dimensional(1200.0);
        let ctx = DynamicsContext::default();
        let mut rng = SimRng::from_seed(42);
        let new_sv = linear.advance(&sv, &ctx, &mut rng);
        assert_skill_eq(&sv, &new_sv, 1e-9);
    }
    #[test]
    fn zero_correlation_gives_independent() {
        let dist = SkillDistribution::Independent(vec![
            Marginal {
                name: "a".to_string(),
                distribution: DistributionConfig::Normal {
                    mean: 0.0,
                    stddev: 10.0,
                },
            },
            Marginal {
                name: "b".to_string(),
                distribution: DistributionConfig::Normal {
                    mean: 0.0,
                    stddev: 10.0,
                },
            },
        ]);
        let mut rng = SimRng::from_seed(42);
        let mut a_vals = Vec::new();
        let mut b_vals = Vec::new();
        for _ in 0..1000 {
            let sv = draw_skill_vector(&dist, &mut rng);
            a_vals.push(sv.dimensions["a"]);
            b_vals.push(sv.dimensions["b"]);
        }
        let r = pearson_correlation(&a_vals, &b_vals);
        assert!(r.abs() < 0.1, "zero correlation: got {r}");
    }
    #[test]
    fn zero_synergy_gives_additive() {
        let additive = matchlab_game::team::AdditiveTeamModel;
        let complementary = matchlab_game::team::ComplementaryTeamModel {
            synergy_bonus: 0.0,
            complementarity_threshold: 0.0,
        };
        let ctx = matchlab_game::team::TeamContext {
            team_size: 2,
            role_requirements: None,
        };
        let players = vec![
            SkillVector::one_dimensional(100.0),
            SkillVector::one_dimensional(200.0),
        ];
        let s = additive.team_strength(&players, &ctx);
        let c = complementary.team_strength(&players, &ctx);
        assert_limiting_case_eq(|| c, || s, 1e-9);
    }
    #[test]
    fn scalar_skill_vector_backward_compat() {
        let sv = SkillVector::one_dimensional(1200.0);
        assert_eq!(sv.ndim(), 1);
        assert_eq!(sv.overall(), 1200.0);
        assert_eq!(sv.dimensions["overall"], 1200.0);
    }
}
