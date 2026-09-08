//! Experimental design validation: machine-checkable design
//! validation that detects issues early, before simulation resources are spent.
use super::design::ExperimentalDesign;
use super::factorial::FactorGrid;
/// A validation error in an experimental design.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DesignError {
    pub code: String,
    pub message: String,
    pub location: Option<String>,
}
/// A validation warning (non-fatal).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DesignWarning {
    pub code: String,
    pub message: String,
}
/// Validation result for an experimental design.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DesignValidation {
    pub errors: Vec<DesignError>,
    pub warnings: Vec<DesignWarning>,
}
impl DesignValidation {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}
/// Validate a complete experimental design .
pub fn validate_design(
    design: &ExperimentalDesign,
    factor_grid: Option<&FactorGrid>,
    n_replications_per_condition: Option<u64>,
    n_conditions: Option<usize>,
) -> DesignValidation {
    let mut result = DesignValidation::default();
    if let Some(grid) = factor_grid {
        if let Err(e) = grid.validate() {
            result.errors.push(DesignError {
                code: "FACTOR_GRID_INVALID".to_string(),
                message: e,
                location: Some("factor_grid".to_string()),
            });
        }
    }
    if let Some(n) = n_replications_per_condition {
        if n < 2 {
            result.errors.push(DesignError {
                code: "INSUFFICIENT_REPLICATION".to_string(),
                message: format!("n={n} per condition; at least 2 required for CI"),
                location: Some("replication_count".to_string()),
            });
        }
    }
    if let Some(nc) = n_conditions {
        if nc == 0 {
            result.errors.push(DesignError {
                code: "ZERO_CONDITIONS".to_string(),
                message: "design has 0 conditions".to_string(),
                location: Some("conditions".to_string()),
            });
        }
    }
    match design {
        ExperimentalDesign::Independent { .. } => {}
        ExperimentalDesign::Paired { .. } | ExperimentalDesign::Crn { .. } => {
            result.warnings.push(DesignWarning {
                code: "PAIRED_REQUIRES_EQUAL_ARMS".to_string(),
                message: "paired/CRN design requires equal-length arms at analysis time"
                    .to_string(),
            });
        }
        ExperimentalDesign::Counterfactual {
            parent_study_id, ..
        } => {
            if parent_study_id.is_none() {
                result.errors.push(DesignError {
                    code: "COUNTERFACTUAL_MISSING_PARENT".to_string(),
                    message: "counterfactual design requires parent_study_id".to_string(),
                    location: Some("experimental_design".to_string()),
                });
            }
        }
        ExperimentalDesign::Blocked { block, .. } => {
            if block.levels.is_empty() {
                result.errors.push(DesignError {
                    code: "BLOCKED_NO_LEVELS".to_string(),
                    message: "blocked design has no block levels".to_string(),
                    location: Some("block".to_string()),
                });
            }
            if block.factor.is_empty() {
                result.errors.push(DesignError {
                    code: "BLOCKED_NO_FACTOR".to_string(),
                    message: "blocked design has no factor name".to_string(),
                    location: Some("block.factor".to_string()),
                });
            }
        }
    }
    if let Some(grid) = factor_grid {
        if let ExperimentalDesign::Blocked { block, .. } = design {
            if !block.factor.is_empty() && grid.factors.iter().any(|f| f.name == block.factor) {
                result.warnings.push(DesignWarning {
                    code: "BLOCKING_FACTOR_IS_EXPERIMENTAL".to_string(),
                    message: format!(
                        "blocking factor '{}' is also an experimental factor; \
                         ensure this is intentional",
                        block.factor
                    ),
                });
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Block, PairingKey};
    use crate::factorial::{FactorLevel, TypedFactor};
    use serde_yaml::Value;
    fn valid_grid() -> FactorGrid {
        FactorGrid {
            factors: vec![TypedFactor {
                name: "rating".to_string(),
                levels: vec![
                    FactorLevel {
                        label: "elo".to_string(),
                        config_override: Value::from("elo"),
                    },
                    FactorLevel {
                        label: "glicko".to_string(),
                        config_override: Value::from("glicko"),
                    },
                ],
            }],
        }
    }
    #[test]
    fn valid_design_passes() {
        let design = ExperimentalDesign::Crn {
            pairing: PairingKey {
                name: "test".into(),
                description: "test".into(),
            },
        };
        let result = validate_design(&design, Some(&valid_grid()), Some(10), Some(2));
        assert!(result.is_valid(), "valid design: {:?}", result.errors);
    }
    #[test]
    fn insufficient_replication_fails() {
        let design = ExperimentalDesign::Independent { randomized: false };
        let result = validate_design(&design, None, Some(1), None);
        assert!(!result.is_valid());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == "INSUFFICIENT_REPLICATION")
        );
    }
    #[test]
    fn zero_conditions_fails() {
        let design = ExperimentalDesign::Independent { randomized: false };
        let result = validate_design(&design, None, None, Some(0));
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.code == "ZERO_CONDITIONS"));
    }
    #[test]
    fn counterfactual_without_parent_fails() {
        let design = ExperimentalDesign::Counterfactual {
            pairing: PairingKey {
                name: "test".into(),
                description: "test".into(),
            },
            parent_study_id: None,
        };
        let result = validate_design(&design, None, None, None);
        assert!(!result.is_valid());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == "COUNTERFACTUAL_MISSING_PARENT")
        );
    }
    #[test]
    fn blocked_without_levels_fails() {
        let design = ExperimentalDesign::Blocked {
            block: Block {
                factor: "scenario".to_string(),
                levels: vec![],
            },
            inner: Box::new(ExperimentalDesign::Independent { randomized: false }),
        };
        let result = validate_design(&design, None, None, None);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.code == "BLOCKED_NO_LEVELS"));
    }
    #[test]
    fn invalid_factor_grid_fails() {
        let grid = FactorGrid {
            factors: vec![crate::factorial::TypedFactor {
                name: "empty".to_string(),
                levels: vec![],
            }],
        };
        let design = ExperimentalDesign::Independent { randomized: false };
        let result = validate_design(&design, Some(&grid), None, None);
        assert!(!result.is_valid());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == "FACTOR_GRID_INVALID")
        );
    }
    #[test]
    fn validation_is_deterministic() {
        let design = ExperimentalDesign::Independent { randomized: false };
        let r1 = validate_design(&design, None, Some(1), None);
        let r2 = validate_design(&design, None, Some(1), None);
        assert_eq!(r1.errors, r2.errors);
    }
}
