//! Archetype configuration for population generation (spec §5.7).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ArchetypeConfig {
    pub name: String,
    pub proportion: f64,
    pub skill_distribution: DistributionConfig,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    /// If set, overrides sampled skill with this initial rating.
    /// Critical for smurfs: true skill is sampled from the distribution, but
    /// the visible rating starts at this value. No boolean smurf flag is
    /// exposed anywhere (AGENTS.md principle 1).
    #[serde(default)]
    pub initial_rating: Option<f64>,
    /// Optional role label (e.g. `killer` / `survivor`). Absent ⇒ "any" role.
    #[serde(default)]
    pub role: Option<String>,
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
    fn archetype_config_deserializes_from_yaml_with_optional_initial_rating() {
        let yaml = r#"
name: stable
proportion: 0.6
skill_distribution: { type: normal, mean: 1000, stddev: 250 }
skill_volatility: 5.0
improvement_rate: 0.0
play_frequency: 0.8
session_length: 1800.0
quit_probability: 0.01
"#;
        let cfg: ArchetypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.name, "stable");
        assert_eq!(cfg.proportion, 0.6);
        assert_eq!(cfg.initial_rating, None);
    }

    #[test]
    fn archetype_config_parses_initial_rating_when_present() {
        let yaml = r#"
name: smurf
proportion: 0.02
skill_distribution: { type: normal, mean: 1500, stddev: 100 }
skill_volatility: 5.0
improvement_rate: 0.0
play_frequency: 0.95
session_length: 3600.0
quit_probability: 0.002
initial_rating: 700
"#;
        let cfg: ArchetypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.initial_rating, Some(700.0));
    }
}
