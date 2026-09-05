//! Factorial design (spec §13.5): generate the Cartesian product of config
//! overrides from a base config. Each factor is a dot-separated path into the
//! config tree with a list of values; `generate_configs` produces N = Π|factor_i|
//! configs, each a deep copy of the base with the factor's value applied.

use crate::config::ExperimentConfig;
use serde_yaml::Value;

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

fn set_nested_value(config: &mut ExperimentConfig, path: &str, value: Value) {
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
        MatchmakingSpec, OutputSpec, PopulationSpec, RatingSpec, RatingSystemSpec,
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
                    }],
                },
                game: GameSpec {
                    team_size: 5,
                    outcome_model: "logistic".to_string(),
                    beta: 400.0,
                    noise: 0.05,
                },
                matchmaking: MatchmakingSpec {
                    algorithm: "batch".to_string(),
                    max_queue_time: 60.0,
                    params: BTreeMap::new(),
                },
                rating: RatingSpec {
                    systems: vec![RatingSystemSpec {
                        name: "elo".to_string(),
                        params: BTreeMap::new(),
                    }],
                },
                detection: None,
                ranking: None,
                metrics: vec!["match_quality".to_string()],
                objectives: None,
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
            },
        }
    }

    #[test]
    fn empty_factors_returns_base() {
        let design = FactorialDesign { factors: Vec::new() };
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
        assert_eq!(configs[0].experiment.game.beta, 300.0);
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
        assert_eq!(configs[0].experiment.rating.systems[0].name, "elo");
        assert_eq!(configs[1].experiment.rating.systems[0].name, "flatpoints");
    }
}