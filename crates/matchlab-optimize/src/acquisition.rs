use crate::gp::GaussianProcess;
use ndarray::{Array1, Array2};
use rand::Rng;
use rand::SeedableRng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionKind {
    EI,
    UCB,
    PI,
}

impl AcquisitionKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "ei" | "expected_improvement" => Ok(Self::EI),
            "ucb" | "upper_confidence_bound" => Ok(Self::UCB),
            "pi" | "probability_of_improvement" => Ok(Self::PI),
            _ => Err(format!("unknown acquisition function: {s}")),
        }
    }
}

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

pub fn probability_of_improvement(
    gp: &GaussianProcess,
    x: &Array2<f64>,
    best_y: f64,
    xi: f64,
) -> Array1<f64> {
    let (mean, std) = gp.predict(x);
    let n = mean.len();
    let mut pi = Array1::<f64>::zeros(n);

    for i in 0..n {
        let s = std[i];
        if s < 1e-10 {
            pi[i] = if mean[i] > best_y + xi { 1.0 } else { 0.0 };
            continue;
        }
        let z = (mean[i] - best_y - xi) / s;
        pi[i] = 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
    }
    pi
}

pub fn evaluate_acquisition(
    kind: AcquisitionKind,
    gp: &GaussianProcess,
    x: &Array2<f64>,
    best_y: f64,
    xi: f64,
    beta: f64,
) -> Array1<f64> {
    match kind {
        AcquisitionKind::EI => expected_improvement(gp, x, best_y, xi),
        AcquisitionKind::UCB => upper_confidence_bound(gp, x, beta),
        AcquisitionKind::PI => probability_of_improvement(gp, x, best_y, xi),
    }
}

pub fn erf(x: f64) -> f64 {
    1.0 - erfc(x)
}

pub fn erfc(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let val = poly * (-x * x).exp();
    if x >= 0.0 { val } else { 2.0 - val }
}

pub fn parego_scalarize(objectives: &[f64], weights: &[f64], directions: &[bool], eta: f64) -> f64 {
    let mut max_exp_arg = f64::NEG_INFINITY;
    let mut exp_args = Vec::with_capacity(objectives.len());
    for (i, (&obj, &w)) in objectives.iter().zip(weights.iter()).enumerate() {
        let val = if directions[i] { obj } else { -obj };
        let _ = w;
        let arg = -val / eta;
        exp_args.push(arg);
        if arg > max_exp_arg {
            max_exp_arg = arg;
        }
    }
    let mut s = 0.0;
    for (i, &w) in weights.iter().enumerate() {
        s += w * (exp_args[i] - max_exp_arg).exp();
    }
    -eta * (max_exp_arg + s.ln())
}

pub fn parego_weights(n_objectives: usize, seed: u64) -> Vec<f64> {
    if n_objectives == 0 {
        return vec![];
    }
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
    eta: f64,
) -> (usize, f64) {
    let mut best_idx = 0;
    let mut best_val = f64::NEG_INFINITY;
    for (i, obj) in objectives.iter().enumerate() {
        let val = parego_scalarize(obj, weights, directions, eta);
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
    fn erf_negative() {
        assert!((erf(-3.0) + 1.0).abs() < 0.01);
    }

    #[test]
    fn parego_weights_sum_to_one() {
        let w = parego_weights(3, 42);
        let sum: f64 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn acquisition_kind_parse() {
        assert_eq!(AcquisitionKind::parse("ei").unwrap(), AcquisitionKind::EI);
        assert_eq!(
            AcquisitionKind::parse("expected_improvement").unwrap(),
            AcquisitionKind::EI
        );
        assert_eq!(AcquisitionKind::parse("ucb").unwrap(), AcquisitionKind::UCB);
        assert_eq!(AcquisitionKind::parse("pi").unwrap(), AcquisitionKind::PI);
        assert_eq!(
            AcquisitionKind::parse("probability_of_improvement").unwrap(),
            AcquisitionKind::PI
        );
        assert!(AcquisitionKind::parse("unknown").is_err());
    }

    #[test]
    fn parego_scalarize_single_objective() {
        let obj = vec![1.0];
        let weights = vec![1.0];
        let dirs = vec![true];
        let val = parego_scalarize(&obj, &weights, &dirs, 0.05);
        assert!(val > 0.0);
    }

    #[test]
    fn parego_scalarize_respects_eta() {
        let obj = vec![1.0, 2.0];
        let weights = vec![0.5, 0.5];
        let dirs = vec![true, true];
        let val_low = parego_scalarize(&obj, &weights, &dirs, 0.01);
        let val_high = parego_scalarize(&obj, &weights, &dirs, 0.5);
        assert!(val_low != val_high);
    }

    #[test]
    fn erf_erfc_roundtrip() {
        for x in [-3.0, -1.0, -0.5, 0.0, 0.5, 1.0, 3.0] {
            let sum = erf(x) + erfc(x);
            assert!(
                (sum - 1.0).abs() < 1e-6,
                "erf({x}) + erfc({x}) = {sum}, expected 1.0"
            );
        }
    }

    #[test]
    fn erfc_zero() {
        assert!((erfc(0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn erf_one() {
        let val = erf(1.0);
        assert!(val > 0.84 && val < 0.85, "erf(1.0) = {val}");
    }

    #[test]
    fn parego_weights_zero_objectives() {
        let w = parego_weights(0, 42);
        assert!(w.is_empty());
    }

    #[test]
    fn ei_non_negative() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let params = crate::kernel::KernelParams::new(1, vec![], vec![]);
        let gp = crate::gp::GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let x_cand = Array2::from_shape_vec((5, 1), vec![-0.5, 0.25, 0.5, 0.75, 1.5]).unwrap();
        let ei = expected_improvement(&gp, &x_cand, 1.0, 0.01);
        for v in ei.iter() {
            assert!(*v >= 0.0, "EI should be non-negative, got {v}");
        }
    }

    #[test]
    fn ucb_greater_than_mean() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let params = crate::kernel::KernelParams::new(1, vec![], vec![]);
        let gp = crate::gp::GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let x_cand = Array2::from_shape_vec((3, 1), vec![0.25, 0.5, 0.75]).unwrap();
        let ucb = upper_confidence_bound(&gp, &x_cand, 2.0);
        let (mean, _std) = gp.predict(&x_cand);
        for i in 0..3 {
            assert!(
                ucb[i] >= mean[i],
                "UCB[{i}]={} should be >= mean[{i}]={}",
                ucb[i],
                mean[i]
            );
        }
    }

    #[test]
    fn pi_in_unit_interval() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let params = crate::kernel::KernelParams::new(1, vec![], vec![]);
        let gp = crate::gp::GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let x_cand = Array2::from_shape_vec((5, 1), vec![-0.5, 0.25, 0.5, 0.75, 1.5]).unwrap();
        let pi = probability_of_improvement(&gp, &x_cand, 0.5, 0.01);
        for v in pi.iter() {
            assert!(*v >= 0.0 && *v <= 1.0, "PI should be in [0,1], got {v}");
        }
    }

    #[test]
    fn evaluate_acquisition_dispatch() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let params = crate::kernel::KernelParams::new(1, vec![], vec![]);
        let gp = crate::gp::GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let x_cand = Array2::from_shape_vec((3, 1), vec![0.25, 0.5, 0.75]).unwrap();
        let best_y = 1.0;
        let xi = 0.01;
        let beta = 2.0;

        let ei = evaluate_acquisition(AcquisitionKind::EI, &gp, &x_cand, best_y, xi, beta);
        let ucb = evaluate_acquisition(AcquisitionKind::UCB, &gp, &x_cand, best_y, xi, beta);
        let pi = evaluate_acquisition(AcquisitionKind::PI, &gp, &x_cand, best_y, xi, beta);

        assert_eq!(ei.len(), 3);
        assert_eq!(ucb.len(), 3);
        assert_eq!(pi.len(), 3);

        for v in ei.iter() {
            assert!(*v >= 0.0, "EI should be non-negative, got {v}");
        }
        for v in pi.iter() {
            assert!(*v >= 0.0 && *v <= 1.0, "PI should be in [0,1], got {v}");
        }
        let (mean, _) = gp.predict(&x_cand);
        for i in 0..3 {
            assert!(ucb[i] >= mean[i], "UCB should be >= mean");
        }
    }
}
