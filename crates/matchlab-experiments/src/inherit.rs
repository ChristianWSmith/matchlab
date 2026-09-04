//! Config inheritance (spec §13.3).
//!
//! An experiment manifest may declare a `base:` path. The runner loads the
//! base chain first, then deep-merges overrides on top: mappings merge
//! recursively, scalars and sequences are replaced wholesale. Deep-merging the
//! raw YAML values (not the typed structs) is what lets an override supply only
//! `rating:` while inheriting population/game/matchmaking/duration unchanged.

use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml::Value;

use crate::config::ExperimentConfig;

/// Load a manifest, resolving its `base:` chain, and parse to `ExperimentConfig`.
pub fn load(config_path: &Path) -> Result<ExperimentConfig, String> {
    let text = fs::read_to_string(config_path)
        .map_err(|e| format!("cannot read {}: {e}", config_path.display()))?;
    let base_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    resolve_str(&text, &base_dir)
}

/// Parse a manifest string, resolving `base:` relative to `base_dir`.
pub fn resolve_str(text: &str, base_dir: &Path) -> Result<ExperimentConfig, String> {
    let value = resolve_value(text, base_dir)?;
    serde_yaml::from_value::<ExperimentConfig>(value)
        .map_err(|e| format!("invalid experiment config: {e}"))
}

fn resolve_value(text: &str, base_dir: &Path) -> Result<Value, String> {
    let mut doc: Value =
        serde_yaml::from_str(text).map_err(|e| format!("yaml parse error: {e}"))?;
    if let Some(base) = doc.get("base").and_then(Value::as_str) {
        let base_path = base_dir.join(base);
        let base_value = load_value(&base_path)?;
        if let Some(map) = doc.as_mapping_mut() {
            map.remove("base");
        }
        doc = deep_merge(base_value, doc);
    }
    Ok(doc)
}

fn load_value(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("cannot read base {}: {e}", path.display()))?;
    let base_dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    resolve_value(&text, &base_dir)
}

/// Recursive merge: mappings merge key-by-key, every other value is replaced
/// by the override. This is the spec's deep-merge semantics.
fn deep_merge(base: Value, over: Value) -> Value {
    match (base, over) {
        (Value::Mapping(mut base_map), Value::Mapping(over_map)) => {
            for (key, over_val) in over_map {
                match base_map.get(&key) {
                    Some(base_val) => {
                        let merged = deep_merge(base_val.clone(), over_val);
                        base_map.insert(key, merged);
                    }
                    None => {
                        base_map.insert(key, over_val);
                    }
                }
            }
            Value::Mapping(base_map)
        }
        (_, over) => over,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
experiment:
  name: _base
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
    outcome_model: logistic
    beta: 400.0
    noise: 0.05
  matchmaking:
    algorithm: batch
    batch_interval: 10
    max_queue_time: 60.0
  cohorts: []
  duration:
    matches: 1000000
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;

    const OVERRIDE: &str = r#"
base: standard.yaml
experiment:
  name: elo_only
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics:
    - match_quality
    - queue_time
"#;

    #[test]
    fn merge_replaces_sequences_and_scalars_merges_maps() {
        let base: Value = serde_yaml::from_str(BASE).unwrap();
        let over: Value = serde_yaml::from_str("experiment:\n  name: x\n  seed: 7\n  rating:\n    systems:\n      - name: flatpoints\n").unwrap();
        let merged = deep_merge(base, over);

        let exp = merged.get("experiment").unwrap();
        assert_eq!(exp.get("name").and_then(Value::as_str), Some("x"));
        assert_eq!(exp.get("seed").and_then(Value::as_u64), Some(7));
        assert_eq!(
            exp.get("population")
                .and_then(|p| p.get("size"))
                .and_then(Value::as_u64),
            Some(10000),
            "population preserved from base"
        );
        // rating.systems is a sequence → replaced wholesale, not appended
        let systems = exp
            .get("rating")
            .and_then(|r| r.get("systems"))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(systems.len(), 1);
        assert_eq!(
            systems[0].get("name").and_then(Value::as_str),
            Some("flatpoints")
        );
    }

    #[test]
    fn override_only_supplies_rating_and_metrics() {
        let base: Value = serde_yaml::from_str(BASE).unwrap();
        let over: Value = serde_yaml::from_str(OVERRIDE).unwrap();
        let merged = deep_merge(base, over);
        let config: ExperimentConfig = serde_yaml::from_value(merged).unwrap();
        assert_eq!(config.experiment.name, "elo_only");
        assert_eq!(config.experiment.rating.systems.len(), 1);
        assert_eq!(
            config.experiment.metrics,
            vec!["match_quality", "queue_time"]
        );
        assert_eq!(config.experiment.population.size, 10000);
        assert_eq!(config.experiment.game.team_size, 5);
        assert_eq!(config.experiment.duration.matches, 1_000_000);
        assert_eq!(config.experiment.seed, 42);
    }

    #[test]
    fn load_resolves_base_chain_from_disk() {
        let dir =
            std::env::temp_dir().join(format!("matchlab_inherit_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("standard.yaml"), BASE).unwrap();
        fs::write(dir.join("override.yaml"), OVERRIDE).unwrap();

        let config = load(&dir.join("override.yaml")).expect("base chain resolves");
        assert_eq!(config.experiment.name, "elo_only");
        assert_eq!(config.experiment.population.size, 10000);
        assert_eq!(
            config.experiment.metrics,
            vec!["match_quality", "queue_time"]
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_base_file_reports_error() {
        let dir =
            std::env::temp_dir().join(format!("matchlab_inherit_missing_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("x.yaml"),
            "base: nope.yaml\nexperiment:\n  name: x\n",
        )
        .unwrap();
        assert!(load(&dir.join("x.yaml")).is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
