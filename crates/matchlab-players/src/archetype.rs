//! Archetype configuration for population generation (spec §5.7,).
use serde::Deserialize;
use std::collections::HashMap;
/// Per-dimension skill specification .
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionSpec {
    pub distribution: DistributionConfig,
}
/// Correlation specification between two dimensions .
#[derive(Debug, Clone, Deserialize)]
pub struct CorrelationSpec {
    pub pairs: HashMap<String, f64>,
}
/// Skill dynamics specification .
#[derive(Debug, Clone, Deserialize)]
pub struct DynamicsSpec {
    #[serde(rename = "type")]
    pub dynamics_type: String,
    #[serde(default)]
    pub learning_rate: Option<f64>,
    #[serde(default)]
    pub plateau_games: Option<u64>,
    #[serde(default)]
    pub decay_rate: Option<f64>,
    #[serde(default)]
    pub inactive_threshold: Option<u64>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ArchetypeConfig {
    pub name: String,
    pub proportion: f64,
    /// Single-dimension skill distribution (backward compat: v0.1–v0.4).
    pub skill_distribution: DistributionConfig,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    /// If set, overrides sampled skill with this initial rating.
    #[serde(default)]
    pub initial_rating: Option<f64>,
    /// Optional role label (e.g. `killer` / `survivor`). Absent ⇒ "any" role.
    #[serde(default)]
    pub role: Option<String>,
    /// Multidimensional skill specification . When present, overrides
    /// `skill_distribution` for generation.
    #[serde(default)]
    pub skill_dimensions: Option<HashMap<String, DimensionSpec>>,
    /// Correlation between skill dimensions .
    #[serde(default)]
    pub correlation: Option<CorrelationSpec>,
    /// Skill dynamics specification .
    #[serde(default)]
    pub dynamics: Option<DynamicsSpec>,
}
impl ArchetypeConfig {
    /// Whether this archetype uses multidimensional skill.
    pub fn is_multidimensional(&self) -> bool {
        self.skill_dimensions.is_some()
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum DistributionConfig {
    #[serde(rename = "normal")]
    Normal { mean: f64, stddev: f64 },
    #[serde(rename = "uniform")]
    Uniform { low: f64, high: f64 },
    #[serde(rename = "log_normal")]
    LogNormal { mean: f64, stddev: f64 },
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deserialize_archetype_with_skill_distribution() {
        let yaml = r#"
name: test
proportion: 0.5
skill_distribution:
  type: normal
  mean: 1000.0
  stddev: 250.0
skill_volatility: 5.0
improvement_rate: 0.0
play_frequency: 0.8
session_length: 1800.0
quit_probability: 0.01
"#;
        let config: ArchetypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "test");
        assert!(!config.is_multidimensional());
    }
    #[test]
    fn deserialize_archetype_with_multidim_skill() {
        let yaml = r#"
name: mechanical
proportion: 0.2
skill_distribution:
  type: normal
  mean: 1200.0
  stddev: 200.0
skill_dimensions:
  aim:
    distribution:
      type: normal
      mean: 1500.0
      stddev: 200.0
  movement:
    distribution:
      type: normal
      mean: 1100.0
      stddev: 150.0
skill_volatility: 5.0
improvement_rate: 0.0
play_frequency: 0.8
session_length: 1800.0
quit_probability: 0.01
"#;
        let config: ArchetypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "mechanical");
        assert!(config.is_multidimensional());
        let dims = config.skill_dimensions.as_ref().unwrap();
        assert_eq!(dims.len(), 2);
        assert!(dims.contains_key("aim"));
        assert!(dims.contains_key("movement"));
    }
    #[test]
    fn backward_compat_skill_distribution_still_works() {
        let yaml = r#"
name: test
proportion: 0.5
skill_distribution:
  type: normal
  mean: 1000.0
  stddev: 250.0
skill_volatility: 5.0
improvement_rate: 0.0
play_frequency: 0.8
session_length: 1800.0
quit_probability: 0.01
"#;
        let config: ArchetypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.is_multidimensional());
        assert!(config.skill_dimensions.is_none());
    }
}
