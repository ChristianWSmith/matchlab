use crate::kernel::{KernelKind, KernelParams, kernel_matrix, kernel_matrix_with_noise};
use ndarray::{Array1, Array2, Array3};
use rand::Rng;
use rand::SeedableRng;

#[derive(Debug, Clone)]
pub struct GpConfig {
    pub phase1_restarts: u64,
    pub phase1_inner_iters: u64,
    pub phase1_perturbation: f64,
    pub phase2_restarts: u64,
    pub phase2_inner_iters: u64,
    pub phase2_perturbation: f64,
    pub threads: Option<usize>,
}

impl Default for GpConfig {
    fn default() -> Self {
        Self {
            phase1_restarts: 50,
            phase1_inner_iters: 10,
            phase1_perturbation: 0.5,
            phase2_restarts: 200,
            phase2_inner_iters: 40,
            phase2_perturbation: 0.3,
            threads: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GaussianProcess {
    pub params: KernelParams,
    pub x_train: Array2<f64>,
    pub y_train: Array1<f64>,
    k_inv: Array2<f64>,
    alpha: Array1<f64>,
    pub cont_indices: Vec<usize>,
}

impl GaussianProcess {
    pub fn fit(
        x: &Array2<f64>,
        y: &Array1<f64>,
        params: &KernelParams,
        cont_indices: &[usize],
    ) -> Result<Self, String> {
        let k = kernel_matrix(x, x, params, cont_indices);
        let k_noisy = kernel_matrix_with_noise(&k, params.noise_variance);
        let k_inv = invert_matrix(&k_noisy)?;
        let alpha = k_inv.dot(y);

        Ok(Self {
            params: params.clone(),
            x_train: x.clone(),
            y_train: y.clone(),
            k_inv,
            alpha,
            cont_indices: cont_indices.to_vec(),
        })
    }

    pub fn predict(&self, x_new: &Array2<f64>) -> (Array1<f64>, Array1<f64>) {
        let n_new = x_new.nrows();
        let n_train = self.x_train.nrows();
        let mut mean = Array1::<f64>::zeros(n_new);
        let mut variance = Array1::<f64>::zeros(n_new);

        for i in 0..n_new {
            let mut k_star_i = Array1::<f64>::zeros(n_train);
            for j in 0..n_train {
                k_star_i[j] = crate::kernel::kernel_pair(
                    x_new.row(i),
                    self.x_train.row(j),
                    &self.params,
                    &self.cont_indices,
                );
            }
            mean[i] = k_star_i.dot(&self.alpha);

            let k_ii = crate::kernel::kernel_pair(
                x_new.row(i),
                x_new.row(i),
                &self.params,
                &self.cont_indices,
            );
            let v = k_star_i.dot(&self.k_inv);
            variance[i] = k_ii - v.dot(&k_star_i);
            if variance[i] < 1e-10 {
                variance[i] = 1e-10;
            }
        }

        (mean, variance.mapv(|v| v.sqrt()))
    }

    pub fn optimize_hyperparameters(
        x: &Array2<f64>,
        y: &Array1<f64>,
        cont_indices: &[usize],
        cat_indices: &[usize],
        cat_n_levels: &[usize],
        kernel_kind: KernelKind,
        seed: u64,
    ) -> Result<KernelParams, String> {
        Self::optimize_hyperparameters_with_config(
            x,
            y,
            cont_indices,
            cat_indices,
            cat_n_levels,
            kernel_kind,
            seed,
            &GpConfig::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn optimize_hyperparameters_with_config(
        x: &Array2<f64>,
        y: &Array1<f64>,
        cont_indices: &[usize],
        cat_indices: &[usize],
        cat_n_levels: &[usize],
        kernel_kind: KernelKind,
        seed: u64,
        gp_config: &GpConfig,
    ) -> Result<KernelParams, String> {
        use rayon::prelude::*;

        let n_cont = cont_indices.len();
        let base_params = KernelParams::new(n_cont, cat_indices.to_vec(), cat_n_levels.to_vec());

        let diffs = pairwise_raw_diffs(x, cont_indices);

        let phase1_pert = gp_config.phase1_perturbation;
        let phase1_inner = gp_config.phase1_inner_iters;

        let run_phase1 = |thread_seed: u64| -> (KernelParams, f64) {
            let mut rng = rand::rngs::SmallRng::seed_from_u64(thread_seed);
            let mut params = base_params.clone();
            params.kernel_kind = kernel_kind;

            let mut best_local_params = params.clone();
            let mut best_local_mll = f64::NEG_INFINITY;

            for _ in 0..phase1_inner {
                let mut p = params.clone();
                let log_ls: Vec<f64> = p
                    .length_scales
                    .iter()
                    .map(|&v| v.ln() + rng.gen_range(-phase1_pert..phase1_pert))
                    .collect();
                for (i, &ls) in log_ls.iter().enumerate() {
                    p.length_scales[i] = ls.exp().max(0.01);
                }
                let log_sv = p.signal_variance.ln() + rng.gen_range(-phase1_pert..phase1_pert);
                p.signal_variance = log_sv.exp().max(0.01);
                let log_nv = p.noise_variance.ln() + rng.gen_range(-phase1_pert..phase1_pert);
                p.noise_variance = log_nv.exp().max(1e-6);

                if let Ok(gp) = GaussianProcess::fit(x, y, &p, cont_indices) {
                    let mll = gp.marginal_log_likelihood();
                    if mll.is_finite() && mll > best_local_mll {
                        best_local_mll = mll;
                        best_local_params = p;
                    }
                }
            }
            (best_local_params, best_local_mll)
        };

        // Phase 1: parallel random restarts for coarse exploration.
        // Seed layout: phase 1 uses seed + i * 1000, phase 2 uses seed + i * 2000 + 50_000.
        // Safe as long as phase1_restarts < 50_000 / 1000 = 50 (well above typical usage).
        let phase1_n = gp_config.phase1_restarts;
        let phase1_results: Vec<(KernelParams, f64)> = if let Some(n_threads) = gp_config.threads {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(n_threads)
                .build()
                .map_err(|e| format!("build thread pool: {e}"))?;
            pool.install(|| {
                (0..phase1_n)
                    .into_par_iter()
                    .map(|i| run_phase1(seed.wrapping_add(i * 1000)))
                    .collect()
            })
        } else {
            (0..phase1_n)
                .into_par_iter()
                .map(|i| run_phase1(seed.wrapping_add(i * 1000)))
                .collect()
        };

        let mut best_params = base_params.clone();
        best_params.kernel_kind = kernel_kind;
        let mut best_mll = f64::NEG_INFINITY;
        for (params, mll) in &phase1_results {
            if *mll > best_mll {
                best_mll = *mll;
                best_params = params.clone();
            }
        }

        // Phase 2: refined search around best_params from phase 1.
        // All restarts share the same starting point — inner iterations with
        // independent random perturbations explore the local neighborhood.
        let phase2_pert = gp_config.phase2_perturbation;
        let phase2_inner = gp_config.phase2_inner_iters;

        let run_phase2 = |thread_seed: u64, start_params: &KernelParams| -> (KernelParams, f64) {
            let mut rng = rand::rngs::SmallRng::seed_from_u64(thread_seed);
            let params = start_params.clone();
            let mut best_local_params = params.clone();
            let mut best_local_mll = f64::NEG_INFINITY;

            for _ in 0..phase2_inner {
                let mut p = params.clone();
                let log_ls: Vec<f64> = p
                    .length_scales
                    .iter()
                    .map(|&v| v.ln() + rng.gen_range(-phase2_pert..phase2_pert))
                    .collect();
                for (i, &ls) in log_ls.iter().enumerate() {
                    p.length_scales[i] = ls.exp().max(0.01);
                }
                let log_sv = p.signal_variance.ln() + rng.gen_range(-phase2_pert..phase2_pert);
                p.signal_variance = log_sv.exp().max(0.01);
                let log_nv = p.noise_variance.ln() + rng.gen_range(-phase2_pert..phase2_pert);
                p.noise_variance = log_nv.exp().max(1e-6);

                let k = kernel_matrix_fast(&diffs, x, &p, cont_indices);
                let k_noisy = kernel_matrix_with_noise(&k, p.noise_variance);
                if let Ok(l) = cholesky_lower(&k_noisy) {
                    let alpha = solve_cholesky(&l, y);
                    let log_det = 2.0
                        * l.diag()
                            .mapv(|v| v.max(1e-10))
                            .fold(0.0, |acc, v| acc + v.ln());
                    let quad = y.dot(&alpha);
                    let mll = -0.5
                        * (quad + log_det + y.len() as f64 * (2.0 * std::f64::consts::PI).ln());
                    if mll > best_local_mll {
                        best_local_mll = mll;
                        best_local_params = p;
                    }
                }
            }
            (best_local_params, best_local_mll)
        };

        let phase2_n = gp_config.phase2_restarts;
        let phase2_results: Vec<(KernelParams, f64)> = if let Some(n_threads) = gp_config.threads {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(n_threads)
                .build()
                .map_err(|e| format!("build thread pool: {e}"))?;
            pool.install(|| {
                (0..phase2_n)
                    .into_par_iter()
                    .map(|i| run_phase2(seed.wrapping_add(i * 2000 + 50_000), &best_params))
                    .collect()
            })
        } else {
            (0..phase2_n)
                .into_par_iter()
                .map(|i| run_phase2(seed.wrapping_add(i * 2000 + 50_000), &best_params))
                .collect()
        };

        for (params, mll) in &phase2_results {
            if *mll > best_mll {
                best_mll = *mll;
                best_params = params.clone();
            }
        }

        if best_mll == f64::NEG_INFINITY {
            return Err(
                "GP hyperparameter optimization failed: all Cholesky decompositions failed".into(),
            );
        }

        Ok(best_params)
    }

    fn marginal_log_likelihood(&self) -> f64 {
        let n = self.y_train.len() as f64;
        let k = kernel_matrix(
            &self.x_train,
            &self.x_train,
            &self.params,
            &self.cont_indices,
        );
        let k_noisy = kernel_matrix_with_noise(&k, self.params.noise_variance);

        if let Ok(l) = cholesky_lower(&k_noisy) {
            let alpha = solve_cholesky(&l, &self.y_train);
            let log_det = 2.0
                * l.diag()
                    .mapv(|v| v.max(1e-10))
                    .fold(0.0, |acc, v| acc + v.ln());
            let quad = self.y_train.dot(&alpha);
            -0.5 * (quad + log_det + n * (2.0 * std::f64::consts::PI).ln())
        } else {
            f64::NEG_INFINITY
        }
    }
}

fn pairwise_raw_diffs(x: &Array2<f64>, cont_indices: &[usize]) -> Array3<f64> {
    let n = x.nrows();
    let d = cont_indices.len();
    let mut diffs = Array3::<f64>::zeros((n, n, d));
    for i in 0..n {
        for j in 0..n {
            for (dim, &ci) in cont_indices.iter().enumerate() {
                diffs[[i, j, dim]] = x[[i, ci]] - x[[j, ci]];
            }
        }
    }
    diffs
}

fn kernel_matrix_fast(
    diffs: &Array3<f64>,
    x: &Array2<f64>,
    params: &KernelParams,
    cont_indices: &[usize],
) -> Array2<f64> {
    let n = x.nrows();
    let mut k = Array2::<f64>::zeros((n, n));

    for i in 0..n {
        for j in 0..=i {
            let mut r2_cont = 0.0;
            for dim in 0..cont_indices.len() {
                let diff = diffs[[i, j, dim]] / params.length_scales[dim];
                r2_cont += diff * diff;
            }
            let r_cont = r2_cont.sqrt();
            let k_cont = params.signal_variance
                * crate::kernel::apply_continuous_kernel(
                    params.kernel_kind,
                    r_cont,
                    params.rq_alpha,
                );

            let cat_vals_a: Vec<f64> = params
                .categorical_indices
                .iter()
                .map(|&ci| x[[i, ci]])
                .collect();
            let cat_vals_b: Vec<f64> = params
                .categorical_indices
                .iter()
                .map(|&ci| x[[j, ci]])
                .collect();
            let k_cat = crate::kernel::categorical_factor(
                &cat_vals_a,
                &cat_vals_b,
                &params.categorical_n_levels,
            );

            let val = k_cont * k_cat;
            k[[i, j]] = val;
            if i != j {
                k[[j, i]] = val;
            }
        }
    }
    k
}

fn invert_matrix(a: &Array2<f64>) -> Result<Array2<f64>, String> {
    let n = a.nrows();
    let l = cholesky_lower(a).map_err(|e| format!("Cholesky failed: {e}"))?;
    let mut inv = Array2::<f64>::eye(n);
    for i in 0..n {
        let e_i = inv.column(i).to_owned();
        let y = solve_cholesky(&l, &e_i);
        let x = solve_cholesky_transpose(&l, &y);
        inv.column_mut(i).assign(&x);
    }
    Ok(inv)
}

fn cholesky_lower(a: &Array2<f64>) -> Result<Array2<f64>, String> {
    let n = a.nrows();
    let mut l = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[[i, k]] * l[[j, k]];
            }
            if i == j {
                let diag = a[[i, i]] - sum;
                if diag <= 1e-12 {
                    return Err(format!("non-positive diagonal at {i}: {diag}"));
                }
                l[[i, j]] = diag.sqrt();
            } else {
                l[[i, j]] = (a[[i, j]] - sum) / l[[j, j]];
            }
        }
    }
    Ok(l)
}

fn solve_cholesky(l: &Array2<f64>, b: &Array1<f64>) -> Array1<f64> {
    let n = l.nrows();
    let mut y = Array1::<f64>::zeros(n);
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..i {
            sum += l[[i, j]] * y[j];
        }
        y[i] = (b[i] - sum) / l[[i, i]];
    }
    y
}

fn solve_cholesky_transpose(l: &Array2<f64>, y: &Array1<f64>) -> Array1<f64> {
    let n = l.nrows();
    let mut x = Array1::<f64>::zeros(n);
    for i in (0..n).rev() {
        let mut sum = 0.0;
        for j in (i + 1)..n {
            sum += l[[j, i]] * x[j];
        }
        x[i] = (y[i] - sum) / l[[i, i]];
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gp_fit_and_predict() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let params = KernelParams::new(1, vec![], vec![]);
        let gp = GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let (mean, std) = gp.predict(&Array2::from_shape_vec((1, 1), vec![0.5]).unwrap());
        assert!(mean[0].abs() < 2.0);
        assert!(std[0] > 0.0);
    }

    #[test]
    fn gp_fit_rbf() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let mut params = KernelParams::new(1, vec![], vec![]);
        params.kernel_kind = KernelKind::RBF;
        let gp = GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let (mean, std) = gp.predict(&Array2::from_shape_vec((1, 1), vec![0.5]).unwrap());
        assert!(mean[0].abs() < 2.0);
        assert!(std[0] > 0.0);
    }

    #[test]
    fn gp_fit_matern32() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let mut params = KernelParams::new(1, vec![], vec![]);
        params.kernel_kind = KernelKind::Matern32;
        let gp = GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let (mean, std) = gp.predict(&Array2::from_shape_vec((1, 1), vec![0.5]).unwrap());
        assert!(mean[0].abs() < 2.0);
        assert!(std[0] > 0.0);
    }

    #[test]
    fn gp_fit_single_point() {
        let x = Array2::from_shape_vec((1, 1), vec![0.5]).unwrap();
        let y = Array1::from_vec(vec![1.0]);
        let params = KernelParams::new(1, vec![], vec![]);
        let gp = GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let (mean, std) = gp.predict(&Array2::from_shape_vec((1, 1), vec![0.5]).unwrap());
        assert!(mean[0].is_finite());
        assert!(std[0] >= 0.0);
    }

    #[test]
    fn cholesky_known_matrix() {
        let a = Array2::from_shape_vec((2, 2), vec![4.0, 2.0, 2.0, 3.0]).unwrap();
        let l = super::cholesky_lower(&a).unwrap();
        let reconstructed = l.dot(&l.t());
        for i in 0..2 {
            for j in 0..2 {
                assert!(
                    (reconstructed[[i, j]] - a[[i, j]]).abs() < 1e-10,
                    "Cholesky mismatch at [{i},{j}]"
                );
            }
        }
    }

    #[test]
    fn optimize_hyperparameters_returns_ok() {
        let x = Array2::from_shape_vec((5, 1), vec![0.0, 0.25, 0.5, 0.75, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 0.5, 1.0, 0.5, 0.0]);
        let result = GaussianProcess::optimize_hyperparameters(
            &x,
            &y,
            &[0],
            &[],
            &[],
            KernelKind::Matern52,
            42,
        );
        assert!(result.is_ok(), "optimize_hyperparameters should succeed");
        let params = result.unwrap();
        assert!(params.signal_variance > 0.0);
        assert!(params.noise_variance > 0.0);
    }

    #[test]
    fn gp_fit_rq() {
        let x = Array2::from_shape_vec((3, 1), vec![0.0, 0.5, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        let mut params = KernelParams::new(1, vec![], vec![]);
        params.kernel_kind = KernelKind::RationalQuadratic;
        let gp = GaussianProcess::fit(&x, &y, &params, &[0]).unwrap();
        let (mean, std) = gp.predict(&Array2::from_shape_vec((1, 1), vec![0.5]).unwrap());
        assert!(mean[0].abs() < 2.0);
        assert!(std[0] > 0.0);
    }

    #[test]
    fn optimize_hyperparameters_identical_points() {
        let x = Array2::from_shape_vec((2, 1), vec![1.0, 1.0]).unwrap();
        let y = Array1::from_vec(vec![1.0, 1.0]);
        let result =
            GaussianProcess::optimize_hyperparameters(&x, &y, &[0], &[], &[], KernelKind::RBF, 42);
        assert!(result.is_ok());
        let params = result.unwrap();
        assert!(params.noise_variance > 0.0);
    }

    #[test]
    fn optimize_hyperparameters_with_custom_config() {
        let x = Array2::from_shape_vec((5, 1), vec![0.0, 0.25, 0.5, 0.75, 1.0]).unwrap();
        let y = Array1::from_vec(vec![0.0, 0.5, 1.0, 0.5, 0.0]);
        let gp_cfg = GpConfig {
            phase1_restarts: 5,
            phase1_inner_iters: 3,
            phase1_perturbation: 0.3,
            phase2_restarts: 10,
            phase2_inner_iters: 5,
            phase2_perturbation: 0.2,
            threads: Some(2),
        };
        let result = GaussianProcess::optimize_hyperparameters_with_config(
            &x,
            &y,
            &[0],
            &[],
            &[],
            KernelKind::Matern52,
            42,
            &gp_cfg,
        );
        assert!(
            result.is_ok(),
            "optimize_hyperparameters_with_config should succeed"
        );
        let params = result.unwrap();
        assert!(params.signal_variance > 0.0);
        assert!(params.noise_variance > 0.0);
    }

    #[test]
    fn gp_config_default_values() {
        let cfg = GpConfig::default();
        assert_eq!(cfg.phase1_restarts, 50);
        assert_eq!(cfg.phase1_inner_iters, 10);
        assert_eq!(cfg.phase1_perturbation, 0.5);
        assert_eq!(cfg.phase2_restarts, 200);
        assert_eq!(cfg.phase2_inner_iters, 40);
        assert_eq!(cfg.phase2_perturbation, 0.3);
        assert!(cfg.threads.is_none());
    }
}
