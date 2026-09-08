//! First-class experimental designs (): describes the structure
//! under which observations were generated. The design propagates through
//! execution, aggregation, analysis, and provenance.
use super::{DesignType, SeedStrategy};
use serde::{Deserialize, Serialize};
/// A pairing key identifies which observations are paired (e.g. by population
/// seed, by block, or by explicit index matching).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingKey {
    pub name: String,
    pub description: String,
}
/// A block groups replications that share some known source of variation .
/// Blocking allows MatchLab to deliberately control variation rather than merely
/// observe it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// What is blocked on (e.g. "population_seed", "scenario", "cohort").
    pub factor: String,
    /// Block labels: one per block level.
    pub levels: Vec<String>,
}
/// An experimental design describes the full structure of an experiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExperimentalDesign {
    /// Independent/unpaired observations with no structural coupling.
    Independent {
        /// Randomized assignment to conditions.
        randomized: bool,
    },
    /// Paired observations (same replication index, shared RNG seed).
    Paired {
        /// The pairing key: what is shared between paired observations.
        pairing: PairingKey,
    },
    /// Common-random-number design: paired observations that share
    /// the same RNG stream for identical stochastic structure.
    Crn {
        /// The pairing key.
        pairing: PairingKey,
    },
    /// Counterfactual design: a live arm paired with a replay arm.
    Counterfactual {
        /// The pairing key.
        pairing: PairingKey,
        /// The parent study ID that was replayed.
        parent_study_id: Option<String>,
    },
    /// Blocked design: replications within a block share a known source of
    /// variation; replications across blocks are independent .
    Blocked {
        /// The blocking factor.
        block: Block,
        /// The underlying paired/independent structure within each block.
        inner: Box<ExperimentalDesign>,
    },
}
impl ExperimentalDesign {
    /// Derive the flat `DesignType` summary from the rich design.
    pub fn design_type(&self) -> DesignType {
        match self {
            ExperimentalDesign::Independent { .. } => DesignType::Independent,
            ExperimentalDesign::Paired { .. } => DesignType::Paired,
            ExperimentalDesign::Crn { .. } => DesignType::Paired,
            ExperimentalDesign::Counterfactual { .. } => DesignType::Counterfactual,
            ExperimentalDesign::Blocked { inner, .. } => inner.design_type(),
        }
    }
    /// Whether this design pairs observations (Paired, Crn, or Counterfactual).
    pub fn is_paired(&self) -> bool {
        self.design_type().is_paired()
    }
    /// Whether this design uses blocking.
    pub fn is_blocked(&self) -> bool {
        matches!(self, ExperimentalDesign::Blocked { .. })
    }
    /// The blocking factor, if any.
    pub fn block(&self) -> Option<&Block> {
        match self {
            ExperimentalDesign::Blocked { block, .. } => Some(block),
            _ => None,
        }
    }
    /// Construct from a `SeedStrategy` (convenience for back-compat).
    pub fn from_strategy(strategy: SeedStrategy) -> Self {
        match strategy {
            SeedStrategy::Independent => ExperimentalDesign::Independent { randomized: false },
            SeedStrategy::Crn => ExperimentalDesign::Crn {
                pairing: PairingKey {
                    name: "seed_strategy".to_string(),
                    description: "paired by CRN seed strategy".to_string(),
                },
            },
            SeedStrategy::Counterfactual => ExperimentalDesign::Counterfactual {
                pairing: PairingKey {
                    name: "seed_strategy".to_string(),
                    description: "paired by counterfactual replay".to_string(),
                },
                parent_study_id: None,
            },
        }
    }
    /// Construct from a `DesignType` (convenience for back-compat).
    pub fn from_design_type(dt: DesignType) -> Self {
        match dt {
            DesignType::Independent => ExperimentalDesign::Independent { randomized: false },
            DesignType::Paired => ExperimentalDesign::Crn {
                pairing: PairingKey {
                    name: "default".to_string(),
                    description: "default paired design".to_string(),
                },
            },
            DesignType::Counterfactual => ExperimentalDesign::Counterfactual {
                pairing: PairingKey {
                    name: "default".to_string(),
                    description: "default counterfactual design".to_string(),
                },
                parent_study_id: None,
            },
        }
    }
}
impl Default for ExperimentalDesign {
    fn default() -> Self {
        ExperimentalDesign::Crn {
            pairing: PairingKey {
                name: "default".to_string(),
                description: "default paired design".to_string(),
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn design_type_derived_correctly() {
        let indep = ExperimentalDesign::Independent { randomized: false };
        assert_eq!(indep.design_type(), DesignType::Independent);
        assert!(!indep.is_paired());
        let paired = ExperimentalDesign::Paired {
            pairing: PairingKey {
                name: "test".into(),
                description: "test".into(),
            },
        };
        assert_eq!(paired.design_type(), DesignType::Paired);
        assert!(paired.is_paired());
        let crn = ExperimentalDesign::Crn {
            pairing: PairingKey {
                name: "test".into(),
                description: "test".into(),
            },
        };
        assert_eq!(crn.design_type(), DesignType::Paired);
        assert!(crn.is_paired());
    }
    #[test]
    fn round_trips_serde() {
        let designs = vec![
            ExperimentalDesign::Independent { randomized: true },
            ExperimentalDesign::Paired {
                pairing: PairingKey {
                    name: "pop_seed".into(),
                    description: "paired by population seed".into(),
                },
            },
            ExperimentalDesign::Crn {
                pairing: PairingKey {
                    name: "master_rng".into(),
                    description: "shared master RNG".into(),
                },
            },
            ExperimentalDesign::Counterfactual {
                pairing: PairingKey {
                    name: "replay".into(),
                    description: "counterfactual replay".into(),
                },
                parent_study_id: Some("study-abc".into()),
            },
        ];
        for d in &designs {
            let json = serde_json::to_string(d).unwrap();
            let back: ExperimentalDesign = serde_json::from_str(&json).unwrap();
            assert_eq!(*d, back);
        }
    }
    #[test]
    fn from_strategy_produces_correct_variant() {
        assert!(matches!(
            ExperimentalDesign::from_strategy(SeedStrategy::Independent),
            ExperimentalDesign::Independent { .. }
        ));
        assert!(matches!(
            ExperimentalDesign::from_strategy(SeedStrategy::Crn),
            ExperimentalDesign::Crn { .. }
        ));
        assert!(matches!(
            ExperimentalDesign::from_strategy(SeedStrategy::Counterfactual),
            ExperimentalDesign::Counterfactual { .. }
        ));
    }
    #[test]
    fn from_design_type_produces_correct_variant() {
        assert!(matches!(
            ExperimentalDesign::from_design_type(DesignType::Independent),
            ExperimentalDesign::Independent { .. }
        ));
        assert!(matches!(
            ExperimentalDesign::from_design_type(DesignType::Paired),
            ExperimentalDesign::Crn { .. }
        ));
        assert!(matches!(
            ExperimentalDesign::from_design_type(DesignType::Counterfactual),
            ExperimentalDesign::Counterfactual { .. }
        ));
    }
    #[test]
    fn default_is_crn_paired() {
        let d = ExperimentalDesign::default();
        assert!(d.is_paired());
        assert_eq!(d.design_type(), DesignType::Paired);
    }
    #[test]
    fn blocked_design_derives_correct_type() {
        let blocked = ExperimentalDesign::Blocked {
            block: Block {
                factor: "population_seed".to_string(),
                levels: vec!["seed1".to_string(), "seed2".to_string()],
            },
            inner: Box::new(ExperimentalDesign::Crn {
                pairing: PairingKey {
                    name: "within_block".to_string(),
                    description: "paired within block".to_string(),
                },
            }),
        };
        assert!(blocked.is_paired());
        assert!(blocked.is_blocked());
        assert_eq!(blocked.design_type(), DesignType::Paired);
        let block = blocked.block().unwrap();
        assert_eq!(block.factor, "population_seed");
        assert_eq!(block.levels.len(), 2);
    }
    #[test]
    fn blocked_round_trips_serde() {
        let d = ExperimentalDesign::Blocked {
            block: Block {
                factor: "scenario".to_string(),
                levels: vec!["A".to_string(), "B".to_string()],
            },
            inner: Box::new(ExperimentalDesign::Independent { randomized: false }),
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: ExperimentalDesign = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
        assert!(!back.is_paired());
        assert!(back.is_blocked());
    }
    #[test]
    fn non_blocked_design_not_blocked() {
        let d = ExperimentalDesign::Independent { randomized: false };
        assert!(!d.is_blocked());
        assert!(d.block().is_none());
    }
}
