//! First-class estimands (spec §14.x,).
//!
//! An [`Estimand`] states *exactly what quantity is being estimated*: the
//! functional, the outcome metric, and the arm labels. A report that runs
//! through this module can say "the pre-specified estimand is the mean paired
//! difference in replication-level MAE (elo − glicko2)" rather than implying a
//! generic "Glicko was better".
//!
//! The functional shapes are **not interchangeable** (the proposal's warning):
//!
//! ```text
//! mean(A) − mean(B)      # AbsoluteDifference: delta of marginal means
//! mean(A − B)            # MeanPairedDifference: mean of index-paired diffs
//! median(A − B)          # MedianPairedDifference: robust centre of paired diffs
//! ```
//!
//! On skewed matchmaking metrics (`queue_time` congestion tails) they land on
//! visibly different numbers, which is exactly why an explicit estimand type —
//! and not an implied one — is load-bearing. [`estimate_point`] is the single
//! dispatch point for the exact functional; [`effect_for`] adds the CI and
//! self-description for the report layer.
//!
//! Quantile treatment effects, ratios, risk/odds ratios are reserved names not
//! implemented in v0.3.
use crate::hierarchy::ReplicationScalar;
use matchlab_core::rng::SimRng;
use matchlab_experiments::seed::derive;
use serde::{Deserialize, Serialize};
/// Resample count for the percentile bootstrap (same as `effect.rs`).
const N_BOOT: usize = 10_000;
/// The nominal self-description of an estimand: which outcome metric, and over
/// which arms (treatment is always "the thing being compared to control").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimandDef {
    pub outcome: String,
    pub treatment: String,
    pub control: String,
}
/// The declared quantity being estimated . Each variant carries its
/// nominal [`EstimandDef`] so an estimate is self-describing end-to-end.
///
/// Serde (manifest) format is the name string (e.g. `mean_paired_difference`);
/// the def is filled in by the report layer from the metric + arm context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Estimand {
    /// `E[Y_T] − E[Y_C]` — delta of marginal means; samples may be unpaired.
    AbsoluteDifference(EstimandDef),
    /// `(E[Y_T] − E[Y_C]) / |E[Y_C]|` — scale-free change relative to control.
    RelativeDifference(EstimandDef),
    /// `100 · (E[Y_T] − E[Y_C]) / |E[Y_C]|` — `RelativeDifference` as a percent.
    PercentImprovement(EstimandDef),
    /// `mean_i(Y_Ti − Y_Ci)` — mean of index-paired differences (CRN-aware).
    MeanPairedDifference(EstimandDef),
    /// `median_i(Y_Ti − Y_Ci)` — robust centre of index-paired differences.
    MedianPairedDifference(EstimandDef),
}
impl Estimand {
    /// Stable machine name for each variant (also the serde manifest form).
    pub fn name(&self) -> &'static str {
        match self {
            Estimand::AbsoluteDifference(_) => "absolute_difference",
            Estimand::RelativeDifference(_) => "relative_difference",
            Estimand::PercentImprovement(_) => "percent_improvement",
            Estimand::MeanPairedDifference(_) => "mean_paired_difference",
            Estimand::MedianPairedDifference(_) => "median_paired_difference",
        }
    }
    /// Resolve a manifest name to the variant (def empty).
    pub fn from_name(name: &str) -> Option<Estimand> {
        let def = EstimandDef {
            outcome: String::new(),
            treatment: String::new(),
            control: String::new(),
        };
        Some(match name {
            "absolute_difference" => Estimand::AbsoluteDifference(def),
            "relative_difference" => Estimand::RelativeDifference(def),
            "percent_improvement" => Estimand::PercentImprovement(def),
            "mean_paired_difference" => Estimand::MeanPairedDifference(def),
            "median_paired_difference" => Estimand::MedianPairedDifference(def),
            _ => return None,
        })
    }
    pub fn def(&self) -> &EstimandDef {
        match self {
            Estimand::AbsoluteDifference(d)
            | Estimand::RelativeDifference(d)
            | Estimand::PercentImprovement(d)
            | Estimand::MeanPairedDifference(d)
            | Estimand::MedianPairedDifference(d) => d,
        }
    }
    /// Fill in the nominal self-description (outcome + arm labels). The
    /// functional is unchanged, so the def is purely descriptive.
    pub fn with_def(self, def: EstimandDef) -> Estimand {
        match self {
            Estimand::AbsoluteDifference(_) => Estimand::AbsoluteDifference(def),
            Estimand::RelativeDifference(_) => Estimand::RelativeDifference(def),
            Estimand::PercentImprovement(_) => Estimand::PercentImprovement(def),
            Estimand::MeanPairedDifference(_) => Estimand::MeanPairedDifference(def),
            Estimand::MedianPairedDifference(_) => Estimand::MedianPairedDifference(def),
        }
    }
    /// Paired estimands require index-matched samples (equal length).
    pub fn is_paired(&self) -> bool {
        matches!(
            self,
            Estimand::MeanPairedDifference(_) | Estimand::MedianPairedDifference(_)
        )
    }
}
/// The exact functional of an [`Estimand`] applied to raw samples. Returns
/// `None` for empty samples, for unlike indexes on a paired estimand, or for a
/// zero control mean on the relative variants.
pub fn estimate_point(estimand: &Estimand, control: &[f64], treatment: &[f64]) -> Option<f64> {
    if control.is_empty() || treatment.is_empty() {
        return None;
    }
    match estimand {
        Estimand::AbsoluteDifference(_) => Some(mean(treatment) - mean(control)),
        Estimand::RelativeDifference(_) => {
            let mc = mean(control);
            if mc.abs() < f64::EPSILON {
                return None;
            }
            Some((mean(treatment) - mc) / mc.abs())
        }
        Estimand::PercentImprovement(_) => {
            let mc = mean(control);
            if mc.abs() < f64::EPSILON {
                return None;
            }
            Some(100.0 * (mean(treatment) - mc) / mc.abs())
        }
        Estimand::MeanPairedDifference(_) => Some(mean(&paired_deltas(control, treatment)?)),
        Estimand::MedianPairedDifference(_) => Some(median(&paired_deltas(control, treatment)?)),
    }
}
/// The estimate produced by [`effect_for`]: the estimand (self-describing), its
/// point value, and a percentile-bootstrap CI over the same functional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticalEstimate {
    pub estimand: Estimand,
    pub point: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
}
/// A CI for an [`Estimand`]'s point functional. The resampling mirrors the
/// estimand's shape (paired variants resample index-matched pairs; unpaired
/// variants resample each marginal independently), so the interval brackets the
/// *same* quantity the point estimates. Deterministic for a given seed.
fn bootstrap_ci_for(
    estimand: &Estimand,
    control: &[f64],
    treatment: &[f64],
    conf: f64,
    seed: u64,
) -> (f64, f64) {
    if control.is_empty() || treatment.is_empty() {
        return (0.0, 0.0);
    }
    let mut rng = SimRng::from_seed(derive(seed, 0));
    let n = if estimand.is_paired() {
        control.len().min(treatment.len())
    } else {
        control.len()
    };
    let nc = control.len();
    let nt = treatment.len();
    let mut boot = Vec::with_capacity(N_BOOT);
    for _ in 0..N_BOOT {
        let (cs, ts): (Vec<f64>, Vec<f64>) = if estimand.is_paired() {
            let mut cs = Vec::with_capacity(n);
            let mut ts = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = (rng.gen_u64() % n as u64) as usize;
                cs.push(control[idx]);
                ts.push(treatment[idx]);
            }
            (cs, ts)
        } else {
            let cs: Vec<f64> = (0..nc)
                .map(|_| control[(rng.gen_u64() % nc as u64) as usize])
                .collect();
            let ts: Vec<f64> = (0..nt)
                .map(|_| treatment[(rng.gen_u64() % nt as u64) as usize])
                .collect();
            (cs, ts)
        };
        boot.push(estimate_point(estimand, &cs, &ts).unwrap_or(0.0));
    }
    boot.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let alpha = (1.0 - conf) / 2.0;
    (
        percentile_sorted(&boot, alpha),
        percentile_sorted(&boot, 1.0 - alpha),
    )
}
/// Compute a full [`StatisticalEstimate`] for an [`Estimand`] over
/// replication-level scalars. The default (v0.2-compatible) estimand is
/// [`Estimand::MeanPairedDifference`], which reproduces `effect::effect_sizes`
/// `mean_delta` bit-for-bit on equal-length arms.
pub fn effect_for(
    estimand: &Estimand,
    control: &[ReplicationScalar],
    treatment: &[ReplicationScalar],
    conf: f64,
    seed: u64,
) -> Option<StatisticalEstimate> {
    let control_v: Vec<f64> = control.iter().map(ReplicationScalar::value).collect();
    let treatment_v: Vec<f64> = treatment.iter().map(ReplicationScalar::value).collect();
    if estimand.is_paired() && control_v.len() != treatment_v.len() {
        return None;
    }
    let point = estimate_point(estimand, &control_v, &treatment_v)?;
    let (ci_lo, ci_hi) = bootstrap_ci_for(estimand, &control_v, &treatment_v, conf, seed);
    Some(StatisticalEstimate {
        estimand: estimand.clone(),
        point,
        ci_lo,
        ci_hi,
    })
}
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
/// Nearest-rank median (per `matchlab_metrics::stats::Summary.median`): the
/// sorted sample at index `floor(0.5 · (n − 1))` (truncation, not rounding).
fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    percentile_sorted(&sorted, 0.5)
}
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    let idx = (p.clamp(0.0, 1.0) * (sorted.len() - 1) as f64) as usize;
    sorted[idx]
}
fn paired_deltas(control: &[f64], treatment: &[f64]) -> Option<Vec<f64>> {
    if control.len() != treatment.len() {
        return None;
    }
    Some(control.iter().zip(treatment).map(|(c, t)| t - c).collect())
}
impl TryFrom<String> for Estimand {
    type Error = String;
    fn try_from(name: String) -> Result<Estimand, String> {
        Estimand::from_name(&name).ok_or_else(|| format!("unknown estimand: {name}"))
    }
}
impl From<Estimand> for String {
    fn from(estimand: Estimand) -> String {
        estimand.name().to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scalars(values: &[f64]) -> Vec<ReplicationScalar> {
        values
            .iter()
            .map(|v| matchlab_metrics::MetricResult::Scalar(*v))
            .map(|r| crate::hierarchy::per_replication::from_metric(&r).expect("scalarable"))
            .collect()
    }
    fn def() -> EstimandDef {
        EstimandDef {
            outcome: "rating_accuracy".to_string(),
            treatment: "glicko2".to_string(),
            control: "elo".to_string(),
        }
    }
    #[test]
    fn estimate_point_pins_each_functional_on_known_samples() {
        let control = vec![1.0, 2.0, 3.0, 4.0];
        let treatment = vec![2.0, 4.0, 6.0, 8.0];
        let abs = estimate_point(&Estimand::AbsoluteDifference(def()), &control, &treatment)
            .expect("value");
        assert!((abs - 2.5).abs() < 1e-12, "mean(T) − mean(C) = 5 − 2.5");
        let rel = estimate_point(&Estimand::RelativeDifference(def()), &control, &treatment)
            .expect("value");
        assert!((rel - 1.0).abs() < 1e-12, "(5 − 2.5)/2.5");
        let pct = estimate_point(&Estimand::PercentImprovement(def()), &control, &treatment)
            .expect("value");
        assert!((pct - 100.0).abs() < 1e-9, "2.5/2.5 × 100");
        let paired = estimate_point(&Estimand::MeanPairedDifference(def()), &control, &treatment)
            .expect("value");
        assert!((paired - 2.5).abs() < 1e-12);
    }
    #[test]
    fn paired_estimands_require_equal_lengths() {
        let control = vec![1.0, 2.0];
        let treatment = vec![1.0, 2.0, 3.0];
        assert!(
            estimate_point(&Estimand::MeanPairedDifference(def()), &control, &treatment).is_none()
        );
        assert!(
            estimate_point(
                &Estimand::MedianPairedDifference(def()),
                &control,
                &treatment
            )
            .is_none()
        );
        assert!(
            estimate_point(&Estimand::AbsoluteDifference(def()), &control, &treatment).is_some()
        );
    }
    #[test]
    fn empty_and_zero_control_are_safe() {
        assert!(estimate_point(&Estimand::AbsoluteDifference(def()), &[], &[1.0]).is_none());
        assert!(estimate_point(&Estimand::AbsoluteDifference(def()), &[1.0], &[]).is_none());
        assert!(
            estimate_point(&Estimand::RelativeDifference(def()), &[0.0, 0.0], &[1.0]).is_none()
        );
        assert!(estimate_point(&Estimand::PercentImprovement(def()), &[0.0], &[1.0]).is_none());
    }
    /// acceptance: on a skewed synthetic sample the three shapes are NOT
    /// interchangeable — the type is load-bearing, not decorative. The
    /// unpaired functional operates on the full marginals (here control keeps a
    /// congestion tail the aligned treatment lacks); the paired functionals
    /// operate on the index-matched pairs.
    #[test]
    fn skewed_sample_shows_estimands_diverge() {
        let control = vec![200.0, 40.0, 30.0, 20.0, 300.0];
        let treatment = vec![150.0, 60.0, 50.0, 45.0];
        let abs =
            estimate_point(&Estimand::AbsoluteDifference(def()), &control, &treatment).unwrap();
        assert!(
            (abs - (-41.75)).abs() < 1e-12,
            "mean(T) − mean(C) over marginals"
        );
        let aligned_c = vec![200.0, 40.0, 30.0, 20.0];
        let aligned_t = vec![150.0, 60.0, 50.0, 45.0];
        let mean_p = estimate_point(
            &Estimand::MeanPairedDifference(def()),
            &aligned_c,
            &aligned_t,
        )
        .unwrap();
        let med_p = estimate_point(
            &Estimand::MedianPairedDifference(def()),
            &aligned_c,
            &aligned_t,
        )
        .unwrap();
        assert!((mean_p - 3.75).abs() < 1e-12);
        assert!((med_p - 20.0).abs() < 1e-12);
        assert!(mean_p != abs && med_p != abs && mean_p != med_p);
    }
    #[test]
    fn effect_for_default_reproduces_v02_mean_delta() {
        let control_s = scalars(&[1.0, 2.0, 3.0, 4.0]);
        let treatment_s = scalars(&[2.0, 4.0, 6.0, 8.0]);
        let est = effect_for(
            &Estimand::MeanPairedDifference(def()),
            &control_s,
            &treatment_s,
            0.95,
            42,
        )
        .expect("estimate");
        assert!((est.point - 2.5).abs() < 1e-12, "mean_delta = 5 − 2.5");
        let es =
            crate::effect::effect_sizes(&control_s, &treatment_s, true, 0.95, 42).expect("effect");
        assert_eq!(est.point, es.mean_delta);
        assert!((est.ci_lo - es.ci_lo).abs() < 1e-12);
        assert!((est.ci_hi - es.ci_hi).abs() < 1e-12);
    }
    #[test]
    fn name_round_trips_through_serde() {
        for name in [
            "absolute_difference",
            "relative_difference",
            "percent_improvement",
            "mean_paired_difference",
            "median_paired_difference",
        ] {
            let e = Estimand::from_name(name).unwrap();
            assert_eq!(e.name(), name);
            let json = serde_json::to_string(&e).unwrap();
            let back: Estimand = serde_json::from_str(&json).unwrap();
            assert_eq!(back.name(), name);
        }
        assert!(Estimand::from_name("quantile_effect").is_none());
    }
    #[test]
    fn def_is_purely_descriptive() {
        let e = Estimand::MeanPairedDifference(def()).with_def(def());
        let control = vec![1.0, 2.0];
        let treatment = vec![2.0, 3.0];
        let point = estimate_point(&e, &control, &treatment).unwrap();
        assert!((point - 1.0).abs() < 1e-12);
        assert_eq!(e.def().outcome, "rating_accuracy");
        assert_eq!(e.def().treatment, "glicko2");
        assert_eq!(e.def().control, "elo");
    }
    #[test]
    fn median_follows_nearest_rank_summary_convention() {
        let m = median(&[1.0, 2.0, 3.0, 4.0, 100.0]);
        assert_eq!(m, 3.0);
        let even = median(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(even, 2.0);
    }
}
