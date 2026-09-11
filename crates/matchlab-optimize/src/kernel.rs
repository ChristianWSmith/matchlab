use ndarray::{Array1, Array2};

#[derive(Debug, Clone)]
pub struct KernelParams {
    pub length_scales: Array1<f64>,
    pub signal_variance: f64,
    pub noise_variance: f64,
    pub categorical_indices: Vec<usize>,
    pub categorical_n_levels: Vec<usize>,
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

pub fn matern52_derivative(r: f64) -> f64 {
    if r < 1e-10 {
        return 0.0;
    }
    let sqrt5 = 5.0_f64.sqrt();
    let exp_term = (-sqrt5 * r).exp();
    let _poly = sqrt5 * r + 5.0 * r * r / 3.0;
    -sqrt5 * exp_term * (1.0 + sqrt5 * r + 5.0 * r * r / 3.0) / r
        + exp_term * (sqrt5 + 10.0 * r / 3.0)
}

pub fn hamming_match(a: f64, b: f64) -> f64 {
    if (a - b).abs() < 1e-10 { 1.0 } else { 0.0 }
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
    let k_cont = params.signal_variance * matern52(r_cont);

    let mut k_cat = 1.0;
    for (&cat_idx, &n_levels) in params
        .categorical_indices
        .iter()
        .zip(params.categorical_n_levels.iter())
    {
        if n_levels > 1 {
            let match_val = hamming_match(a[cat_idx], b[cat_idx]);
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

pub fn kernel_cross(
    x_new: &Array2<f64>,
    x_train: &Array2<f64>,
    params: &KernelParams,
    cont_indices: &[usize],
) -> Array1<f64> {
    let n_new = x_new.nrows();
    let n_train = x_train.nrows();
    let mut k = Array1::<f64>::zeros(n_new);

    for i in 0..n_new {
        let mut sum = 0.0;
        for j in 0..n_train {
            sum += kernel_pair(x_new.row(i), x_train.row(j), params, cont_indices);
        }
        k[i] = sum;
    }
    k
}

pub fn kernel_self(x: &Array2<f64>, params: &KernelParams, cont_indices: &[usize]) -> Array1<f64> {
    let n = x.nrows();
    let mut k = Array1::<f64>::zeros(n);
    for i in 0..n {
        k[i] = kernel_pair(x.row(i), x.row(i), params, cont_indices);
    }
    k
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
    fn hamming_same() {
        assert!((hamming_match(3.0, 3.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn hamming_different() {
        assert!((hamming_match(3.0, 5.0) - 0.0).abs() < 1e-10);
    }
}
