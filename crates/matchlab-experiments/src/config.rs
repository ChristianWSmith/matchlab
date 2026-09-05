//! Experiment manifest schema (spec §13.2), v0.1-scoped.
//!
//! Configs serialize/deserialize as YAML so the manifest hash in
//! `ExperimentResult` is stable. Fields outside the v0.1 build (detection,
//! ranking, objectives, cohorts) parse for forward compatibility but are
//! ignored by the v0.1 runner.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExperimentConfig {
    pub experiment: ExperimentSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExperimentSpec {
    pub name: String,
    pub description: Option<String>,
    pub seed: u64,
    pub population: PopulationSpec,
    pub game: GameSpec,
    pub matchmaking: MatchmakingSpec,
    pub rating: RatingSpec,
    #[serde(default)]
    pub detection: Option<DetectionSpec>,
    #[serde(default)]
    pub ranking: Option<RankingSpec>,
    pub metrics: Vec<String>,
    #[serde(default)]
    pub objectives: Option<ObjectiveWeightsSpec>,
    #[serde(default)]
    pub adversarial: Option<AdversarialSpec>,
    #[serde(default)]
    pub satisfaction: Option<SatisfactionSpec>,
    pub cohorts: Vec<CohortSpec>,
    pub duration: DurationSpec,
    pub output: OutputSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PopulationSpec {
    pub size: u64,
    pub seed: u64,
    pub archetypes: Vec<ArchetypeSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArchetypeSpec {
    pub name: String,
    pub proportion: f64,
    pub skill_distribution: DistributionSpec,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    #[serde(default)]
    pub initial_rating: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DistributionSpec {
    #[serde(rename = "normal")]
    Normal { mean: f64, stddev: f64 },
    #[serde(rename = "uniform")]
    Uniform { low: f64, high: f64 },
    #[serde(rename = "log_normal")]
    LogNormal { mean: f64, stddev: f64 },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GameSpec {
    pub team_size: usize,
    /// Path to the Lua outcome-model script (e.g. plugins/game/logistic.lua).
    #[serde(default = "default_outcome_script")]
    pub script: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

fn default_outcome_script() -> String {
    "plugins/game/logistic.lua".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchmakingSpec {
    /// Path to the Lua matchmaker script (e.g. plugins/matchmaking/batch.lua).
    #[serde(default = "default_matchmaker_script")]
    pub script: String,
    pub max_queue_time: f64,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

fn default_matchmaker_script() -> String {
    "plugins/matchmaking/batch.lua".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RatingSpec {
    pub systems: Vec<RatingSystemSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RatingSystemSpec {
    /// Optional label; when present it resolves to a built-in script
    /// (e.g. "elo" → plugins/rating/elo.lua). When absent, `script` is used.
    #[serde(default)]
    pub name: Option<String>,
    /// Path to the Lua rating-system script.
    #[serde(default)]
    pub script: Option<String>,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DetectionSpec {
    pub enabled: bool,
    /// Path to the Lua detection script (e.g. plugins/detection/smurf.lua).
    #[serde(default = "default_detection_script")]
    pub script: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

fn default_detection_script() -> String {
    "plugins/detection/smurf.lua".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RankingSpec {
    /// Path to the Lua rank mapper script (e.g. plugins/ranking/brackets.lua).
    #[serde(default = "default_ranking_script")]
    pub script: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

fn default_ranking_script() -> String {
    "plugins/ranking/brackets.lua".to_string()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AdversarialSpec {
    pub agents: Vec<AdversarialAgentSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdversarialAgentSpec {
    /// Player id the agent is attached to (for single-player agents).
    #[serde(default)]
    pub player: Option<u64>,
    /// Path to the Lua agent script (e.g. plugins/adversarial/afk.lua).
    pub script: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SatisfactionSpec {
    pub enabled: bool,
    /// Path to the Lua satisfaction script (e.g. plugins/utility/satisfaction.lua).
    #[serde(default = "default_satisfaction_script")]
    pub script: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

fn default_satisfaction_script() -> String {
    "plugins/utility/satisfaction.lua".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObjectiveWeightsSpec {
    pub match_quality: Option<f64>,
    pub queue_time: Option<f64>,
    pub rating_accuracy: Option<f64>,
    pub convergence_speed: Option<f64>,
    pub smurf_damage: Option<f64>,
    pub false_positive_rate: Option<f64>,
    pub streak_frustration: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CohortSpec {
    pub name: String,
    pub filter: CohortFilterSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum CohortFilterSpec {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "archetype")]
    Archetype { value: String },
    #[serde(rename = "smurf_by_properties")]
    SmurfByProperties,
    #[serde(rename = "games_played_range")]
    GamesPlayedRange { low: u64, high: u64 },
    #[serde(rename = "skill_range")]
    SkillRange { low: f64, high: f64 },
    #[serde(rename = "party_size")]
    PartySize { size: usize },
    #[serde(rename = "session_length")]
    SessionLength { min: f64, max: f64 },
    #[serde(rename = "rank_tier")]
    RankTier { tier: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DurationSpec {
    pub matches: u64,
    pub max_time: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputSpec {
    pub directory: String,
    pub formats: Vec<String>,
    pub plots: bool,
    pub report: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_v01_manifest_parses() {
        let yaml = r#"
experiment:
  name: v0_1_basic
  description: "Minimal Elo test with static skill population"
  seed: 42
  population:
    size: 10000
    seed: 42
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    team_size: 5
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics:
    - match_quality
    - queue_time
    - rating_accuracy
  cohorts:
    - name: all
      filter: { type: all }
  duration:
    matches: 1000000
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
        let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.experiment.name, "v0_1_basic");
        assert_eq!(config.experiment.seed, 42);
        assert_eq!(config.experiment.population.size, 10000);
        assert_eq!(config.experiment.population.archetypes.len(), 1);
        assert_eq!(config.experiment.game.team_size, 5);
        assert_eq!(
            config.experiment.matchmaking.script,
            "plugins/matchmaking/batch.lua"
        );
        let batch = config
            .experiment
            .matchmaking
            .params
            .get("batch_interval")
            .and_then(|v| v.as_u64());
        assert_eq!(batch, Some(10));
        assert_eq!(config.experiment.rating.systems.len(), 1);
        assert_eq!(
            config.experiment.rating.systems[0].name,
            Some("elo".to_string())
        );
        assert_eq!(
            config.experiment.rating.systems[0]
                .params
                .get("k_factor")
                .and_then(|v| v.as_f64()),
            Some(32.0)
        );
        assert_eq!(config.experiment.metrics.len(), 3);
        assert_eq!(config.experiment.duration.matches, 1_000_000);
        assert_eq!(config.experiment.duration.max_time, 604800.0);
        assert!(config.experiment.detection.is_none());
        assert!(config.experiment.objectives.is_none());
        assert_eq!(config.experiment.cohorts.len(), 1);
    }

    #[test]
    fn config_serializes_back_to_yaml_for_hashing() {
        let yaml = r#"
experiment:
  name: x
  seed: 1
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: uniform, low: 0.0, high: 1000.0 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    team_size: 1
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics:
    - match_quality
  cohorts: []
  duration:
    matches: 10
    max_time: 1000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
        let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
        let text = serde_yaml::to_string(&config).unwrap();
        let reparsed: ExperimentConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(reparsed.experiment.name, "x");
        assert_eq!(reparsed.experiment.population.size, 10);
    }

    #[test]
    fn adversarial_and_satisfaction_specs_parse() {
        let yaml = r#"
experiment:
  name: full
  seed: 1
  population:
    size: 10
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    team_size: 1
    script: plugins/game/fatigue.lua
    beta: 400.0
    noise: 0.05
    fatigue_decay_rate: 0.01
  matchmaking:
    script: plugins/matchmaking/expanding_window.lua
    batch_interval: 10
    max_queue_time: 60.0
    tiers: [[5.0, 25.0], [10.0, 50.0]]
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  detection:
    enabled: true
    script: plugins/detection/smurf.lua
    min_games_before_action: 3
  ranking:
    script: plugins/ranking/brackets.lua
    brackets:
      - { tier: bronze, division: 1, min: 0, max: 1200 }
  adversarial:
    agents:
      - script: plugins/adversarial/afk.lua
        player: 1
        go_afk_probability: 0.5
  satisfaction:
    enabled: true
    script: plugins/utility/satisfaction.lua
    match_quality: 1.0
    queue_time_penalty: -0.01
  metrics: []
  objectives:
    match_quality: 1.0
  cohorts: []
  duration:
    matches: 10
    max_time: 1000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
        let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.experiment.game.script, "plugins/game/fatigue.lua");
        assert_eq!(
            config
                .experiment
                .game
                .params
                .get("fatigue_decay_rate")
                .and_then(|v| v.as_f64()),
            Some(0.01)
        );
        assert_eq!(
            config.experiment.matchmaking.script,
            "plugins/matchmaking/expanding_window.lua"
        );
        assert!(config.experiment.detection.as_ref().unwrap().enabled);
        assert_eq!(
            config
                .experiment
                .ranking
                .as_ref()
                .unwrap()
                .params
                .get("brackets")
                .and_then(|v| v.as_sequence())
                .map(|s| s.len()),
            Some(1)
        );
        assert_eq!(
            config.experiment.adversarial.as_ref().unwrap().agents.len(),
            1
        );
        assert!(config.experiment.satisfaction.as_ref().unwrap().enabled);
        assert!(config.experiment.objectives.is_some());
    }

    #[test]
    fn log_normal_distribution_parses() {
        let yaml = r#"
experiment:
  name: log
  seed: 1
  population:
    size: 100
    seed: 1
    archetypes:
      - name: a
        proportion: 1.0
        skill_distribution: { type: log_normal, mean: 7.0, stddev: 0.5 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    team_size: 1
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics: []
  cohorts: []
  duration:
    matches: 10
    max_time: 1000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
        let config: ExperimentConfig = serde_yaml::from_str(yaml).unwrap();
        let spec = &config.experiment.population.archetypes[0].skill_distribution;
        assert!(matches!(spec, DistributionSpec::LogNormal { .. }));
    }
}
