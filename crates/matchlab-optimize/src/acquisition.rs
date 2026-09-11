use crate::gp::GaussianProcess;
use ndarray::{Array1, Array2};
use rand::Rng;
use rand::SeedableRng;

pub fn expected_improvement(
    gp: &GaussianProcess,
    x: &Array2<f64>,
    best_y: f64,
    xi: f64,
) -> Array1<f64> {
    let (mean, std) = gp.predict(x);
    let n = mean.len();
    let mut ei = Array1::<f64>::zeros(n);

    for i in 0..n {
        let s = std[i];
        if s < 1e-10 {
            ei[i] = 0.0;
            continue;
        }
        let z = (mean[i] - best_y - xi) / s;
        let phi = 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
        let pdf = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
        ei[i] = (mean[i] - best_y - xi) * phi + s * pdf;
        if ei[i] < 0.0 {
            ei[i] = 0.0;
        }
    }
    ei
}

pub fn upper_confidence_bound(gp: &GaussianProcess, x: &Array2<f64>, beta: f64) -> Array1<f64> {
    let (mean, std) = gp.predict(x);
    mean + beta.sqrt() * std
}

pub fn erfc(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let val = poly * (-x * x).exp();
    if x >= 0.0 { val } else { 2.0 - val }
}

pub fn erf(x: f64) -> f64 {
    1.0 - erfc(x)
}

pub fn parego_scalarize(objectives: &[f64], weights: &[f64], directions: &[bool]) -> f64 {
    let eta = 0.05;
    let mut s = 0.0;
    for (i, (&obj, &w)) in objectives.iter().zip(weights.iter()).enumerate() {
        let val = if directions[i] { obj } else { -obj };
        s += w * (-val / eta).exp();
    }
    -eta * s.ln()
}

pub fn parego_weights(n_objectives: usize, seed: u64) -> Vec<f64> {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut weights = Vec::with_capacity(n_objectives);
    for _ in 0..n_objectives {
        weights.push(-rng.r#gen::<f64>().ln());
    }
    let sum: f64 = weights.iter().sum();
    for w in &mut weights {
        *w /= sum;
    }
    weights
}

pub fn find_best_scalarized(
    objectives: &[Vec<f64>],
    weights: &[f64],
    directions: &[bool],
) -> (usize, f64) {
    let mut best_idx = 0;
    let mut best_val = f64::NEG_INFINITY;
    for (i, obj) in objectives.iter().enumerate() {
        let val = parego_scalarize(obj, weights, directions);
        if val > best_val {
            best_val = val;
            best_idx = i;
        }
    }
    (best_idx, best_val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erf_zero() {
        assert!((erf(0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn erf_large_positive() {
        assert!((erf(3.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn parego_weights_sum_to_one() {
        let w = parego_weights(3, 42);
        let sum: f64 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }
}
