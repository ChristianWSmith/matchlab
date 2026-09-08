//! Skill distributions and correlation: configurable skill
//! distributions with correlation structure for generating multidimensional
//! skill vectors.
use matchlab_core::player::SkillVector;
use matchlab_core::rng::SimRng;
/// A marginal distribution for a single skill dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct Marginal {
    pub name: String,
    pub distribution: DistributionConfig,
}
/// Supported distribution types.
#[derive(Debug, Clone, PartialEq)]
pub enum DistributionConfig {
    Normal { mean: f64, stddev: f64 },
    LogNormal { mean: f64, stddev: f64 },
    Uniform { min: f64, max: f64 },
}
/// Skill distribution specification: either independent or correlated.
#[derive(Debug, Clone)]
pub enum SkillDistribution {
    /// Each dimension is drawn independently.
    Independent(Vec<Marginal>),
    /// Multivariate normal with correlation structure.
    MultivariateNormal {
        means: Vec<f64>,
        covariance: Vec<Vec<f64>>,
        dimension_names: Vec<String>,
    },
}
/// Generate correlated normal samples using Cholesky decomposition.
/// Given a covariance matrix, produce correlated standard normals.
pub fn cholesky_decompose(cov: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = cov.len();
    let mut l = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let sum: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            let diag = if i == j {
                let val = cov[i][i] - sum;
                if val <= 0.0 {
                    return None;
                }
                val.sqrt()
            } else {
                (cov[i][j] - sum) / l[j][j]
            };
            l[i][j] = diag;
        }
    }
    Some(l)
}
/// Sample from a multivariate normal distribution.
pub fn sample_multivariate_normal(means: &[f64], chol: &[Vec<f64>], rng: &mut SimRng) -> Vec<f64> {
    let n = means.len();
    let z: Vec<f64> = (0..n).map(|_| rng.sample_normal(0.0, 1.0)).collect();
    (0..n)
        .map(|i| {
            let mut val = means[i];
            for j in 0..=i {
                val += chol[i][j] * z[j];
            }
            val
        })
        .collect()
}
/// Draw a skill vector from the configured distribution.
pub fn draw_skill_vector(dist: &SkillDistribution, rng: &mut SimRng) -> SkillVector {
    match dist {
        SkillDistribution::Independent(marginals) => {
            let dimensions = marginals
                .iter()
                .map(|m| {
                    let val = match &m.distribution {
                        DistributionConfig::Normal { mean, stddev } => {
                            rng.sample_normal(*mean, *stddev)
                        }
                        DistributionConfig::LogNormal { mean, stddev } => {
                            rng.sample_normal(*mean, *stddev).exp()
                        }
                        DistributionConfig::Uniform { min, max } => {
                            let range = max - min;
                            min + rng.gen_range(0.0, 1.0) * range
                        }
                    };
                    (m.name.clone(), val)
                })
                .collect();
            SkillVector { dimensions }
        }
        SkillDistribution::MultivariateNormal {
            means,
            covariance,
            dimension_names,
        } => {
            let chol =
                cholesky_decompose(covariance).expect("covariance must be positive definite");
            let values = sample_multivariate_normal(means, &chol, rng);
            let dimensions = dimension_names
                .iter()
                .zip(values)
                .map(|(name, val)| (name.clone(), val))
                .collect();
            SkillVector { dimensions }
        }
    }
}
/// Pearson correlation coefficient between two samples.
pub fn pearson_correlation(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "samples must have equal length");
    let n = a.len() as f64;
    let mean_a: f64 = a.iter().sum::<f64>() / n;
    let mean_b: f64 = b.iter().sum::<f64>() / n;
    let mut cov_ab = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    for (ai, bi) in a.iter().zip(b) {
        let da = ai - mean_a;
        let db = bi - mean_b;
        cov_ab += da * db;
        var_a += da * da;
        var_b += db * db;
    }
    let denom = (var_a * var_b).sqrt();
    if denom < 1e-12 { 0.0 } else { cov_ab / denom }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_distribution_produces_correct_dimensions() {
        let dist = SkillDistribution::Independent(vec![
            Marginal {
                name: "aim".to_string(),
                distribution: DistributionConfig::Normal {
                    mean: 100.0,
                    stddev: 10.0,
                },
            },
            Marginal {
                name: "movement".to_string(),
                distribution: DistributionConfig::Normal {
                    mean: 200.0,
                    stddev: 10.0,
                },
            },
        ]);
        let mut rng = SimRng::from_seed(42);
        let sv = draw_skill_vector(&dist, &mut rng);
        assert_eq!(sv.ndim(), 2);
        assert!(sv.dimensions.contains_key("aim"));
        assert!(sv.dimensions.contains_key("movement"));
    }
    #[test]
    fn cholesky_identity_for_diagonal_covariance() {
        let cov = vec![vec![4.0, 0.0], vec![0.0, 9.0]];
        let chol = cholesky_decompose(&cov).unwrap();
        assert!((chol[0][0] - 2.0).abs() < 1e-12);
        assert!((chol[1][1] - 3.0).abs() < 1e-12);
        assert!(chol[1][0].abs() < 1e-12);
    }
    #[test]
    fn cholesky_returns_none_for_non_positive_definite() {
        let cov = vec![vec![1.0, 2.0], vec![2.0, 1.0]];
        assert!(cholesky_decompose(&cov).is_none());
    }
    #[test]
    fn multivariate_normal_produces_correct_dimensions() {
        let means = vec![100.0, 200.0];
        let cov = vec![vec![100.0, 0.0], vec![0.0, 100.0]];
        let dist = SkillDistribution::MultivariateNormal {
            means: means.clone(),
            covariance: cov,
            dimension_names: vec!["a".to_string(), "b".to_string()],
        };
        let mut rng = SimRng::from_seed(42);
        let sv = draw_skill_vector(&dist, &mut rng);
        assert_eq!(sv.ndim(), 2);
        assert!(sv.dimensions.contains_key("a"));
        assert!(sv.dimensions.contains_key("b"));
    }
    #[test]
    fn independent_dimensions_have_near_zero_correlation() {
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
        assert!(
            r.abs() < 0.1,
            "independent dimensions should have near-zero correlation, got {r}"
        );
    }
    #[test]
    fn pearson_correlation_known_values() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = pearson_correlation(&a, &b);
        assert!((r - 1.0).abs() < 1e-9, "perfect positive correlation");
        let c = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let r2 = pearson_correlation(&a, &c);
        assert!((r2 + 1.0).abs() < 1e-9, "perfect negative correlation");
    }
    #[test]
    fn pearson_correlation_constant_is_zero() {
        let a = vec![5.0, 5.0, 5.0, 5.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let r = pearson_correlation(&a, &b);
        assert!(r.abs() < 1e-9, "constant input gives zero correlation");
    }
    #[test]
    fn lognormal_distribution_positive() {
        let dist = SkillDistribution::Independent(vec![Marginal {
            name: "x".to_string(),
            distribution: DistributionConfig::LogNormal {
                mean: 5.0,
                stddev: 1.0,
            },
        }]);
        let mut rng = SimRng::from_seed(42);
        for _ in 0..100 {
            let sv = draw_skill_vector(&dist, &mut rng);
            assert!(sv.dimensions["x"] > 0.0, "lognormal must be positive");
        }
    }
    #[test]
    fn uniform_distribution_within_bounds() {
        let dist = SkillDistribution::Independent(vec![Marginal {
            name: "x".to_string(),
            distribution: DistributionConfig::Uniform {
                min: 10.0,
                max: 20.0,
            },
        }]);
        let mut rng = SimRng::from_seed(42);
        for _ in 0..100 {
            let sv = draw_skill_vector(&dist, &mut rng);
            assert!(sv.dimensions["x"] >= 10.0 && sv.dimensions["x"] <= 20.0);
        }
    }
}
