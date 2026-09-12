use ndarray::{Array1, Array2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelKind {
    Matern52,
    Matern32,
    RBF,
    RationalQuadratic,
}

impl KernelKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "matern52" | "matern_52" => Ok(Self::Matern52),
            "matern32" | "matern_32" => Ok(Self::Matern32),
            "rbf" | "squared_exponential" | "se" => Ok(Self::RBF),
            "rq" | "rational_quadratic" => Ok(Self::RationalQuadratic),
            _ => Err(format!("unknown kernel: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct KernelParams {
    pub length_scales: Array1<f64>,
    pub signal_variance: f64,
    pub noise_variance: f64,
    pub categorical_indices: Vec<usize>,
    pub categorical_n_levels: Vec<usize>,
    pub kernel_kind: KernelKind,
    pub rq_alpha: f64,
}

impl KernelParams {
    pub fn new(
        n_continuous: usize,
        categorical_indices: Vec<usize>,
        categorical_n_levels: Vec<usize>,
    ) -> Self {
        Self {
            length_scales: Array1::ones(n_continuous),
            signal_variance: 1.0,
            noise_variance: 0.1,
            categorical_indices,
            categorical_n_levels,
            kernel_kind: KernelKind::Matern52,
            rq_alpha: 1.0,
        }
    }
}

pub fn matern52(r: f64) -> f64 {
    if r < 1e-10 {
        return 1.0;
    }
    let sqrt5_r = 5.0_f64.sqrt() * r;
    (1.0 + sqrt5_r + sqrt5_r * sqrt5_r / 3.0) * (-sqrt5_r).exp()
}

pub fn matern32(r: f64) -> f64 {
    if r < 1e-10 {
        return 1.0;
    }
    let sqrt3_r = 3.0_f64.sqrt() * r;
    (1.0 + sqrt3_r) * (-sqrt3_r).exp()
}

pub fn rbf(r: f64) -> f64 {
    (-0.5 * r * r).exp()
}

pub fn rational_quadratic(r: f64, alpha: f64) -> f64 {
    (1.0 + r * r / (2.0 * alpha)).powf(-alpha)
}

pub fn apply_continuous_kernel(kind: KernelKind, r: f64, alpha: f64) -> f64 {
    match kind {
        KernelKind::Matern52 => matern52(r),
        KernelKind::Matern32 => matern32(r),
        KernelKind::RBF => rbf(r),
        KernelKind::RationalQuadratic => rational_quadratic(r, alpha),
    }
}

pub fn hamming_match(a: f64, b: f64) -> f64 {
    if (a - b).abs() < 1e-10 { 1.0 } else { 0.0 }
}

pub fn categorical_factor(vals_a: &[f64], vals_b: &[f64], categorical_n_levels: &[usize]) -> f64 {
    let mut k_cat = 1.0;
    for ((&n_levels, &va), &vb) in categorical_n_levels
        .iter()
        .zip(vals_a.iter())
        .zip(vals_b.iter())
    {
        if n_levels > 1 {
            let match_val = hamming_match(va, vb);
            let cat_var = 1.0 / n_levels as f64;
            let cat_weight = 2.0 * cat_var * (1.0 - cat_var);
            let k_cat_val = if cat_weight > 1e-10 {
                (match_val - cat_var) / cat_var
            } else {
                match_val
            };
            k_cat *= 1.0 + cat_weight * (k_cat_val - 1.0);
        }
    }
    k_cat
}

pub fn kernel_matrix(
    x1: &Array2<f64>,
    x2: &Array2<f64>,
    params: &KernelParams,
    cont_indices: &[usize],
) -> Array2<f64> {
    let n1 = x1.nrows();
    let n2 = x2.nrows();
    let mut k = Array2::<f64>::zeros((n1, n2));

    for i in 0..n1 {
        for j in 0..n2 {
            k[[i, j]] = kernel_pair(x1.row(i), x2.row(j), params, cont_indices);
        }
    }
    k
}

pub fn kernel_pair(
    a: ndarray::ArrayView1<f64>,
    b: ndarray::ArrayView1<f64>,
    params: &KernelParams,
    cont_indices: &[usize],
) -> f64 {
    let mut r2_cont = 0.0;
    for (dim, &ci) in cont_indices.iter().enumerate() {
        let diff = (a[ci] - b[ci]) / params.length_scales[dim];
        r2_cont += diff * diff;
    }
    let r_cont = r2_cont.sqrt();
    let k_cont = params.signal_variance
        * apply_continuous_kernel(params.kernel_kind, r_cont, params.rq_alpha);

    let cat_vals_a: Vec<f64> = params.categorical_indices.iter().map(|&ci| a[ci]).collect();
    let cat_vals_b: Vec<f64> = params.categorical_indices.iter().map(|&ci| b[ci]).collect();
    let k_cat = categorical_factor(&cat_vals_a, &cat_vals_b, &params.categorical_n_levels);

    k_cont * k_cat
}

pub fn kernel_matrix_with_noise(k: &Array2<f64>, noise: f64) -> Array2<f64> {
    let mut kn = k.clone();
    let n = k.nrows();
    for i in 0..n {
        kn[[i, i]] += noise;
    }
    kn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matern52_at_zero() {
        assert!((matern52(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn matern52_decay() {
        let k1 = matern52(0.1);
        let k2 = matern52(1.0);
        let k3 = matern52(10.0);
        assert!(k1 > k2);
        assert!(k2 > k3);
        assert!(k3 > 0.0);
    }

    #[test]
    fn matern32_at_zero() {
        assert!((matern32(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn matern32_decay() {
        let k1 = matern32(0.1);
        let k2 = matern32(1.0);
        let k3 = matern32(10.0);
        assert!(k1 > k2);
        assert!(k2 > k3);
        assert!(k3 > 0.0);
    }

    #[test]
    fn rbf_at_zero() {
        assert!((rbf(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn rbf_decay() {
        let k1 = rbf(0.1);
        let k2 = rbf(1.0);
        let k3 = rbf(10.0);
        assert!(k1 > k2);
        assert!(k2 > k3);
        assert!(k3 > 0.0);
    }

    #[test]
    fn rq_at_zero() {
        assert!((rational_quadratic(0.0, 1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn rq_decay() {
        let k1 = rational_quadratic(0.1, 1.0);
        let k2 = rational_quadratic(1.0, 1.0);
        let k3 = rational_quadratic(10.0, 1.0);
        assert!(k1 > k2);
        assert!(k2 > k3);
        assert!(k3 > 0.0);
    }

    #[test]
    fn rq_alpha_large_approaches_rbf() {
        let alpha = 1000.0;
        let rq_val = rational_quadratic(1.0, alpha);
        let rbf_val = rbf(1.0);
        assert!((rq_val - rbf_val).abs() < 0.01);
    }

    #[test]
    fn kernel_kind_parse() {
        assert_eq!(KernelKind::parse("matern52").unwrap(), KernelKind::Matern52);
        assert_eq!(
            KernelKind::parse("MATERN_52").unwrap(),
            KernelKind::Matern52
        );
        assert_eq!(KernelKind::parse("matern32").unwrap(), KernelKind::Matern32);
        assert_eq!(KernelKind::parse("rbf").unwrap(), KernelKind::RBF);
        assert_eq!(
            KernelKind::parse("squared_exponential").unwrap(),
            KernelKind::RBF
        );
        assert_eq!(
            KernelKind::parse("rq").unwrap(),
            KernelKind::RationalQuadratic
        );
        assert_eq!(
            KernelKind::parse("rational_quadratic").unwrap(),
            KernelKind::RationalQuadratic
        );
        assert!(KernelKind::parse("unknown").is_err());
    }

    #[test]
    fn hamming_same() {
        assert!((hamming_match(3.0, 3.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn hamming_different() {
        assert!((hamming_match(3.0, 5.0) - 0.0).abs() < 1e-10);
    }
}
