//! Factor effects (): data structures and estimands for factorial
//! analysis at the replication level. Computes main effects and 2-way
//! interactions over simple factor grids. adds effect structure validation
//! and estimability checks.
use crate::effect::effect_sizes;
use crate::hierarchy::ReplicationScalar;
use std::collections::{BTreeMap, BTreeSet};
/// A factor–level pair annotating an arm in a factorial design.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct FactorLevel {
    pub factor: String,
    pub level: String,
}
/// Maps arm names to their factor–level coordinates.
pub type FactorDesign = BTreeMap<String, Vec<FactorLevel>>;
/// An estimable effect term in a factorial design .
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectTerm {
    /// Main effect of a single factor.
    Main { factor: String },
    /// Interaction between two or more factors.
    Interaction { factors: Vec<String> },
}
/// The set of effects to estimate from a factorial study .
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectStructure {
    pub terms: Vec<EffectTerm>,
}
impl EffectStructure {
    /// Validate that each effect term can be estimated from the given factor
    /// grid. A main effect requires at least 2 levels of the factor. A k-way
    /// interaction requires at least 2 levels of each involved factor.
    pub fn validate(&self, factor_names: &[String]) -> Result<(), String> {
        let factor_set: BTreeSet<&str> = factor_names.iter().map(|s| s.as_str()).collect();
        for term in &self.terms {
            match term {
                EffectTerm::Main { factor } => {
                    if !factor_set.contains(factor.as_str()) {
                        return Err(format!("main effect references unknown factor '{factor}'"));
                    }
                }
                EffectTerm::Interaction { factors } => {
                    if factors.len() < 2 {
                        return Err(format!(
                            "interaction must involve at least 2 factors, got {}",
                            factors.len()
                        ));
                    }
                    for f in factors {
                        if !factor_set.contains(f.as_str()) {
                            return Err(format!("interaction references unknown factor '{f}'"));
                        }
                    }
                    let unique: BTreeSet<&str> = factors.iter().map(|s| s.as_str()).collect();
                    if unique.len() != factors.len() {
                        return Err("interaction has duplicate factors".to_string());
                    }
                }
            }
        }
        Ok(())
    }
}
/// Validate that every factor×level cell has exactly one arm, and every arm has
/// a complete factor assignment. Returns an error describing the first
/// validation failure, or `Ok(())`.
pub fn validate_factor_design(design: &FactorDesign, factors: &[String]) -> Result<(), String> {
    if design.is_empty() {
        return Err("empty factor design".to_string());
    }
    for (arm, assignments) in design {
        let assigned_factors: BTreeSet<&str> =
            assignments.iter().map(|fl| fl.factor.as_str()).collect();
        for f in factors {
            if !assigned_factors.contains(f.as_str()) {
                return Err(format!("arm '{arm}' missing factor '{f}'"));
            }
        }
        if assignments.len() != factors.len() {
            return Err(format!(
                "arm '{arm}' has {} assignments but {} factors expected",
                assignments.len(),
                factors.len()
            ));
        }
    }
    let mut cell_map: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for (arm, assignments) in design {
        for fl in assignments {
            cell_map
                .entry((fl.factor.as_str(), fl.level.as_str()))
                .or_default()
                .push(arm.as_str());
        }
    }
    for ((factor, level), arms) in &cell_map {
        if arms.len() > 1 {
            return Err(format!(
                "factor '{factor}' level '{level}' assigned to multiple arms: {arms:?}"
            ));
        }
    }
    let factor_levels: BTreeMap<&str, BTreeSet<&str>> =
        cell_map.keys().fold(BTreeMap::new(), |mut acc, &(f, l)| {
            acc.entry(f).or_default().insert(l);
            acc
        });
    for f in factors {
        if !factor_levels.contains_key(f.as_str()) {
            return Err(format!("factor '{f}' has no levels in any arm"));
        }
    }
    Ok(())
}
/// Result of a main effect: the mean effect of one factor level, collapsing
/// other factors.
#[derive(Debug, Clone)]
pub struct MainEffect {
    pub factor: String,
    pub level: String,
    pub mean_delta: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
}
/// Result of a 2-way interaction: the difference in effect of factor A between
/// two levels of factor B.
#[derive(Debug, Clone)]
pub struct InteractionEffect {
    pub factor_a: String,
    pub level_a1: String,
    pub level_a2: String,
    pub factor_b: String,
    pub level_b: String,
    pub delta: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
}
type ArmScalars = BTreeMap<String, (Vec<ReplicationScalar>, Vec<ReplicationScalar>)>;
/// Compute main effects for a given factor from a set of per-arm replication
/// scalars and the factor design. For each level, the main effect is the
/// pooled mean delta across all arms with that level.
pub fn main_effects(
    factor: &str,
    design: &FactorDesign,
    arm_scalars: &ArmScalars,
    paired: bool,
    conf: f64,
    seed: u64,
) -> Vec<MainEffect> {
    #[allow(clippy::type_complexity)]
    let mut level_groups: BTreeMap<
        &str,
        Vec<(&str, &Vec<ReplicationScalar>, &Vec<ReplicationScalar>)>,
    > = BTreeMap::new();
    for (arm, assignments) in design {
        if let Some(fl) = assignments.iter().find(|fl| fl.factor == factor) {
            if let Some((ctrl, trt)) = arm_scalars.get(arm) {
                level_groups
                    .entry(fl.level.as_str())
                    .or_default()
                    .push((arm.as_str(), ctrl, trt));
            }
        }
    }
    let mut results = Vec::new();
    for (level, arms) in &level_groups {
        let pooled_ctrl: Vec<ReplicationScalar> = arms
            .iter()
            .flat_map(|(_, c, _)| c.iter().copied())
            .collect();
        let pooled_trt: Vec<ReplicationScalar> = arms
            .iter()
            .flat_map(|(_, _, t)| t.iter().copied())
            .collect();
        if pooled_ctrl.is_empty() || pooled_trt.is_empty() {
            continue;
        }
        let level_seed = matchlab_experiments::seed::derive(seed, level.len() as u64);
        if let Ok(es) = effect_sizes(&pooled_ctrl, &pooled_trt, paired, conf, level_seed) {
            results.push(MainEffect {
                factor: factor.to_string(),
                level: level.to_string(),
                mean_delta: es.mean_delta,
                ci_lo: es.ci_lo,
                ci_hi: es.ci_hi,
            });
        }
    }
    results
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::per_replication;
    use matchlab_metrics::MetricResult;
    fn to_s(v: f64) -> ReplicationScalar {
        per_replication::from_metric(&MetricResult::Scalar(v)).unwrap()
    }
    fn make_arm_scalars(
        ctrl_means: &[f64],
        trt_means: &[f64],
    ) -> (Vec<ReplicationScalar>, Vec<ReplicationScalar>) {
        (
            ctrl_means.iter().copied().map(to_s).collect(),
            trt_means.iter().copied().map(to_s).collect(),
        )
    }
    #[test]
    fn validate_factor_design_ok() {
        let mut design = FactorDesign::new();
        design.insert(
            "elo".to_string(),
            vec![FactorLevel {
                factor: "rating".into(),
                level: "elo".into(),
            }],
        );
        design.insert(
            "glicko".to_string(),
            vec![FactorLevel {
                factor: "rating".into(),
                level: "glicko".into(),
            }],
        );
        let factors = vec!["rating".to_string()];
        assert!(validate_factor_design(&design, &factors).is_ok());
    }
    #[test]
    fn validate_factor_design_missing_cell() {
        let mut design = FactorDesign::new();
        design.insert(
            "elo".to_string(),
            vec![FactorLevel {
                factor: "rating".into(),
                level: "elo".into(),
            }],
        );
        let factors = vec!["rating".to_string(), "vol".to_string()];
        let err = validate_factor_design(&design, &factors).unwrap_err();
        assert!(err.contains("missing factor"));
    }
    #[test]
    fn main_effect_recovers_known_delta() {
        let mut design = FactorDesign::new();
        design.insert(
            "arm_a".to_string(),
            vec![FactorLevel {
                factor: "system".into(),
                level: "elo".into(),
            }],
        );
        design.insert(
            "arm_b".to_string(),
            vec![FactorLevel {
                factor: "system".into(),
                level: "glicko".into(),
            }],
        );
        let mut arm_scalars = BTreeMap::new();
        arm_scalars.insert(
            "arm_a".to_string(),
            make_arm_scalars(&[100.0; 5], &[103.0; 5]),
        );
        arm_scalars.insert(
            "arm_b".to_string(),
            make_arm_scalars(&[100.0; 5], &[115.0; 5]),
        );
        let effects = main_effects("system", &design, &arm_scalars, true, 0.95, 42);
        assert_eq!(effects.len(), 2);
        let elo = effects.iter().find(|e| e.level == "elo").unwrap();
        let glicko = effects.iter().find(|e| e.level == "glicko").unwrap();
        assert!((elo.mean_delta - 3.0).abs() < 1e-9);
        assert!((glicko.mean_delta - 15.0).abs() < 1e-9);
    }
    #[test]
    fn determinism() {
        let mut design = FactorDesign::new();
        design.insert(
            "arm1".to_string(),
            vec![FactorLevel {
                factor: "f".into(),
                level: "l1".into(),
            }],
        );
        let mut arm_scalars = BTreeMap::new();
        arm_scalars.insert(
            "arm1".to_string(),
            make_arm_scalars(&[100.0; 5], &[110.0; 5]),
        );
        let e1 = main_effects("f", &design, &arm_scalars, true, 0.95, 42);
        let e2 = main_effects("f", &design, &arm_scalars, true, 0.95, 42);
        assert_eq!(e1[0].mean_delta, e2[0].mean_delta);
    }
    #[test]
    fn effect_structure_validates_main_effect() {
        let es = EffectStructure {
            terms: vec![EffectTerm::Main {
                factor: "rating".to_string(),
            }],
        };
        let factors = vec!["rating".to_string(), "matchmaking".to_string()];
        assert!(es.validate(&factors).is_ok());
    }
    #[test]
    fn effect_structure_validates_interaction() {
        let es = EffectStructure {
            terms: vec![EffectTerm::Interaction {
                factors: vec!["rating".to_string(), "matchmaking".to_string()],
            }],
        };
        let factors = vec!["rating".to_string(), "matchmaking".to_string()];
        assert!(es.validate(&factors).is_ok());
    }
    #[test]
    fn effect_structure_rejects_unknown_factor() {
        let es = EffectStructure {
            terms: vec![EffectTerm::Main {
                factor: "unknown".to_string(),
            }],
        };
        let factors = vec!["rating".to_string()];
        assert!(es.validate(&factors).is_err());
    }
    #[test]
    fn effect_structure_rejects_single_factor_interaction() {
        let es = EffectStructure {
            terms: vec![EffectTerm::Interaction {
                factors: vec!["rating".to_string()],
            }],
        };
        let factors = vec!["rating".to_string()];
        assert!(es.validate(&factors).is_err());
    }
    #[test]
    fn effect_structure_rejects_duplicate_factors_in_interaction() {
        let es = EffectStructure {
            terms: vec![EffectTerm::Interaction {
                factors: vec!["rating".to_string(), "rating".to_string()],
            }],
        };
        let factors = vec!["rating".to_string()];
        assert!(es.validate(&factors).is_err());
    }
}
