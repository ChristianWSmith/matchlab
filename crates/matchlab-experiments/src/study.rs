//! Study manifests and runner (spec §13.8,).
//!
//! Two manifest shapes drive multi-replicate studies, differing only in
//! ergonomics: an embedded `replication:` block inside an experiment manifest
//! (single-condition repetition) and a top-level `study.yaml` with a `base:`
//! config plus named arms (held-constant-except-factor comparisons). Arms are
//! "factor overrides applied to a shared base" — the controlled
//! one-variable-differs guarantee — applied via `factorial::set_nested_value`.
use crate::config::CohortSpec;
use crate::factorial::set_nested_value;
use crate::inherit;
use crate::replicate::{ArmConfig, ReplicationRunner, ReplicationSpec, StudyResult};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
/// Top-level study manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyConfig {
    pub study: StudySpec,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudySpec {
    pub name: String,
    /// Inherited base experiment manifest (spec §13.3).
    pub base: String,
    pub arms: Vec<ArmSpec>,
    pub replication: ReplicationSpec,
    /// Study-level settings applied to every arm.
    #[serde(default)]
    pub metrics: Vec<crate::config::MetricEntry>,
    #[serde(default)]
    pub cohorts: Vec<CohortSpec>,
    /// Optional declared primary estimand . A name string parsed by
    /// `matchlab-analysis` (`Estimand::from_name`); absent → the v0.2 default
    /// `MeanPairedDifference`. Consulted by the analysis layer only.
    #[serde(default)]
    pub estimand: Option<String>,
    pub output: StudyOutputSpec,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmSpec {
    pub name: String,
    /// Dotted-path overrides applied to the base config. May not touch the
    /// seed — replication owns seeds.
    #[serde(default)]
    pub overrides: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyOutputSpec {
    pub directory: String,
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,
}
fn default_formats() -> Vec<String> {
    vec!["json".to_string()]
}
pub struct StudyRunner;
impl StudyRunner {
    /// Load a study manifest from disk, resolving its `base:` path relative to
    /// the study file's directory.
    pub fn load(path: &Path) -> Result<StudyConfig, String> {
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut config: StudyConfig =
            serde_yaml::from_str(&text).map_err(|e| format!("invalid study manifest: {e}"))?;
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let base_path = PathBuf::from(&config.study.base);
        if !base_path.is_absolute() {
            config.study.base = base_dir.join(base_path).to_string_lossy().into_owned();
        }
        Ok(config)
    }
    /// Resolve the base plus each arm's overrides into `ArmConfig`s, then
    /// delegate to `ReplicationRunner`.
    ///
    /// `config_hash` semantics (documented): `StudyResult.config_hash` is the
    /// hash of arm `0`'s resolved full config (base + that arm's overrides +
    /// study-level `metrics`/`cohorts`).
    pub fn run(study: &StudyConfig, threads: usize) -> Result<StudyResult, String> {
        let mut base_config = inherit::load(Path::new(&study.study.base))?;
        base_config.experiment.metrics = study.study.metrics.clone();
        base_config.experiment.cohorts = study.study.cohorts.clone();
        let mut arms: Vec<ArmConfig> = Vec::with_capacity(study.study.arms.len());
        for arm in &study.study.arms {
            let mut cfg = base_config.clone();
            for (path, value) in &arm.overrides {
                if path == "experiment.seed" || path == "seed" {
                    return Err(format!(
                        "arm '{}' may not override the seed (replication owns seeds)",
                        arm.name
                    ));
                }
                set_nested_value(&mut cfg, path, value.clone());
            }
            arms.push(ArmConfig {
                name: arm.name.clone(),
                config: cfg,
            });
        }
        let mut result = ReplicationRunner::run_arms(&arms, &study.study.replication, threads)?;
        result.name = study.study.name.clone();
        let hash8 = &result.config_hash[..8];
        result.study_id = format!(
            "{}-{hash8}-{}-r{}",
            study.study.name,
            study.study.replication.strategy.key(),
            study.study.replication.count
        );
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::replicate::SeedStrategy;
    use std::io::Write;
    const MINI_BASE: &str = r#"
experiment:
  name: mini-base
  seed: 1
  population:
    size: 20
    seed: 1
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
  metrics: [match_quality]
  cohorts: []
  duration:
    matches: 6
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;
    fn temp_base() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "matchlab_study_base_{}_{}.yaml",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut f = std::fs::File::create(&path).expect("create base");
        f.write_all(MINI_BASE.as_bytes()).expect("write base");
        path.to_string_lossy().into_owned()
    }
    fn study_yaml(base: &str) -> String {
        format!(
            r#"
study:
  name: mini_study
  base: {base}
  arms:
    - name: elo
      overrides:
        experiment.rating.systems.0.name: elo
    - name: flatpoints
      overrides:
        experiment.rating.systems.0.name: flatpoints
        experiment.rating.systems.0.win_points: 10.0
        experiment.rating.systems.0.loss_points: 10.0
  replication:
    count: 2
    strategy: crn
    base_seed: 42
  metrics: [match_quality]
  cohorts: []
  output:
    directory: results/studies/mini/
"#
        )
    }
    #[test]
    fn study_config_parses_both_shapes() {
        let base = temp_base();
        let text = study_yaml(&base);
        let config: StudyConfig = serde_yaml::from_str(&text).expect("valid study");
        assert_eq!(config.study.name, "mini_study");
        assert_eq!(config.study.arms.len(), 2);
        assert_eq!(config.study.replication.count, 2);
        assert_eq!(config.study.replication.strategy, SeedStrategy::Crn);
        assert_eq!(config.study.replication.base_seed, 42);
        assert_eq!(
            config.study.estimand, None,
            "absent estimand defaults to None"
        );
        let mut embedded: crate::config::ExperimentConfig =
            serde_yaml::from_str(MINI_BASE).expect("mini parses");
        embedded.experiment.replication = Some(ReplicationSpec {
            count: 3,
            strategy: SeedStrategy::Independent,
            base_seed: 42,
        });
        let embedded_text = serde_yaml::to_string(&embedded).expect("serialize");
        let back: crate::config::ExperimentConfig =
            serde_yaml::from_str(&embedded_text).expect("embedded replication re-parses");
        assert_eq!(back.experiment.replication.unwrap().count, 3);
        let round: StudyConfig = serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap())
            .expect("study re-parses");
        assert_eq!(round.study.name, config.study.name);
        assert_eq!(round.study.arms.len(), config.study.arms.len());
        let _ = std::fs::remove_file(&base);
    }
    #[test]
    fn declared_estimand_parses_and_defaults_to_none() {
        let spec: StudyConfig = serde_yaml::from_str(
            r#"
study:
  name: s
  base: experiments/base/standard.yaml
  arms: []
  replication:
    count: 2
    strategy: crn
    base_seed: 1
  estimand: mean_paired_difference
  metrics: []
  cohorts: []
  output:
    directory: results/
"#,
        )
        .expect("valid study");
        assert_eq!(
            spec.study.estimand.as_deref(),
            Some("mean_paired_difference")
        );
        assert!(crate::replicate::SeedStrategy::Crn == spec.study.replication.strategy);
    }
    #[test]
    fn arm_overrides_touch_only_their_leaf() {
        let base = temp_base();
        let spec: StudyConfig = serde_yaml::from_str(&study_yaml(&base)).unwrap();
        let base_config = inherit::load(Path::new(&spec.study.base)).expect("resolve base");
        let mut flat = base_config.clone();
        set_nested_value(
            &mut flat,
            "experiment.rating.systems.0.name",
            Value::String("flat".into()),
        );
        set_nested_value(
            &mut flat,
            "experiment.rating.systems.0.k_factor",
            Value::from(10.0),
        );
        assert_eq!(
            flat.experiment.rating.systems[0].name.as_deref(),
            Some("flat")
        );
        assert_eq!(
            flat.experiment.rating.systems[0].params["k_factor"],
            serde_yaml::Value::from(10.0)
        );
        assert_eq!(
            base_config.experiment.name, flat.experiment.name,
            "only the leaf changes"
        );
        assert_eq!(
            base_config.experiment.population.size,
            flat.experiment.population.size
        );
        assert_eq!(
            base_config.experiment.duration.matches,
            flat.experiment.duration.matches
        );
        let _ = std::fs::remove_file(&base);
    }
    #[test]
    fn seed_override_is_rejected() {
        let base = temp_base();
        let text = format!(
            r#"
study:
  name: mini_study
  base: {base}
  arms:
    - name: elo
    - name: bad
      overrides:
        experiment.seed: 999
  replication:
    count: 2
    strategy: crn
  metrics: [match_quality]
  cohorts: []
  output:
    directory: results/studies/mini/
"#
        );
        let config: StudyConfig = serde_yaml::from_str(&text).expect("parses");
        let err = StudyRunner::run(&config, 1).expect_err("seed override must be rejected");
        assert!(err.contains("seed"), "error names the seed: {err}");
        let _ = std::fs::remove_file(&base);
    }
    #[test]
    fn mini_study_runs_deterministically() {
        let base = temp_base();
        let config: StudyConfig = serde_yaml::from_str(&study_yaml(&base)).expect("valid study");
        let mut a = StudyRunner::run(&config, 1).expect("run study a");
        let mut b = StudyRunner::run(&config, 1).expect("run study b");
        assert_eq!(a.arms.len(), 2);
        let elo = &a.arms[0];
        assert_eq!(elo.name, "elo");
        assert_eq!(elo.replicates.len(), 2);
        let flat = &a.arms[1];
        assert_eq!(flat.replicates.len(), 2);
        assert_eq!(elo.replicates[0].seed, flat.replicates[0].seed);
        assert_eq!(elo.replicates[1].seed, flat.replicates[1].seed);
        assert_ne!(elo.replicates[0].seed, elo.replicates[1].seed);
        assert_eq!(
            a.arms[0].replicates[0].result.name,
            a.arms[1].replicates[0].result.name
        );
        for arm in a.arms.iter_mut().chain(b.arms.iter_mut()) {
            for repl in arm.replicates.iter_mut() {
                repl.result.timestamp.clear();
            }
        }
        assert_eq!(
            a, b,
            "identical study inputs must produce identical results"
        );
        assert!(
            a.study_id.starts_with("mini_study-"),
            "study_id: {}",
            a.study_id
        );
        assert!(a.study_id.ends_with("-crn-r2"), "study_id: {}", a.study_id);
        let _ = std::fs::remove_file(&base);
    }
}
