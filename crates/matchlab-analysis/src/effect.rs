//! Effect-size and uncertainty estimation across replicates (spec §14.7,
//!).
//!
//! This is the layer that turns a replicate set into distributions,
//! confidence intervals, and effect sizes — "Δ = −14.2, 95% CI [−16.1, −12.4],
//! N = 500 paired replications" instead of "Elo = 158 vs Glicko = 144".
//!
//! Everything is hand-rolled and deterministic: the bootstrap uses
//! `matchlab_core::SimRng`, seeded via `matchlab_experiments::seed::derive`, so
//! any `(samples, conf, seed)` triple yields the same interval forever.
use crate::hierarchy::{MetricObservation, ReplicationScalar, replication_scalars};
use matchlab_core::rng::SimRng;
use matchlab_experiments::ArmResult;
use matchlab_experiments::seed::derive;
use matchlab_metrics::MetricResult;
use tracing;
/// Resample count for the percentile bootstrap.
const DEFAULT_N_BOOT: usize = 10_000;
/// The method used to compute a confidence interval, recorded for provenance
///  and reports .
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiMethod {
    StudentT,
    WelchT,
    Wilson,
    Bootstrap,
    PairedBootstrap,
}
/// A confidence interval with its method recorded for provenance.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceInterval {
    pub lower: f64,
    pub upper: f64,
    pub conf: f64,
    pub method: CiMethod,
}
impl ConfidenceInterval {
    pub fn new(lower: f64, upper: f64, conf: f64, method: CiMethod) -> Self {
        Self {
            lower,
            upper,
            conf,
            method,
        }
    }
}
/// Which `MetricResult` shapes are scalarizable into the aggregation unit
/// (§14.7).
pub fn extract_scalar(result: &MetricResult) -> Option<f64> {
    match result {
        MetricResult::Scalar(v) => Some(*v),
        MetricResult::Summary { mean, .. } => Some(*mean),
        MetricResult::TimeSeries { bucket_means } => {
            if bucket_means.is_empty() {
                Some(0.0)
            } else {
                Some(bucket_means.iter().sum::<f64>() / bucket_means.len() as f64)
            }
        }
        MetricResult::Distribution(_) => None,
        MetricResult::Histogram { .. } => None,
    }
}
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
/// Sample standard deviation (`n − 1` denominator), the t-stat companion.
fn sample_stddev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    (values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64).sqrt()
}
/// Inverse standard-normal CDF (Acklam's rational approximation). Deterministic
/// and dependency-free; used for the Wilson critical value and as the large-df
/// limit of the t correction. The coefficients carry canonical reference
/// precision.
#[allow(clippy::excessive_precision)]
pub(crate) fn norm_quantile(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    const LOW: f64 = 0.02425;
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p < LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p > 1.0 - LOW {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}
/// Lanczos log-gamma (g = 7, n = 9), the t-CDF building block.
fn ln_gamma(xx: f64) -> f64 {
    const G: f64 = 7.0;
    const COF: [f64; 9] = [
        0.9999999999998099,
        676.5203681218851,
        -1259.1392167224028,
        771.3234287776531,
        -176.6150291621406,
        12.507343278686905,
        -0.13857109526572012,
        9.984369578019572e-6,
        1.5056327351493116e-7,
    ];
    if xx < 0.5 {
        std::f64::consts::PI.ln() - (std::f64::consts::PI * xx).sin().ln() - ln_gamma(1.0 - xx)
    } else {
        let x = xx - 1.0;
        let mut a = COF[0];
        let t = x + G + 0.5;
        for (i, c) in COF.iter().enumerate().skip(1) {
            a += c / (x + i as f64);
        }
        0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}
/// Continued-fraction evaluation of the incomplete beta (Numerical Recipes
/// `betacf`, 200 steps, `FPMIN = 1e-300`).
fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const MAXIT: usize = 200;
    const EPS: f64 = 3.0e-12;
    const FPMIN: f64 = 1.0e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    d = if d.abs() < FPMIN { FPMIN } else { 1.0 / d };
    let mut h = d;
    for m in 1..=MAXIT {
        let m2 = 2 * m;
        let aa = m as f64 * (b - m as f64) * x / ((qam + m2 as f64) * (a + m2 as f64));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa2 = -(a + m as f64) * (qab + m as f64) * x / ((a + m2 as f64) * (qap + m2 as f64));
        d = 1.0 + aa2 * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa2 / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    h
}
/// Regularized incomplete beta `I_x(a, b)`.
fn betai(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(a, b, x) / a
    } else {
        1.0 - bt * betacf(b, a, 1.0 - x) / b
    }
}
/// Student-t CDF `F(t) = 1 − ½·I_{ν/(ν+t²)}(ν/2, 1/2)`.
pub(crate) fn t_cdf(df: f64, t: f64) -> f64 {
    if t >= 0.0 {
        let x = df / (df + t * t);
        1.0 - 0.5 * betai(df / 2.0, 0.5, x)
    } else {
        1.0 - t_cdf(df, -t)
    }
}
/// Upper-tail t quantile (e.g. `t_quantile(9, 0.975) ≈ 2.262`), computed by
/// bisection on the exact t CDF — deterministic, no approximation tables.
fn t_quantile(df: f64, p: f64) -> f64 {
    let target = if p >= 0.5 { p } else { 1.0 - p };
    let mut lo = 0.0;
    let mut hi = 32.0;
    while t_cdf(df, hi) < target {
        hi *= 2.0;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if t_cdf(df, mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let x = 0.5 * (lo + hi);
    if p >= 0.5 { x } else { -x }
}
/// Nearest-rank percentile over a sorted sample (`p` in [0, 1]).
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    let idx = (p.clamp(0.0, 1.0) * (sorted.len() - 1) as f64) as usize;
    sorted[idx]
}
/// Percentile-method bootstrap CI over a sample's mean
/// (`n_boot = 10_000`). Deterministic for a given `seed`.
pub fn bootstrap_ci(samples: &[f64], conf: f64, seed: u64) -> ConfidenceInterval {
    if samples.is_empty() {
        return ConfidenceInterval::new(0.0, 0.0, conf, CiMethod::Bootstrap);
    }
    let n = samples.len();
    let mut rng = SimRng::from_seed(seed);
    let mut boot = Vec::with_capacity(DEFAULT_N_BOOT);
    for _ in 0..DEFAULT_N_BOOT {
        let mut sum = 0.0;
        for _ in 0..n {
            sum += samples[(rng.gen_u64() % n as u64) as usize];
        }
        boot.push(sum / n as f64);
    }
    boot.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let alpha = (1.0 - conf) / 2.0;
    ConfidenceInterval::new(
        percentile_sorted(&boot, alpha),
        percentile_sorted(&boot, 1.0 - alpha),
        conf,
        CiMethod::Bootstrap,
    )
}
/// Paired percentile bootstrap over per-replicate differences — **the
/// headline stat for CRN and counterfactual study arms**. The caller computes
/// the paired differences `treatment − control` (same replicate index), which
/// cancels between-replicate noise; the CI is over those `deltas` only.
pub fn paired_bootstrap_ci(deltas: &[f64], conf: f64, seed: u64) -> ConfidenceInterval {
    let ci = bootstrap_ci(deltas, conf, seed);
    ConfidenceInterval::new(ci.lower, ci.upper, ci.conf, CiMethod::PairedBootstrap)
}
/// Unpaired two-sample percentile bootstrap (mean of treatment minus mean of
/// control). Used when arm sizes differ; wider than the paired version when
/// arms are correlated.
fn two_sample_bootstrap_ci(
    control: &[f64],
    treatment: &[f64],
    conf: f64,
    seed: u64,
) -> ConfidenceInterval {
    if control.is_empty() || treatment.is_empty() {
        return ConfidenceInterval::new(0.0, 0.0, conf, CiMethod::Bootstrap);
    }
    let nc = control.len();
    let nt = treatment.len();
    let mut rng = SimRng::from_seed(seed);
    let mut boot = Vec::with_capacity(DEFAULT_N_BOOT);
    for _ in 0..DEFAULT_N_BOOT {
        let mut sum_c = 0.0;
        for _ in 0..nc {
            sum_c += control[(rng.gen_u64() % nc as u64) as usize];
        }
        let mut sum_t = 0.0;
        for _ in 0..nt {
            sum_t += treatment[(rng.gen_u64() % nt as u64) as usize];
        }
        boot.push(sum_t / nt as f64 - sum_c / nc as f64);
    }
    boot.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let alpha = (1.0 - conf) / 2.0;
    ConfidenceInterval::new(
        percentile_sorted(&boot, alpha),
        percentile_sorted(&boot, 1.0 - alpha),
        conf,
        CiMethod::Bootstrap,
    )
}
/// Student-t CI: mean ± `t_{n−1} · s/√n` with the exact t critical value
/// (bisected from the t CDF). The `ci` helper routes here for n ≥ 30, where t
/// and the normal quantile coincide to three decimals.
pub fn student_t_ci(samples: &[f64], conf: f64) -> ConfidenceInterval {
    if samples.is_empty() {
        return ConfidenceInterval::new(0.0, 0.0, conf, CiMethod::StudentT);
    }
    let n = samples.len();
    let m = mean(samples);
    let crit = if n >= 2 {
        t_quantile((n - 1) as f64, (1.0 + conf) / 2.0)
    } else {
        1.0
    };
    let margin = crit * sample_stddev(samples) / (n as f64).sqrt();
    ConfidenceInterval::new(m - margin, m + margin, conf, CiMethod::StudentT)
}
/// Welch–Satterthwaite two-sample t CI for independent samples with unequal
/// variances. The degrees of freedom are computed from the Welch formula rather
/// than the pooled `n₁ + n₂ − 2`.
pub fn welch_ci(control: &[f64], treatment: &[f64], conf: f64) -> ConfidenceInterval {
    if control.is_empty() || treatment.is_empty() {
        return ConfidenceInterval::new(0.0, 0.0, conf, CiMethod::WelchT);
    }
    let n_c = control.len() as f64;
    let n_t = treatment.len() as f64;
    let m_c = mean(control);
    let m_t = mean(treatment);
    let s_c = sample_stddev(control);
    let s_t = sample_stddev(treatment);
    let v_c = s_c * s_c;
    let v_t = s_t * s_t;
    let se = (v_c / n_c + v_t / n_t).sqrt();
    let df = if se > 0.0 {
        let num = (v_c / n_c + v_t / n_t).powi(2);
        let den = (v_c / n_c).powi(2) / (n_c - 1.0) + (v_t / n_t).powi(2) / (n_t - 1.0);
        if den > 0.0 {
            num / den
        } else {
            n_c + n_t - 2.0
        }
    } else {
        n_c + n_t - 2.0
    };
    let crit = if df >= 1.0 {
        t_quantile(df, (1.0 + conf) / 2.0)
    } else {
        12.706
    };
    let margin = crit * se;
    ConfidenceInterval::new(
        m_t - m_c - margin,
        m_t - m_c + margin,
        conf,
        CiMethod::WelchT,
    )
}
/// Wilson score interval for a proportion. Handles edges explicitly:
/// `n == 0` → `(0,0)`; `k == 0` → lo clamped to 0; `k == n` → hi clamped to 1.
pub fn wilson_ci(k: usize, n: usize, conf: f64) -> ConfidenceInterval {
    if n == 0 {
        return ConfidenceInterval::new(0.0, 0.0, conf, CiMethod::Wilson);
    }
    let z = norm_quantile((1.0 + conf) / 2.0);
    let p_hat = k as f64 / n as f64;
    let denom = 1.0 + z * z / n as f64;
    let center = (p_hat + z * z / (2.0 * n as f64)) / denom;
    let margin =
        z * (p_hat * (1.0 - p_hat) / n as f64 + z * z / (4.0 * n as f64 * n as f64)).sqrt() / denom;
    ConfidenceInterval::new(
        (center - margin).max(0.0),
        (center + margin).min(1.0),
        conf,
        CiMethod::Wilson,
    )
}
/// Design-aware CI dispatch (): for paired designs, the paired
/// bootstrap over per-replicate differences; for independent designs, Welch-T
/// for n ≥ 30, bootstrap otherwise.
pub fn ci(
    control: &[f64],
    treatment: &[f64],
    conf: f64,
    seed: u64,
    paired: bool,
) -> ConfidenceInterval {
    tracing::trace!(
        n_control = control.len(),
        n_treatment = treatment.len(),
        paired,
        "computing confidence interval"
    );
    if paired {
        let deltas: Vec<f64> = control.iter().zip(treatment).map(|(c, t)| t - c).collect();
        paired_bootstrap_ci(&deltas, conf, seed)
    } else if control.len() >= 30 && treatment.len() >= 30 {
        welch_ci(control, treatment, conf)
    } else {
        two_sample_bootstrap_ci(control, treatment, conf, seed)
    }
}
/// Effect size between a control and a treatment arm, plus its CI and
/// standardized magnitude (§14.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectSize {
    pub mean_delta: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
    pub cohen_d: f64,
    /// For paired designs: `d_z = mean_delta / sd(deltas)`. `None` when
    /// `paired` is false (pooled `cohen_d` applies).
    pub d_z: Option<f64>,
    pub relative_diff: f64,
    pub percent_improvement: f64,
    pub ci_method: CiMethod,
}
/// Descriptive statistics for a single arm's replication-level scalars.
#[derive(Debug, Clone, Copy)]
pub struct ArmStat {
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
    pub median: f64,
    pub ci: ConfidenceInterval,
}
/// Compute per-arm descriptive stats from replication-level scalars.
pub fn arm_stat(scalars: &[ReplicationScalar], conf: f64, seed: u64) -> Option<ArmStat> {
    if scalars.is_empty() {
        return None;
    }
    let values: Vec<f64> = scalars.iter().map(ReplicationScalar::value).collect();
    let n = values.len();
    let m = mean(&values);
    let sd = sample_stddev(&values);
    let mut sorted = values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if n % 2 == 0 {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };
    let ci = if n >= 30 {
        student_t_ci(&values, conf)
    } else {
        bootstrap_ci(&values, conf, seed)
    };
    Some(ArmStat {
        n,
        mean: m,
        sd,
        median,
        ci,
    })
}
/// Construct an [`EffectSize`] from the replication table for a given
/// condition pair and metric. Returns `None` when either arm has no
/// scalarizable observations.
pub fn effect_size_for(
    control: &[ReplicationScalar],
    treatment: &[ReplicationScalar],
    paired: bool,
    conf: f64,
    seed: u64,
) -> Option<EffectSize> {
    effect_sizes(control, treatment, paired, conf, seed).ok()
}
///
/// The estimator respects the nesting rule: its inputs are
/// [`ReplicationScalar`]s (never bare per-match `f64`s), so calling it requires
/// having first extracted replication-level values through the documented
/// [`per_replication`] steps or [`MetricObservation::replication_scalar`].
///
/// `paired` is the first-class design flag : `true` for a
/// [`DesignType::Paired`] or [`DesignType::Counterfactual`] study, `false` for
/// [`DesignType::Independent`]. A paired design with unequal arm lengths is an
/// error (the design is misspecified), not a silent fallback.
///
/// The CI is **paired** (per-replicate differences) when `paired` is true —
/// the correct choice for CRN/counterfactual study arms — and falls back to an
/// unpaired two-sample bootstrap otherwise. Cohen's `d` uses the n-weighted
/// pooled sample stddev; `relative_diff` is divided by `|mean_c|` (NaN when
/// `mean_c == 0`); `percent_improvement` is `relative_diff × 100` (positive =
/// treatment higher, whatever "higher" means for the metric under study).
pub fn effect_sizes(
    control: &[ReplicationScalar],
    treatment: &[ReplicationScalar],
    paired: bool,
    conf: f64,
    seed: u64,
) -> Result<EffectSize, &'static str> {
    let control_v: Vec<f64> = control.iter().map(ReplicationScalar::value).collect();
    let treatment_v: Vec<f64> = treatment.iter().map(ReplicationScalar::value).collect();
    effect_sizes_f64(&control_v, &treatment_v, paired, conf, seed)
}
fn effect_sizes_f64(
    control: &[f64],
    treatment: &[f64],
    paired: bool,
    conf: f64,
    seed: u64,
) -> Result<EffectSize, &'static str> {
    if control.is_empty() || treatment.is_empty() {
        return Err("empty arms");
    }
    if paired && control.len() != treatment.len() {
        return Err("paired design requires equal arm lengths");
    }
    let mean_c = mean(control);
    let mean_t = mean(treatment);
    let mean_delta = mean_t - mean_c;
    let ci = ci(control, treatment, conf, derive(seed, 0), paired);
    let n_c = control.len() as f64;
    let n_t = treatment.len() as f64;
    let s_c = sample_stddev(control);
    let s_t = sample_stddev(treatment);
    let pooled =
        ((s_c.powi(2) * (n_c - 1.0) + s_t.powi(2) * (n_t - 1.0)) / (n_c + n_t - 2.0)).sqrt();
    let cohen_d = if pooled > 0.0 {
        mean_delta / pooled
    } else {
        0.0
    };
    let d_z = if paired {
        let deltas: Vec<f64> = control.iter().zip(treatment).map(|(c, t)| t - c).collect();
        let sd = sample_stddev(&deltas);
        if sd > 0.0 {
            Some(mean_delta / sd)
        } else {
            None
        }
    } else {
        None
    };
    let relative_diff = mean_delta / mean_c.abs();
    tracing::debug!(
        mean_delta,
        ci_low = ci.lower,
        ci_high = ci.upper,
        cohens_d = cohen_d,
        n_control = control.len(),
        n_treatment = treatment.len(),
        "effect size computed"
    );
    Ok(EffectSize {
        mean_delta,
        ci_lo: ci.lower,
        ci_hi: ci.upper,
        cohen_d,
        d_z,
        relative_diff,
        percent_improvement: relative_diff * 100.0,
        ci_method: ci.method,
    })
}
/// Convenience layer on 's `ArmResult` : extract the metric's
/// per-replicate observations from both arms through [`replication_scalars`]
/// and run `effect_sizes`. Returns `Err` if either arm lacks the metric, if
/// any replicate's value isn't scalarable, if the arms expose different N/A
/// patterns (a strict all-or-nothing contract: partial data cannot be paired),
/// or if a paired design is declared but arm lengths differ.
pub fn compare(
    control: &ArmResult,
    treatment: &ArmResult,
    metric: &str,
    paired: bool,
    conf: f64,
    seed: u64,
) -> Result<EffectSize, &'static str> {
    let control_obs = replication_scalars(control, metric);
    let treatment_obs = replication_scalars(treatment, metric);
    if control_obs.iter().any(|o| !o.is_scalarable())
        || treatment_obs.iter().any(|o| !o.is_scalarable())
    {
        return Err("non-scalarable metric value");
    }
    if control_obs.len() != treatment_obs.len() {
        return Err("unequal observation counts");
    }
    let control_scalars: Vec<ReplicationScalar> = control_obs
        .iter()
        .filter_map(MetricObservation::replication_scalar)
        .collect();
    let treatment_scalars: Vec<ReplicationScalar> = treatment_obs
        .iter()
        .filter_map(MetricObservation::replication_scalar)
        .collect();
    if control_scalars.len() != treatment_scalars.len() {
        return Err("unequal scalar counts after N/A filtering");
    }
    effect_sizes(&control_scalars, &treatment_scalars, paired, conf, seed)
}
/// Two-sided p-value for a paired t-test: `p = 2·(1 − t_cdf(df, |t|))`
/// where `t = mean(deltas) / (sd(deltas) / √n)`.
pub fn paired_t_pvalue(deltas: &[f64]) -> f64 {
    let n = deltas.len();
    if n < 2 {
        return 1.0;
    }
    let m = mean(deltas);
    let s = sample_stddev(deltas);
    if s == 0.0 {
        return if m == 0.0 { 1.0 } else { 0.0 };
    }
    let t_stat = m / (s / (n as f64).sqrt());
    let df = (n - 1) as f64;
    let p_upper = 1.0 - t_cdf(df, t_stat.abs());
    (2.0 * p_upper).min(1.0)
}
/// Two-sided p-value for a Welch two-sample t-test: `p = 2·(1 − t_cdf(df, |t|))`
/// where `t = (mean_t − mean_c) / SE` and `df` is from the Welch–Satterthwaite
/// formula.
pub fn welch_pvalue(control: &[f64], treatment: &[f64]) -> f64 {
    if control.is_empty() || treatment.is_empty() {
        return 1.0;
    }
    let n_c = control.len() as f64;
    let n_t = treatment.len() as f64;
    let m_c = mean(control);
    let m_t = mean(treatment);
    let s_c = sample_stddev(control);
    let s_t = sample_stddev(treatment);
    let v_c = s_c * s_c;
    let v_t = s_t * s_t;
    let se = (v_c / n_c + v_t / n_t).sqrt();
    if se == 0.0 {
        return if (m_t - m_c).abs() < 1e-15 { 1.0 } else { 0.0 };
    }
    let t_stat = (m_t - m_c) / se;
    let df = {
        let num = (v_c / n_c + v_t / n_t).powi(2);
        let den = (v_c / n_c).powi(2) / (n_c - 1.0) + (v_t / n_t).powi(2) / (n_t - 1.0);
        if den > 0.0 {
            num / den
        } else {
            n_c + n_t - 2.0
        }
    };
    let p_upper = 1.0 - t_cdf(df, t_stat.abs());
    (2.0 * p_upper).min(1.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::per_replication;
    use matchlab_core::rng::SimRng;
    use matchlab_metrics::MetricResult;
    fn scalar(v: f64) -> ReplicationScalar {
        per_replication::from_metric(&MetricResult::Scalar(v)).expect("scalarable")
    }
    fn to_scalars(values: &[f64]) -> Vec<ReplicationScalar> {
        values.iter().copied().map(scalar).collect()
    }
    fn normal_sample(mean: f64, stddev: f64, n: usize, seed: u64) -> Vec<f64> {
        let mut rng = SimRng::from_seed(seed);
        (0..n).map(|_| rng.sample_normal(mean, stddev)).collect()
    }
    #[test]
    fn norm_quantile_matches_known_zscores() {
        assert!((norm_quantile(0.5) - 0.0).abs() < 1e-9);
        assert!((norm_quantile(0.975) - 1.959964).abs() < 1e-3);
        assert!((norm_quantile(0.025) + 1.959964).abs() < 1e-3);
        assert!((norm_quantile(0.95) - 1.644854).abs() < 1e-3);
    }
    #[test]
    fn coverage_sanity_on_large_normal_sample() {
        let samples = normal_sample(0.0, 1.0, 200, 7);
        let bci = bootstrap_ci(&samples, 0.95, 42);
        let tci = student_t_ci(&samples, 0.95);
        assert!(
            bci.lower < 0.0 && bci.upper > 0.0,
            "bootstrap CI must straddle 0"
        );
        assert!(tci.lower < 0.0 && tci.upper > 0.0, "t CI must straddle 0");
        assert!(
            bci.upper - bci.lower > 0.05 && bci.upper - bci.lower < 0.6,
            "sane bootstrap width"
        );
    }
    #[test]
    fn small_sample_bootstrap_undercovers_versus_t() {
        let samples = normal_sample(10.0, 2.0, 10, 3);
        let bci = bootstrap_ci(&samples, 0.95, 42);
        let tci = student_t_ci(&samples, 0.95);
        assert!(
            bci.upper - bci.lower < tci.upper - tci.lower,
            "percentile bootstrap is narrower than exact-t at n = 10 ({} < {})",
            bci.upper - bci.lower,
            tci.upper - tci.lower
        );
    }
    #[test]
    fn bootstrap_ci_is_deterministic_for_seed() {
        let samples = normal_sample(5.0, 3.0, 50, 11);
        assert_eq!(
            bootstrap_ci(&samples, 0.95, 99),
            bootstrap_ci(&samples, 0.95, 99)
        );
        assert_ne!(
            bootstrap_ci(&samples, 0.95, 99),
            bootstrap_ci(&samples, 0.95, 100)
        );
    }
    #[test]
    fn identical_arms_have_zero_effect() {
        let control = to_scalars(&normal_sample(0.0, 1.0, 40, 5));
        let treatment = control.clone();
        let es = effect_sizes(&control, &treatment, true, 0.95, 42).expect("effect");
        assert!(es.mean_delta.abs() < 1e-12);
        assert!(
            es.ci_lo <= 0.0 && es.ci_hi >= 0.0,
            "delta CI must contain 0"
        );
        assert!(es.cohen_d.abs() < 1e-12);
    }
    #[test]
    fn constant_shift_size_and_ci() {
        let control = to_scalars(&[1.0, 2.0, 3.0, 4.0]);
        let treatment: Vec<ReplicationScalar> =
            control.iter().map(|s| scalar(s.value() + 5.0)).collect();
        let es = effect_sizes(&control, &treatment, true, 0.95, 42).expect("effect");
        assert!((es.mean_delta - 5.0).abs() < 1e-12);
        assert!(
            es.ci_lo > 4.9 && es.ci_hi < 5.1,
            "constant shift: tight CI about 5"
        );
        assert!(es.ci_lo > 0.0, "CI must exclude 0");
        assert!((es.relative_diff - 2.0).abs() < 1e-9);
        assert!((es.percent_improvement - 200.0).abs() < 1e-9);
    }
    #[test]
    fn cohen_d_matches_hand_computation() {
        let control = to_scalars(&[0.0, 2.0, 0.0, 2.0]);
        let treatment = to_scalars(&[2.0, 4.0, 2.0, 4.0]);
        let es = effect_sizes(&control, &treatment, true, 0.95, 1).expect("effect");
        assert!((es.cohen_d - 3.0f64.sqrt()).abs() < 1e-9);
        assert!(es.d_z.is_none(), "constant deltas produce no d_z");
    }
    /// known-answer: paired d_z = mean_delta / sd(deltas).
    /// control = [10, 12, 11, 13, 10], treatment = [14, 15, 16, 15, 14]
    /// deltas = [4, 3, 5, 2, 4], mean = 3.6, var(deltas) = 1.3
    /// d_z = 3.6 / sqrt(1.3) ≈ 3.158.
    #[test]
    fn d_z_matches_hand_computation_for_paired_design() {
        let control = to_scalars(&[10.0, 12.0, 11.0, 13.0, 10.0]);
        let treatment = to_scalars(&[14.0, 15.0, 16.0, 15.0, 14.0]);
        let es = effect_sizes(&control, &treatment, true, 0.95, 1).expect("effect");
        let expected_dz = 3.6 / (1.3_f64).sqrt();
        let dz = es.d_z.expect("paired design produces d_z");
        assert!(
            (dz - expected_dz).abs() < 1e-9,
            "d_z = {dz}, expected {expected_dz}"
        );
        let es_ind = effect_sizes(&control, &treatment, false, 0.95, 1).expect("effect");
        assert!(es_ind.d_z.is_none(), "independent design has no d_z");
    }
    #[test]
    fn paired_ci_tighter_than_independent_on_correlated_arms() {
        let base = normal_sample(0.0, 1.0, 60, 21);
        let noise = normal_sample(0.0, 0.1, 60, 8);
        let treatment: Vec<f64> = base.iter().zip(&noise).map(|(b, e)| b + e + 5.0).collect();
        let deltas: Vec<f64> = base.iter().zip(&treatment).map(|(b, t)| t - b).collect();
        let pci = paired_bootstrap_ci(&deltas, 0.95, 42);
        let uci = two_sample_bootstrap_ci(&base, &treatment, 0.95, 42);
        assert!(
            pci.upper - pci.lower < uci.upper - uci.lower,
            "paired CI must be tighter than the independent two-sample CI"
        );
        assert!(
            pci.lower > 4.0,
            "paired CI about the +5 shift must exclude 0"
        );
    }
    #[test]
    fn wilson_edges() {
        let ci0 = wilson_ci(0, 0, 0.95);
        assert_eq!(ci0.lower, 0.0, "n = 0 → lower = 0");
        assert_eq!(ci0.upper, 0.0, "n = 0 → upper = 0");
        let ci_k0 = wilson_ci(0, 100, 0.95);
        assert_eq!(ci_k0.lower, 0.0, "k = 0 → lo clamps to 0");
        let ci_kn = wilson_ci(100, 100, 0.95);
        assert_eq!(ci_kn.upper, 1.0, "k = n → hi clamps to 1");
        let ci50 = wilson_ci(50, 100, 0.95);
        assert!(
            ci50.lower <= 0.5 && ci50.upper >= 0.5,
            "middle proportion straddles p̂"
        );
        assert!(ci50.lower > 0.0 && ci50.upper < 1.0);
    }
    #[test]
    fn extract_scalar_mapping() {
        use matchlab_metrics::MetricResult;
        assert_eq!(extract_scalar(&MetricResult::Scalar(3.5)), Some(3.5));
        assert_eq!(
            extract_scalar(&MetricResult::Summary {
                mean: 1.5,
                median: 1.0,
                p75: 2.0,
                p90: 2.5,
                p95: 3.0,
                p99: 3.5,
                stddev: 0.5,
            }),
            Some(1.5)
        );
        assert_eq!(
            extract_scalar(&MetricResult::TimeSeries {
                bucket_means: vec![1.0, 3.0],
            }),
            Some(2.0)
        );
        assert_eq!(
            extract_scalar(&MetricResult::TimeSeries {
                bucket_means: vec![],
            }),
            Some(0.0)
        );
        assert_eq!(extract_scalar(&MetricResult::Distribution(vec![1.0])), None);
        assert_eq!(
            extract_scalar(&MetricResult::Histogram { buckets: vec![] }),
            None
        );
    }
    #[test]
    fn empty_inputs_produce_safe_defaults() {
        let ci = bootstrap_ci(&[], 0.95, 1);
        assert_eq!(ci.lower, 0.0);
        assert_eq!(ci.upper, 0.0);
        let ci = student_t_ci(&[], 0.95);
        assert_eq!(ci.lower, 0.0);
        assert_eq!(ci.upper, 0.0);
        let empty: Vec<ReplicationScalar> = Vec::new();
        assert!(effect_sizes(&empty, &[scalar(1.0)], true, 0.95, 1).is_err());
        assert!(effect_sizes(&[scalar(1.0)], &empty, true, 0.95, 1).is_err());
    }
    /// acceptance: the length-equality hack is gone. A `Paired` design
    /// with unequal replicate counts is an error, not a silent fallback to an
    /// unpaired bootstrap (that fallback would have masked a misspecified
    /// paired study).
    #[test]
    fn paired_design_with_unequal_counts_fails_loudly() {
        let control = to_scalars(&[1.0, 2.0, 3.0]);
        let treatment = to_scalars(&[4.0, 5.0]);
        let err = effect_sizes(&control, &treatment, true, 0.95, 42).expect_err("must fail");
        assert!(
            err.contains("equal arm lengths"),
            "error names the design constraint: {err}"
        );
        assert!(effect_sizes(&control, &treatment, false, 0.95, 42).is_ok());
    }
    /// acceptance: an `Independent` study with equal replicate counts gets
    /// a *wider* CI than the (old, length-inferred) paired interpretation —
    /// proving the old code would have under-covered on correlated arms.
    #[test]
    fn independent_equal_count_ci_is_wider_than_paired() {
        let base: Vec<f64> = (0..20).map(|i| 100.0 + 10.0 * (i % 5) as f64).collect();
        let control = to_scalars(&base);
        let treatment: Vec<ReplicationScalar> = base.iter().map(|v| scalar(v + 2.0)).collect();
        let paired = effect_sizes(&control, &treatment, true, 0.95, 99).expect("paired");
        let independent = effect_sizes(&control, &treatment, false, 0.95, 99).expect("independent");
        assert_eq!(paired.mean_delta, independent.mean_delta);
        assert!(
            (paired.ci_hi - paired.ci_lo) < (independent.ci_hi - independent.ci_lo),
            "independent CI must be wider than paired on correlated arms"
        );
    }
    fn fake_result(name: &str, ra_mean: f64) -> matchlab_experiments::ExperimentResult {
        use matchlab_metrics::MetricResult;
        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "rating_accuracy".to_string(),
            MetricResult::Summary {
                mean: ra_mean,
                median: ra_mean,
                p75: ra_mean,
                p90: ra_mean,
                p95: ra_mean,
                p99: ra_mean,
                stddev: 0.0,
            },
        );
        matchlab_experiments::ExperimentResult {
            experiment_id: format!("{name}-x"),
            name: name.to_string(),
            config_hash: "hash".to_string(),
            git_commit: "abc".to_string(),
            timestamp: "t".to_string(),
            matches_completed: 10,
            matches_formed: 10,
            simulated_time_secs: 100.0,
            metrics,
            utility_score: None,
        }
    }
    fn fake_arm(name: &str, means: &[f64]) -> ArmResult {
        ArmResult {
            name: name.to_string(),
            condition_id: name.to_string(),
            replicates: means
                .iter()
                .enumerate()
                .map(|(i, m)| matchlab_experiments::ReplicateResult {
                    replicate_index: i as u64,
                    seed: i as u64,
                    parent_seed: 0,
                    result: fake_result(name, *m),
                })
                .collect(),
        }
    }
    #[test]
    fn compare_on_arm_results_matches_hand_computation() {
        let control_means: Vec<f64> = (0..10).map(|i| 100.0 + 2.0 * i as f64).collect();
        let treatment_means: Vec<f64> = control_means.iter().map(|m| m + 4.0).collect();
        let control = fake_arm("control", &control_means);
        let treatment = fake_arm("treatment", &treatment_means);
        let es = compare(&control, &treatment, "rating_accuracy", true, 0.95, 7).expect("compare");
        assert!((es.mean_delta - 4.0).abs() < 1e-9);
        assert!((es.ci_lo - 4.0).abs() < 1e-9 && (es.ci_hi - 4.0).abs() < 1e-9);
        assert!((es.relative_diff - 4.0 / 109.0).abs() < 1e-9);
        assert!(es.ci_lo > 0.0, "constant +4 shift excludes 0");
    }
    #[test]
    fn compare_missing_metric_returns_none() {
        let control = fake_arm("ctrl", &[1.0, 2.0]);
        let treatment = fake_arm("trt", &[3.0, 4.0]);
        assert!(compare(&control, &treatment, "queue_time", true, 0.95, 1).is_err());
    }
    /// negative control: a deliberately per-match aggregate (a
    /// `Distribution` of per-match samples) is NOT accepted where replication
    /// scalars are required, without the explicit `per_replication` step. The
    /// live `compare` path records it as N/A and refuses to pair it — proving
    /// per-match data cannot silently masquerade as the replication unit.
    #[test]
    fn compare_rejects_per_match_aggregates_without_explicit_step() {
        use matchlab_metrics::MetricResult;
        let mut per_match = fake_arm("ctrl", &[1.0, 2.0]);
        for repl in &mut per_match.replicates {
            repl.result.metrics.insert(
                "match_quality".to_string(),
                MetricResult::Distribution(vec![0.9, 0.97, 0.96]),
            );
        }
        let treatment = fake_arm("trt", &[3.0, 4.0]);
        assert!(
            compare(&per_match, &treatment, "match_quality", true, 0.95, 1).is_err(),
            "per-match Distribution must not pair as replication scalars"
        );
        let obs = replication_scalars(&per_match, "match_quality");
        assert!(
            obs.iter()
                .all(|o| o.value == crate::hierarchy::MetricValue::NotScalarizable)
        );
        let collapsed: Vec<ReplicationScalar> = obs
            .iter()
            .map(|o| match &o.value {
                crate::hierarchy::MetricValue::NotScalarizable => {
                    let summary = MetricResult::Summary {
                        mean: 0.9,
                        median: 0.9,
                        p75: 0.9,
                        p90: 0.9,
                        p95: 0.9,
                        p99: 0.9,
                        stddev: 0.0,
                    };
                    per_replication::from_metric(&summary).expect("documented named step")
                }
                _ => o.replication_scalar().expect("scalar"),
            })
            .collect();
        assert_eq!(
            collapsed.len(),
            2,
            "explicit collapse yields replication scalars"
        );
    }
    #[test]
    fn welch_ci_matches_hand_worked_example() {
        let control = vec![10.0, 12.0, 11.0, 13.0, 10.0];
        let treatment = vec![14.0, 15.0, 16.0, 15.0, 14.0];
        let ci = welch_ci(&control, &treatment, 0.95);
        let delta = 3.6;
        assert!(
            (ci.lower - (delta - 2.365 * 0.6928)).abs() < 0.01
                || (ci.upper - (delta + 2.365 * 0.6928)).abs() < 0.01,
            "welch CI bounds close to hand computation: [{:.3}, {:.3}]",
            ci.lower,
            ci.upper,
        );
        assert!(ci.lower < delta && ci.upper > delta, "CI straddles delta");
        assert_eq!(ci.method, CiMethod::WelchT);
    }
    #[test]
    fn ci_method_is_recorded_in_effect_size() {
        let control = to_scalars(&normal_sample(100.0, 10.0, 50, 11));
        let treatment: Vec<ReplicationScalar> =
            control.iter().map(|s| scalar(s.value() + 3.0)).collect();
        let es = effect_sizes(&control, &treatment, true, 0.95, 42).expect("paired effect");
        assert_eq!(es.ci_method, CiMethod::PairedBootstrap);
        let es_ind =
            effect_sizes(&control, &treatment, false, 0.95, 42).expect("independent effect");
        assert_eq!(
            es_ind.ci_method,
            CiMethod::WelchT,
            "n=50 → Welch for independent"
        );
    }
    #[test]
    fn ci_for_dispatch_returns_correct_method() {
        let control = normal_sample(0.0, 1.0, 10, 7);
        let treatment: Vec<f64> = control.iter().map(|v| v + 2.0).collect();
        let paired = ci(&control, &treatment, 0.95, 42, true);
        assert_eq!(paired.method, CiMethod::PairedBootstrap);
        let independent = ci(&control, &treatment, 0.95, 42, false);
        assert_eq!(
            independent.method,
            CiMethod::Bootstrap,
            "n=10 → bootstrap for independent"
        );
        let large_c = normal_sample(0.0, 1.0, 60, 33);
        let large_t: Vec<f64> = large_c.iter().map(|v| v + 1.0).collect();
        let large_ind = ci(&large_c, &large_t, 0.95, 42, false);
        assert_eq!(
            large_ind.method,
            CiMethod::WelchT,
            "n=60 → Welch for independent"
        );
    }
    #[test]
    fn all_ci_primitives_return_confidence_interval_with_method() {
        let samples = normal_sample(5.0, 2.0, 40, 9);
        assert_eq!(student_t_ci(&samples, 0.95).method, CiMethod::StudentT);
        assert_eq!(bootstrap_ci(&samples, 0.95, 42).method, CiMethod::Bootstrap);
        let deltas = vec![1.0, 2.0, 1.5, 3.0, 2.5];
        assert_eq!(
            paired_bootstrap_ci(&deltas, 0.95, 42).method,
            CiMethod::PairedBootstrap
        );
        assert_eq!(wilson_ci(50, 100, 0.95).method, CiMethod::Wilson);
        let c = normal_sample(0.0, 1.0, 30, 5);
        let t: Vec<f64> = c.iter().map(|v| v + 1.0).collect();
        assert_eq!(welch_ci(&c, &t, 0.95).method, CiMethod::WelchT);
    }
    #[test]
    fn effect_size_for_returns_none_on_empty() {
        let empty: Vec<ReplicationScalar> = Vec::new();
        let one = to_scalars(&[1.0]);
        assert!(effect_size_for(&empty, &one, true, 0.95, 1).is_none());
        assert!(effect_size_for(&one, &empty, true, 0.95, 1).is_none());
    }
    #[test]
    fn arm_stat_computes_descriptive_stats() {
        let scalars = to_scalars(&[10.0, 12.0, 11.0, 13.0, 10.0]);
        let stat = arm_stat(&scalars, 0.95, 42).expect("arm_stat");
        assert_eq!(stat.n, 5);
        assert!((stat.mean - 11.2).abs() < 1e-9);
        assert!((stat.median - 11.0).abs() < 1e-9);
        assert!(stat.sd > 0.0, "non-zero sd for varying samples");
        assert!(stat.ci.lower < stat.mean && stat.ci.upper > stat.mean);
    }
    #[test]
    fn arm_stat_none_for_empty() {
        assert!(arm_stat(&[], 0.95, 1).is_none());
    }
    #[test]
    fn paired_t_pvalue_consistent_with_ci() {
        let deltas = vec![1.0, 2.0, 1.5, 3.0, 2.5, 1.8, 2.2, 1.9];
        let p = paired_t_pvalue(&deltas);
        let ci = bootstrap_ci(&deltas, 0.95, 42);
        assert!(
            (p < 0.05) == (ci.lower > 0.0 || ci.upper < 0.0),
            "p={p:.4}, CI=[{:.4}, {:.4}]",
            ci.lower,
            ci.upper
        );
    }
    #[test]
    fn welch_pvalue_consistent_with_welch_ci() {
        let control = vec![10.0, 12.0, 11.0, 13.0, 10.0, 12.0, 11.0, 13.0];
        let treatment = vec![15.0, 17.0, 16.0, 18.0, 15.0, 17.0, 16.0, 18.0];
        let p = welch_pvalue(&control, &treatment);
        let ci = welch_ci(&control, &treatment, 0.95);
        assert!(
            (p < 0.05) == (ci.lower > 0.0 || ci.upper < 0.0),
            "p={p:.4}, CI=[{:.4}, {:.4}]",
            ci.lower,
            ci.upper
        );
    }
    #[test]
    fn pvalue_zero_for_identical_groups() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let p = welch_pvalue(&data, &data);
        assert!(p > 0.9, "identical groups: p={p}");
    }
    #[test]
    fn pvalue_one_for_empty() {
        assert_eq!(paired_t_pvalue(&[]), 1.0);
        assert_eq!(welch_pvalue(&[], &[]), 1.0);
    }
}
