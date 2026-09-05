//! Seed derivation and reproducibility bookkeeping (spec §13.7).
//!
//! `SeedManager` derives independent seeds for population, games, arrivals, and
//! behavior from a single experiment seed, so one manifest value reproduces the
//! whole run. `hash_config` and `git_commit_hash` give every `ExperimentResult`
//! an auditable fingerprint of the exact config + code version that produced it.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::config::ExperimentConfig;

pub struct SeedManager {
    pub experiment_seed: u64,
    pub population_seed: u64,
    pub game_seed: u64,
    pub arrival_seed: u64,
    pub behavior_seed: u64,
}

impl SeedManager {
    pub fn from_experiment_seed(seed: u64) -> Self {
        Self {
            experiment_seed: seed,
            population_seed: derive(seed, 1),
            game_seed: derive(seed, 2),
            arrival_seed: derive(seed, 3),
            behavior_seed: derive(seed, 4),
        }
    }
}

pub fn derive(seed: u64, index: u64) -> u64 {
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    index.hash(&mut h);
    h.finish()
}

pub fn hash_config(config: &ExperimentConfig) -> String {
    let mut h = DefaultHasher::new();
    let serialized = serde_yaml::to_string(config).unwrap_or_default();
    serialized.hash(&mut h);
    // Fold in the contents of every referenced Lua script so an uncommitted
    // script edit still changes the experiment identity (scripts are the
    // algorithms now; the manifest alone no longer pins behavior).
    for path in collect_script_paths(config) {
        path.hash(&mut h);
        let resolved = matchlab_lua::resolve::resolve_script_path(&path);
        let content = std::fs::read_to_string(&resolved).unwrap_or_default();
        content.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// Collect every Lua script path a config references: `.lua` string values
/// anywhere in the serialized config, plus the metric names resolved to
/// `plugins/metrics/<name>.lua`. Sorted + deduplicated for determinism.
fn collect_script_paths(config: &ExperimentConfig) -> Vec<String> {
    let mut paths = Vec::new();
    let value = serde_yaml::to_value(config).unwrap_or_default();
    fn walk(v: &serde_yaml::Value, out: &mut Vec<String>) {
        match v {
            serde_yaml::Value::String(s) if s.ends_with(".lua") => out.push(s.clone()),
            serde_yaml::Value::Sequence(seq) => {
                for item in seq {
                    walk(item, out);
                }
            }
            serde_yaml::Value::Mapping(map) => {
                for (_, val) in map {
                    walk(val, out);
                }
            }
            serde_yaml::Value::Tagged(t) => walk(&t.value, out),
            _ => {}
        }
    }
    walk(&value, &mut paths);
    for name in &config.experiment.metrics {
        paths.push(format!("plugins/metrics/{name}.lua"));
    }
    paths.sort();
    paths.dedup();
    paths
}

/// Best-effort: capture the current git commit hash so each `ExperimentResult`
/// records exactly which code version produced it. Falls back to "unknown"
/// when the repo can't be inspected or git is unavailable.
pub fn git_commit_hash() -> String {
    use std::process::Command;
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_distinct_stable_seeds() {
        let seeds = SeedManager::from_experiment_seed(42);
        assert_eq!(seeds.experiment_seed, 42);
        assert_eq!(seeds.population_seed, derive(42, 1));
        assert_eq!(seeds.game_seed, derive(42, 2));
        assert_eq!(seeds.arrival_seed, derive(42, 3));
        assert_eq!(seeds.behavior_seed, derive(42, 4));
        // Different indices must give different seeds for a given experiment seed.
        let distinct = [
            seeds.population_seed,
            seeds.game_seed,
            seeds.arrival_seed,
            seeds.behavior_seed,
        ];
        let mut dedup = distinct.to_vec();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), distinct.len());
    }

    #[test]
    fn derivation_is_deterministic() {
        assert_eq!(derive(1, 1), derive(1, 1));
        assert_ne!(derive(1, 1), derive(1, 2));
        assert_ne!(derive(1, 1), derive(2, 1));
    }

    #[test]
    fn hash_config_changes_with_config() {
        let a: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        let mut b: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        b.experiment.seed = 999;
        let ha = hash_config(&a);
        let hb = hash_config(&b);
        assert_eq!(ha, hash_config(&a));
        assert_ne!(ha, hb);
        assert_eq!(ha.len(), 16);
    }

    #[test]
    fn git_commit_hash_returns_valid_or_unknown() {
        let h = git_commit_hash();
        assert!(h == "unknown" || h.len() >= 7, "got {h:?}");
    }

    #[test]
    fn hash_config_changes_with_script_contents() {
        let a: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        let b: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        let ha = hash_config(&a);
        let hb = hash_config(&b);
        assert_eq!(ha, hb, "identical configs hash identically");
    }

    #[test]
    fn hash_config_detects_script_body_edits() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("matchlab_hash_test_{}.lua", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"k = 1\n").unwrap();

        let mut config: ExperimentConfig = serde_yaml::from_str(TINY_CONFIG).unwrap();
        config.experiment.rating.systems[0].script = Some(path.to_str().unwrap().to_string());
        let first = hash_config(&config);

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"k = 2\n").unwrap();
        let second = hash_config(&config);

        assert_ne!(
            first, second,
            "script body edit must change the config hash"
        );
        let _ = std::fs::remove_file(&path);
    }

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
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
}
