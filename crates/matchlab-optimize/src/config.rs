use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OptConfig {
    pub name: String,
    pub base: String,
    pub seed: u64,
    pub budget: u64,
    pub search_space: SearchSpace,
    pub objectives: Vec<ObjectiveSpec>,
    #[serde(default)]
    pub bo: BoConfig,
    #[serde(default)]
    pub output: OptOutputSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoConfig {
    #[serde(default = "default_initial_design")]
    pub initial_design: String,
    #[serde(default = "default_initial_points")]
    pub initial_points: u64,
    #[serde(default = "default_kernel")]
    pub kernel: String,
    #[serde(default = "default_acquisition")]
    pub acquisition: String,
    #[serde(default)]
    pub xi: Option<f64>,
    #[serde(default)]
    pub eta: Option<f64>,
    #[serde(default)]
    pub ucb_beta: Option<f64>,
    #[serde(default = "default_max_consecutive_failures")]
    pub max_consecutive_failures: Option<u64>,
    #[serde(default)]
    pub batch_size: Option<usize>,
    #[serde(default)]
    pub k_dpp_candidates: Option<usize>,
    #[serde(default)]
    pub gp_phase1_restarts: Option<u64>,
    #[serde(default)]
    pub gp_phase1_inner_iters: Option<u64>,
    #[serde(default)]
    pub gp_phase1_perturbation: Option<f64>,
    #[serde(default)]
    pub gp_phase2_restarts: Option<u64>,
    #[serde(default)]
    pub gp_phase2_inner_iters: Option<u64>,
    #[serde(default)]
    pub gp_phase2_perturbation: Option<f64>,
    #[serde(default)]
    pub min_gp_training_points: Option<usize>,
    #[serde(default)]
    pub early_warning_failures: Option<u64>,
    #[serde(default)]
    pub gp_threads: Option<usize>,
}

impl Default for BoConfig {
    fn default() -> Self {
        Self {
            initial_design: default_initial_design(),
            initial_points: default_initial_points(),
            kernel: default_kernel(),
            acquisition: default_acquisition(),
            xi: None,
            eta: None,
            ucb_beta: None,
            max_consecutive_failures: default_max_consecutive_failures(),
            batch_size: None,
            k_dpp_candidates: None,
            gp_phase1_restarts: None,
            gp_phase1_inner_iters: None,
            gp_phase1_perturbation: None,
            gp_phase2_restarts: None,
            gp_phase2_inner_iters: None,
            gp_phase2_perturbation: None,
            min_gp_training_points: None,
            early_warning_failures: None,
            gp_threads: None,
        }
    }
}

impl BoConfig {
    pub fn xi(&self) -> f64 {
        self.xi.unwrap_or(0.01)
    }
    pub fn eta(&self) -> f64 {
        self.eta.unwrap_or(0.05)
    }
    pub fn ucb_beta(&self) -> f64 {
        self.ucb_beta.unwrap_or(2.0)
    }
    pub fn max_consecutive_failures(&self) -> u64 {
        self.max_consecutive_failures.unwrap_or(10)
    }
    pub fn k_dpp_candidates(&self) -> usize {
        self.k_dpp_candidates.unwrap_or(1000)
    }
    pub fn gp_phase1_restarts(&self) -> u64 {
        self.gp_phase1_restarts.unwrap_or(50)
    }
    pub fn gp_phase1_inner_iters(&self) -> u64 {
        self.gp_phase1_inner_iters.unwrap_or(10)
    }
    pub fn gp_phase1_perturbation(&self) -> f64 {
        self.gp_phase1_perturbation.unwrap_or(0.5)
    }
    pub fn gp_phase2_restarts(&self) -> u64 {
        self.gp_phase2_restarts.unwrap_or(200)
    }
    pub fn gp_phase2_inner_iters(&self) -> u64 {
        self.gp_phase2_inner_iters.unwrap_or(40)
    }
    pub fn gp_phase2_perturbation(&self) -> f64 {
        self.gp_phase2_perturbation.unwrap_or(0.3)
    }
    pub fn min_gp_training_points(&self) -> usize {
        self.min_gp_training_points.unwrap_or(2)
    }
    pub fn early_warning_failures(&self) -> u64 {
        self.early_warning_failures.unwrap_or(5)
    }
    pub fn gp_threads(&self) -> Option<usize> {
        self.gp_threads
    }
}

fn default_initial_design() -> String {
    "latin_hypercube".to_string()
}
fn default_initial_points() -> u64 {
    10
}
fn default_kernel() -> String {
    "matern52".to_string()
}
fn default_acquisition() -> String {
    "ei".to_string()
}
fn default_max_consecutive_failures() -> Option<u64> {
    Some(10)
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OptOutputSpec {
    #[serde(default = "default_opt_directory")]
    pub directory: String,
    #[serde(default)]
    pub report: bool,
    #[serde(default)]
    pub checkpoint: bool,
    pub checkpoint_interval: Option<u64>,
}

fn default_opt_directory() -> String {
    "results/optimization/".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchSpace {
    pub parameters: BTreeMap<String, ParameterSpec>,
}

impl SearchSpace {
    pub fn validate(&self) -> Result<(), String> {
        for (name, spec) in &self.parameters {
            if let ParameterSpec::Float { bounds, .. } = spec {
                if bounds[0] >= bounds[1] {
                    return Err(format!(
                        "{name}: bounds [{}, {}] must satisfy min < max",
                        bounds[0], bounds[1]
                    ));
                }
            }
            if let ParameterSpec::Categorical { values } = spec {
                if values.is_empty() {
                    return Err(format!("{name}: categorical must have at least one value"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ParameterSpec {
    #[serde(rename = "float")]
    Float {
        bounds: [f64; 2],
        #[serde(default)]
        log_scale: bool,
    },
    #[serde(rename = "categorical")]
    Categorical { values: Vec<String> },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObjectiveSpec {
    pub metric: String,
    pub direction: Direction,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Maximize,
    Minimize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub name: String,
    pub seed: u64,
    pub budget: u64,
    pub search_space: SearchSpace,
    pub objectives: Vec<ObjectiveSpec>,
    pub trials: Vec<TrialResult>,
    pub best_index: usize,
    pub pareto_indices: Vec<usize>,
    pub config_hash: String,
    pub git_commit: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    pub trial_index: u64,
    pub parameters: BTreeMap<String, serde_yaml::Value>,
    pub objectives: Vec<f64>,
    pub utility_score: Option<f64>,
    pub matches_completed: u64,
    pub simulated_time_secs: f64,
}

impl OptConfig {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        let raw: serde_yaml::Value =
            serde_yaml::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))?;
        let config: OptConfig =
            serde_yaml::from_value(raw).map_err(|e| format!("validate {}: {e}", path.display()))?;
        config.search_space.validate()?;
        Ok(config)
    }
}

impl ParameterSpec {
    pub fn is_float(&self) -> bool {
        matches!(self, ParameterSpec::Float { .. })
    }

    pub fn is_categorical(&self) -> bool {
        matches!(self, ParameterSpec::Categorical { .. })
    }

    pub fn n_categories(&self) -> usize {
        match self {
            ParameterSpec::Categorical { values } => values.len(),
            _ => 1,
        }
    }

    pub fn bounds(&self) -> Option<[f64; 2]> {
        match self {
            ParameterSpec::Float { bounds, .. } => Some(*bounds),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_config_parses() {
        let yaml = r#"
name: test_opt
base: experiments/base/standard.yaml
seed: 42
budget: 20
search_space:
  parameters:
    experiment.rating.systems.0.k_factor:
      type: float
      bounds: [1.0, 100.0]
    experiment.rating.systems.0.name:
      type: categorical
      values: [elo, glicko2]
objectives:
  - metric: match_quality
    direction: maximize
  - metric: queue_time
    direction: minimize
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "test_opt");
        assert_eq!(config.budget, 20);
        assert_eq!(config.search_space.parameters.len(), 2);
        assert_eq!(config.objectives.len(), 2);
    }

    #[test]
    fn bo_config_defaults() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters: {}
objectives:
  - metric: match_quality
    direction: maximize
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.bo.initial_design, "latin_hypercube");
        assert_eq!(config.bo.initial_points, 10);
        assert_eq!(config.bo.kernel, "matern52");
        assert_eq!(config.bo.acquisition, "ei");
        assert!(config.bo.xi.is_none());
        assert!(config.bo.eta.is_none());
        assert!(config.bo.ucb_beta.is_none());
        assert_eq!(config.bo.max_consecutive_failures, Some(10));
    }

    #[test]
    fn bo_config_custom() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters: {}
objectives:
  - metric: match_quality
    direction: maximize
bo:
  kernel: rbf
  acquisition: ucb
  xi: 0.05
  eta: 0.1
  ucb_beta: 4.0
  max_consecutive_failures: 5
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.bo.kernel, "rbf");
        assert_eq!(config.bo.acquisition, "ucb");
        assert_eq!(config.bo.xi, Some(0.05));
        assert_eq!(config.bo.eta, Some(0.1));
        assert_eq!(config.bo.ucb_beta, Some(4.0));
        assert_eq!(config.bo.max_consecutive_failures, Some(5));
    }

    #[test]
    fn output_config_checkpoint() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters: {}
objectives:
  - metric: match_quality
    direction: maximize
output:
  checkpoint: true
  checkpoint_interval: 5
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.output.checkpoint);
        assert_eq!(config.output.checkpoint_interval, Some(5));
    }

    #[test]
    fn search_space_validate_rejects_inverted_bounds() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters:
    experiment.rating.systems.0.k_factor:
      type: float
      bounds: [100.0, 1.0]
objectives:
  - metric: match_quality
    direction: maximize
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.search_space.validate().is_err());
    }

    #[test]
    fn search_space_validate_rejects_equal_bounds() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters:
    experiment.rating.systems.0.k_factor:
      type: float
      bounds: [5.0, 5.0]
objectives:
  - metric: match_quality
    direction: maximize
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.search_space.validate().is_err());
    }

    #[test]
    fn search_space_validate_rejects_empty_categorical() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters:
    experiment.rating.systems.0.name:
      type: categorical
      values: []
objectives:
  - metric: match_quality
    direction: maximize
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.search_space.validate().is_err());
    }

    #[test]
    fn search_space_validate_accepts_valid_config() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters:
    experiment.rating.systems.0.k_factor:
      type: float
      bounds: [1.0, 100.0]
    experiment.rating.systems.0.name:
      type: categorical
      values: [elo, glicko2]
objectives:
  - metric: match_quality
    direction: maximize
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.search_space.validate().is_ok());
    }

    #[test]
    fn bo_config_parses_new_fields() {
        let yaml = r#"
name: test
base: base.yaml
seed: 1
budget: 10
search_space:
  parameters: {}
objectives:
  - metric: match_quality
    direction: maximize
bo:
  batch_size: 8
  k_dpp_candidates: 500
  gp_phase1_restarts: 20
  gp_phase1_inner_iters: 5
  gp_phase1_perturbation: 0.3
  gp_phase2_restarts: 100
  gp_phase2_inner_iters: 20
  gp_phase2_perturbation: 0.2
  min_gp_training_points: 3
  early_warning_failures: 3
"#;
        let config: OptConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.bo.batch_size, Some(8));
        assert_eq!(config.bo.k_dpp_candidates, Some(500));
        assert_eq!(config.bo.gp_phase1_restarts, Some(20));
        assert_eq!(config.bo.gp_phase1_inner_iters, Some(5));
        assert_eq!(config.bo.gp_phase1_perturbation, Some(0.3));
        assert_eq!(config.bo.gp_phase2_restarts, Some(100));
        assert_eq!(config.bo.gp_phase2_inner_iters, Some(20));
        assert_eq!(config.bo.gp_phase2_perturbation, Some(0.2));
        assert_eq!(config.bo.min_gp_training_points, Some(3));
        assert_eq!(config.bo.early_warning_failures, Some(3));
    }
}
