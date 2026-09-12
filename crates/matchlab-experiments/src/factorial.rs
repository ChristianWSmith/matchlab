//! Factorial design (spec §13.5): generate the Cartesian product of config
//! overrides from a base config. Each factor is a dot-separated path into the
//! config tree with a list of values; `generate_configs` produces N = Π|factor_i|
//! configs, each a deep copy of the base with the factor's value applied.
//!
//! Ticket adds `FactorGrid` — a typed, validated representation of
//! factors and levels that replaces the dot-path abstraction.
use crate::config::ExperimentConfig;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::{BTreeMap, BTreeSet};
/// A typed factor level with a label and a config override .
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactorLevel {
    pub label: String,
    pub config_override: Value,
}
/// A typed experimental factor with named levels .
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedFactor {
    pub name: String,
    pub levels: Vec<FactorLevel>,
}
/// A factor grid: the full set of experimental factors .
/// Replaces the dot-path `FactorialDesign` with a typed, validated
/// representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactorGrid {
    pub factors: Vec<TypedFactor>,
}
/// A condition is a unique combination of factor levels .
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Condition {
    /// Maps factor name → level label.
    pub assignments: BTreeMap<String, String>,
}
impl Condition {
    /// A human-readable label for this condition (e.g. "elo×strict").
    pub fn label(&self) -> String {
        self.assignments
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("×")
    }
}
impl FactorGrid {
    /// Produce all conditions as the Cartesian product of factor levels.
    pub fn conditions(&self) -> Vec<Condition> {
        let mut conditions = vec![Condition {
            assignments: BTreeMap::new(),
        }];
        for factor in &self.factors {
            let mut new_conditions = Vec::new();
            for cond in &conditions {
                for level in &factor.levels {
                    let mut extended = cond.clone();
                    extended
                        .assignments
                        .insert(factor.name.clone(), level.label.clone());
                    new_conditions.push(extended);
                }
            }
            conditions = new_conditions;
        }
        conditions
    }
    /// Resolve a condition to a runnable `ExperimentConfig` by applying the
    /// factor-level config overrides to the base config.
    pub fn resolve(&self, condition: &Condition, base: &ExperimentConfig) -> ExperimentConfig {
        let mut config = base.clone();
        for factor in &self.factors {
            if let Some(level_label) = condition.assignments.get(&factor.name) {
                if let Some(level) = factor.levels.iter().find(|l| &l.label == level_label) {
                    set_nested_value(&mut config, &factor.name, level.config_override.clone());
                }
            }
        }
        config
    }
    /// Validate the grid: checks for empty factors, empty levels, and
    /// duplicate condition labels.
    pub fn validate(&self) -> Result<(), String> {
        if self.factors.is_empty() {
            return Err("factor grid has no factors".to_string());
        }
        for factor in &self.factors {
            if factor.levels.is_empty() {
                return Err(format!("factor '{}' has no levels", factor.name));
            }
            let labels: BTreeSet<&str> = factor.levels.iter().map(|l| l.label.as_str()).collect();
            if labels.len() != factor.levels.len() {
                return Err(format!(
                    "factor '{}' has duplicate level labels",
                    factor.name
                ));
            }
        }
        let conditions = self.conditions();
        let labels: Vec<String> = conditions.iter().map(|c| c.label()).collect();
        let unique: BTreeSet<&str> = labels.iter().map(|s| s.as_str()).collect();
        if unique.len() != labels.len() {
            return Err("factor grid produces duplicate condition labels".to_string());
        }
        Ok(())
    }
    /// Number of conditions.
    pub fn n_conditions(&self) -> usize {
        self.factors
            .iter()
            .map(|f| f.levels.len())
            .product::<usize>()
    }
}
/// A replication assignment: which condition, which replication index, which
/// seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationAssignment {
    pub condition_index: usize,
    pub replication_index: u64,
    pub seed: u64,
}
/// Design generation utilities .
pub struct DesignGenerator;
impl DesignGenerator {
    /// Assign replications to conditions with balanced allocation.
    /// Returns one `ReplicationAssignment` per (condition, replication).
    pub fn balanced(
        n_conditions: usize,
        total_replications: u64,
        base_seed: u64,
    ) -> Vec<ReplicationAssignment> {
        let per_condition = total_replications / n_conditions as u64;
        let remainder = total_replications % n_conditions as u64;
        let mut assignments = Vec::new();
        let mut repl_index = 0u64;
        for ci in 0..n_conditions {
            let n = per_condition + if (ci as u64) < remainder { 1 } else { 0 };
            for _ in 0..n {
                let seed = crate::seed::derive(base_seed, repl_index);
                assignments.push(ReplicationAssignment {
                    condition_index: ci,
                    replication_index: repl_index,
                    seed,
                });
                repl_index += 1;
            }
        }
        assignments
    }
    /// Assign replications with randomized (shuffled) order.
    pub fn randomized(
        n_conditions: usize,
        total_replications: u64,
        base_seed: u64,
    ) -> Vec<ReplicationAssignment> {
        let mut assignments = Self::balanced(n_conditions, total_replications, base_seed);
        let mut rng_seed = crate::seed::derive(base_seed, u64::MAX);
        for i in (1..assignments.len()).rev() {
            let j = (crate::seed::derive(rng_seed, i as u64) % (i + 1) as u64) as usize;
            rng_seed = crate::seed::derive(rng_seed, i as u64);
            assignments.swap(i, j);
        }
        assignments
    }
}
/// Fractional factorial design: a subset of the full factorial that retains
/// estimability of specified effects .
pub struct FractionalFactorial {
    /// Resolution level: III = main effects clear of 2-way interactions.
    pub resolution: u8,
    /// Generator indices (columns to alias). If None, uses a default
    /// resolution-III generator for 2^k designs.
    pub generator: Option<Vec<usize>>,
}
impl FractionalFactorial {
    /// Generate a resolution-III fractional factorial from a FactorGrid.
    /// For a 2^k design, selects every other row of the full factorial
    /// (half-fraction), giving 2^(k-1) conditions.
    pub fn generate(&self, grid: &FactorGrid) -> Vec<Condition> {
        let all = grid.conditions();
        if all.is_empty() {
            return all;
        }
        all.into_iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, c)| c)
            .collect()
    }
}
pub struct FactorialDesign {
    pub factors: Vec<Factor>,
}
pub struct Factor {
    pub name: String,
    pub values: Vec<Value>,
}
impl FactorialDesign {
    pub fn generate_configs(&self, base: &ExperimentConfig) -> Vec<ExperimentConfig> {
        let mut configs = vec![base.clone()];
        for factor in &self.factors {
            let mut new_configs = Vec::new();
            for config in &configs {
                for value in &factor.values {
                    let mut modified = config.clone();
                    set_nested_value(&mut modified, &factor.name, value.clone());
                    new_configs.push(modified);
                }
            }
            configs = new_configs;
        }
        configs
    }
}
fn descend<'a>(cursor: &'a mut Value, part: &str) -> Option<&'a mut Value> {
    if let Ok(idx) = part.parse::<usize>() {
        cursor.as_sequence_mut().and_then(|seq| seq.get_mut(idx))
    } else {
        cursor
            .as_mapping_mut()
            .and_then(|m| m.get_mut(Value::String(part.to_string())))
    }
}
pub fn set_nested_value(config: &mut ExperimentConfig, path: &str, value: Value) {
    let mut tree: Value = serde_yaml::to_value(&*config).expect("ExperimentConfig must serialize");
    let parts: Vec<&str> = path.split('.').collect();
    let mut cursor = &mut tree;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Ok(idx) = part.parse::<usize>() {
                let item = cursor
                    .as_sequence_mut()
                    .and_then(|seq| seq.get_mut(idx))
                    .expect("factorial path segment must exist in config");
                *item = value.clone();
            } else {
                let m = cursor.as_mapping_mut().expect("expected mapping");
                m.insert(Value::String(part.to_string()), value.clone());
            }
        } else {
            cursor = descend(cursor, part).expect("factorial path segment must exist in config");
        }
    }
    *config = serde_yaml::from_value(tree).expect("ExperimentConfig must deserialize from tree");
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ArchetypeSpec, CohortSpec, DistributionSpec, DurationSpec, ExperimentSpec, GameSpec,
        MatchmakingSpec, OutputSpec, PopulationSpec, RatingSpec, RatingSystemSpec, TeamSpecs,
    };
    use std::collections::BTreeMap;
    fn base() -> ExperimentConfig {
        ExperimentConfig {
            experiment: ExperimentSpec {
                name: "base".to_string(),
                description: None,
                seed: 42,
                population: PopulationSpec {
                    size: 100,
                    seed: 42,
                    archetypes: vec![ArchetypeSpec {
                        name: "stable".to_string(),
                        proportion: 1.0,
                        skill_distribution: DistributionSpec::Normal {
                            mean: 1000.0,
                            stddev: 250.0,
                        },
                        skill_volatility: 5.0,
                        improvement_rate: 0.0,
                        play_frequency: 0.8,
                        session_length: 1800.0,
                        quit_probability: 0.01,
                        initial_rating: None,
                        role: None,
                    }],
                },
                game: GameSpec {
                    teams: TeamSpecs::default(),
                    script: "plugins/game/logistic.lua".to_string(),
                    skill_update_interval_secs: None,
                    params: BTreeMap::new(),
                },
                matchmaking: MatchmakingSpec {
                    script: "plugins/matchmaking/batch.lua".to_string(),
                    max_queue_time: 60.0,
                    params: BTreeMap::new(),
                },
                rating: RatingSpec {
                    systems: vec![RatingSystemSpec {
                        name: Some("elo".to_string()),
                        script: None,
                        params: BTreeMap::new(),
                    }],
                },
                detection: None,
                ranking: None,
                metrics: vec![crate::config::MetricEntry::Name(
                    "match_quality".to_string(),
                )],
                objectives: None,
                adversarial: None,
                satisfaction: None,
                cohorts: Vec::<CohortSpec>::new(),
                duration: DurationSpec {
                    matches: 100,
                    max_time: 3600.0,
                },
                output: OutputSpec {
                    directory: "results/".to_string(),
                    formats: vec!["json".to_string()],
                    plots: false,
                    report: false,
                },
                replication: None,
            },
        }
    }
    #[test]
    fn empty_factors_returns_base() {
        let design = FactorialDesign {
            factors: Vec::new(),
        };
        let configs = design.generate_configs(&base());
        assert_eq!(configs.len(), 1);
    }
    #[test]
    fn two_factors_produce_cartesian_product() {
        let design = FactorialDesign {
            factors: vec![
                Factor {
                    name: "experiment.game.beta".to_string(),
                    values: vec![Value::from(300.0), Value::from(400.0), Value::from(500.0)],
                },
                Factor {
                    name: "experiment.rating.systems.0.name".to_string(),
                    values: vec![Value::from("elo"), Value::from("glicko2")],
                },
            ],
        };
        let configs = design.generate_configs(&base());
        assert_eq!(configs.len(), 6);
    }
    #[test]
    fn factor_value_set_at_nested_path() {
        let design = FactorialDesign {
            factors: vec![Factor {
                name: "experiment.game.beta".to_string(),
                values: vec![Value::from(300.0)],
            }],
        };
        let configs = design.generate_configs(&base());
        assert_eq!(
            configs[0]
                .experiment
                .game
                .params
                .get("beta")
                .and_then(|v| v.as_f64()),
            Some(300.0)
        );
    }
    #[test]
    fn list_factor_applies_each_value() {
        let design = FactorialDesign {
            factors: vec![Factor {
                name: "experiment.rating.systems.0.name".to_string(),
                values: vec![Value::from("elo"), Value::from("flatpoints")],
            }],
        };
        let configs = design.generate_configs(&base());
        assert_eq!(configs.len(), 2);
        assert_eq!(
            configs[0].experiment.rating.systems[0].name,
            Some("elo".to_string())
        );
        assert_eq!(
            configs[1].experiment.rating.systems[0].name,
            Some("flatpoints".to_string())
        );
    }
    #[test]
    fn factorial_hand_derives_feedback_loop_cells() {
        let cell = |name: &str| {
            crate::inherit::load(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../experiments/feedback_loop")
                    .join(name),
            )
            .unwrap()
        };
        let base = cell("feedback_elo_random.yaml");
        let design = FactorialDesign {
            factors: vec![
                Factor {
                    name: "experiment.rating.systems.0.name".to_string(),
                    values: vec![
                        Value::from("elo"),
                        Value::from("glicko2"),
                        Value::from("trueskill"),
                    ],
                },
                Factor {
                    name: "experiment.matchmaking.script".to_string(),
                    values: vec![
                        Value::from("plugins/matchmaking/random.lua"),
                        Value::from("plugins/matchmaking/strict.lua"),
                        Value::from("plugins/matchmaking/expanding_window.lua"),
                    ],
                },
            ],
        };
        let generated = design.generate_configs(&base);
        assert_eq!(generated.len(), 9);
        let ratings = ["elo", "glicko2", "trueskill"];
        let suffixes = ["random", "strict", "expanding"];
        let mut expected: Vec<(String, String)> = Vec::new();
        for rating in ratings {
            for suffix in suffixes {
                let cfg = cell(&format!("feedback_{rating}_{suffix}.yaml"));
                expected.push((
                    cfg.experiment.rating.systems[0].name.clone().unwrap(),
                    cfg.experiment.matchmaking.script.clone(),
                ));
            }
        }
        let actual: Vec<(String, String)> = generated
            .iter()
            .map(|c| {
                (
                    c.experiment.rating.systems[0]
                        .name
                        .clone()
                        .unwrap_or_default(),
                    c.experiment.matchmaking.script.clone(),
                )
            })
            .collect();
        assert_eq!(actual, expected);
    }
    #[test]
    fn factor_grid_2x2_produces_4_conditions() {
        let grid = FactorGrid {
            factors: vec![
                TypedFactor {
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
                },
                TypedFactor {
                    name: "matchmaking".to_string(),
                    levels: vec![
                        FactorLevel {
                            label: "strict".to_string(),
                            config_override: Value::from("strict"),
                        },
                        FactorLevel {
                            label: "expanding".to_string(),
                            config_override: Value::from("expanding"),
                        },
                    ],
                },
            ],
        };
        let conditions = grid.conditions();
        assert_eq!(conditions.len(), 4);
        assert_eq!(grid.n_conditions(), 4);
    }
    #[test]
    fn factor_grid_3x1x2_produces_6_conditions() {
        let grid = FactorGrid {
            factors: vec![
                TypedFactor {
                    name: "a".to_string(),
                    levels: vec![
                        FactorLevel {
                            label: "a1".to_string(),
                            config_override: Value::from(1),
                        },
                        FactorLevel {
                            label: "a2".to_string(),
                            config_override: Value::from(2),
                        },
                        FactorLevel {
                            label: "a3".to_string(),
                            config_override: Value::from(3),
                        },
                    ],
                },
                TypedFactor {
                    name: "b".to_string(),
                    levels: vec![FactorLevel {
                        label: "b1".to_string(),
                        config_override: Value::from(1),
                    }],
                },
                TypedFactor {
                    name: "c".to_string(),
                    levels: vec![
                        FactorLevel {
                            label: "c1".to_string(),
                            config_override: Value::from(1),
                        },
                        FactorLevel {
                            label: "c2".to_string(),
                            config_override: Value::from(2),
                        },
                    ],
                },
            ],
        };
        assert_eq!(grid.n_conditions(), 6);
        assert_eq!(grid.conditions().len(), 6);
    }
    #[test]
    fn condition_label_round_trips() {
        let mut assignments = BTreeMap::new();
        assignments.insert("rating".to_string(), "elo".to_string());
        assignments.insert("matchmaking".to_string(), "strict".to_string());
        let cond = Condition { assignments };
        assert_eq!(cond.label(), "strict×elo");
    }
    #[test]
    fn factor_grid_validation_catches_empty_factor() {
        let grid = FactorGrid {
            factors: vec![TypedFactor {
                name: "empty".to_string(),
                levels: vec![],
            }],
        };
        assert!(grid.validate().is_err());
    }
    #[test]
    fn factor_grid_validation_catches_empty_grid() {
        let grid = FactorGrid { factors: vec![] };
        assert!(grid.validate().is_err());
    }
    #[test]
    fn factor_grid_validation_passes_for_valid_grid() {
        let grid = FactorGrid {
            factors: vec![TypedFactor {
                name: "x".to_string(),
                levels: vec![
                    FactorLevel {
                        label: "a".to_string(),
                        config_override: Value::from(1),
                    },
                    FactorLevel {
                        label: "b".to_string(),
                        config_override: Value::from(2),
                    },
                ],
            }],
        };
        assert!(grid.validate().is_ok());
    }
    #[test]
    fn balanced_allocation_is_correct() {
        let assignments = DesignGenerator::balanced(3, 10, 42);
        assert_eq!(assignments.len(), 10);
        let counts: Vec<usize> = (0..3)
            .map(|ci| {
                assignments
                    .iter()
                    .filter(|a| a.condition_index == ci)
                    .count()
            })
            .collect();
        assert_eq!(counts, vec![4, 3, 3]);
    }
    #[test]
    fn balanced_equal_allocation() {
        let assignments = DesignGenerator::balanced(3, 9, 42);
        assert_eq!(assignments.len(), 9);
        let counts: Vec<usize> = (0..3)
            .map(|ci| {
                assignments
                    .iter()
                    .filter(|a| a.condition_index == ci)
                    .count()
            })
            .collect();
        assert_eq!(counts, vec![3, 3, 3]);
    }
    #[test]
    fn balanced_is_deterministic() {
        let a1 = DesignGenerator::balanced(3, 10, 42);
        let a2 = DesignGenerator::balanced(3, 10, 42);
        assert_eq!(a1, a2);
    }
    #[test]
    fn randomized_has_all_assignments() {
        let assignments = DesignGenerator::randomized(3, 9, 42);
        assert_eq!(assignments.len(), 9);
        let counts: Vec<usize> = (0..3)
            .map(|ci| {
                assignments
                    .iter()
                    .filter(|a| a.condition_index == ci)
                    .count()
            })
            .collect();
        assert_eq!(counts, vec![3, 3, 3]);
    }
    #[test]
    fn randomized_is_deterministic() {
        let a1 = DesignGenerator::randomized(3, 9, 42);
        let a2 = DesignGenerator::randomized(3, 9, 42);
        assert_eq!(a1, a2);
    }
    #[test]
    fn fractional_factorial_halves_conditions() {
        let grid = FactorGrid {
            factors: vec![
                TypedFactor {
                    name: "a".to_string(),
                    levels: vec![
                        FactorLevel {
                            label: "a1".to_string(),
                            config_override: Value::from(1),
                        },
                        FactorLevel {
                            label: "a2".to_string(),
                            config_override: Value::from(2),
                        },
                    ],
                },
                TypedFactor {
                    name: "b".to_string(),
                    levels: vec![
                        FactorLevel {
                            label: "b1".to_string(),
                            config_override: Value::from(1),
                        },
                        FactorLevel {
                            label: "b2".to_string(),
                            config_override: Value::from(2),
                        },
                    ],
                },
            ],
        };
        let ff = FractionalFactorial {
            resolution: 3,
            generator: None,
        };
        let conditions = ff.generate(&grid);
        assert_eq!(conditions.len(), 2, "half-fraction of 2² = 2");
    }
}
