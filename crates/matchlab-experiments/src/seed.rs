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
    let serialized = serde_yaml::to_string(config).unwrap_or_default();
    let mut h = DefaultHasher::new();
    serialized.hash(&mut h);
    format!("{:016x}", h.finish())
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
    outcome_model: logistic
    beta: 400.0
    noise: 0.05
  matchmaking:
    algorithm: batch
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
