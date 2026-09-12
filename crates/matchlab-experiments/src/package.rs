//! Reproducibility package (T-103).
//!
//! A `ReproductionPackage` captures everything needed to exactly reproduce an
//! experiment run: the manifest YAML, every referenced Lua script, and a
//! metadata fingerprint (config hash, git commit, engine version, seed).
use crate::config::ExperimentConfig;
use crate::seed;
use serde::{Deserialize, Serialize};
use std::path::Path;
/// A self-contained bundle for reproducing an experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReproductionPackage {
    pub manifest: String,
    pub scripts: Vec<(String, String)>,
    pub metadata: PackageMetadata,
}
/// Fingerprint of the code + config that produced a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub config_hash: String,
    pub git_commit: String,
    pub engine_version: String,
    pub timestamp: String,
    pub seed: u64,
}
/// Recursively collect every `.lua` string value in a YAML tree.
fn collect_lua_paths(value: &serde_yaml::Value, out: &mut Vec<String>) {
    match value {
        serde_yaml::Value::String(s) if s.ends_with(".lua") => out.push(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                collect_lua_paths(item, out);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (_, v) in map {
                collect_lua_paths(v, out);
            }
        }
        serde_yaml::Value::Tagged(t) => collect_lua_paths(&t.value, out),
        _ => {}
    }
}
impl ReproductionPackage {
    /// Build a package from a resolved config and its on-disk manifest path.
    pub fn create(config: &ExperimentConfig, manifest_path: &Path) -> Result<Self, String> {
        let manifest = std::fs::read_to_string(manifest_path)
            .map_err(|e| format!("cannot read manifest {}: {e}", manifest_path.display()))?;
        let mut lua_paths: Vec<String> = Vec::new();
        let value = serde_yaml::to_value(config).map_err(|e| format!("yaml error: {e}"))?;
        collect_lua_paths(&value, &mut lua_paths);
        for entry in &config.experiment.metrics {
            lua_paths.push(entry.script_path());
        }
        lua_paths.sort();
        lua_paths.dedup();
        let mut scripts = Vec::new();
        for rel in &lua_paths {
            let resolved = matchlab_lua::resolve::resolve_script_path(rel);
            let content = std::fs::read_to_string(&resolved)
                .map_err(|e| format!("cannot read script {rel}: {e}"))?;
            scripts.push((rel.clone(), content));
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let metadata = PackageMetadata {
            config_hash: seed::hash_config(config),
            git_commit: seed::git_commit_hash(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: format!("{}", now.as_secs()),
            seed: config.experiment.seed,
        };
        Ok(Self {
            manifest,
            scripts,
            metadata,
        })
    }
    /// Write the package as a JSON file.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("serde error: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("write error: {e}"))
    }
    /// Read a package from a JSON file.
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read error: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("parse error: {e}"))
    }
    /// Verify the package's metadata matches a live config.
    pub fn verify(&self, config: &ExperimentConfig) -> Result<bool, String> {
        let live_hash = seed::hash_config(config);
        let live_commit = seed::git_commit_hash();
        Ok(self.metadata.config_hash == live_hash && self.metadata.git_commit == live_commit)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const TINY_CONFIG: &str = r#"
experiment:
  name: tiny
  seed: 42
  population:
    size: 10
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
    teams: { a: 1, b: 1 }
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
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    #[test]
    fn create_and_round_trip() {
        let config: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        let manifest_dir = std::env::temp_dir().join("matchlab_pkg_test");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        let manifest_path = manifest_dir.join("test.yaml");
        std::fs::write(&manifest_path, TINY_CONFIG).unwrap();
        let pkg = ReproductionPackage::create(&config, &manifest_path).unwrap();
        assert!(!pkg.scripts.is_empty());
        assert_eq!(pkg.metadata.seed, 42);
        let out_path = manifest_dir.join("package.json");
        pkg.write(&out_path).unwrap();
        let read_back = ReproductionPackage::read(&out_path).unwrap();
        assert_eq!(read_back.metadata.config_hash, pkg.metadata.config_hash);
        assert_eq!(read_back.scripts.len(), pkg.scripts.len());
        let ok = read_back.verify(&config).unwrap();
        assert!(ok);
        let _ = std::fs::remove_dir_all(&manifest_dir);
    }
    #[test]
    fn verify_detects_mismatched_config() {
        let config: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        let manifest_dir = std::env::temp_dir().join("matchlab_pkg_test2");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        let manifest_path = manifest_dir.join("test.yaml");
        std::fs::write(&manifest_path, TINY_CONFIG).unwrap();
        let pkg = ReproductionPackage::create(&config, &manifest_path).unwrap();
        let mut other_config = config.clone();
        other_config.experiment.seed = 999;
        assert!(!pkg.verify(&other_config).unwrap());
        let _ = std::fs::remove_dir_all(&manifest_dir);
    }
}
