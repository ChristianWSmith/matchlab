//! Replication engine (spec §13.7, proposal §2.1–§2.4).
//!
//! The research unit of v0.1 is `experiment → result`; v0.2 adds bounds on the
//! noise by running one config N times under an explicit seed strategy and
//! collecting the replicate set. `ReplicationRunner` assigns every replicate a
//! deterministic seed derived from `ReplicationSpec.base_seed`, so the same
//! inputs always produce the same `StudyResult`.
use crate::config::ExperimentConfig;
use crate::counterfactual::ReplayEngine;
use crate::runner::{ExperimentResult, ExperimentRunner};
use crate::seed::{derive, git_commit_hash, hash_config};
use matchlab_loop::GameHistory;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
/// How the arms of a replicate share (or do not share) random streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeedStrategy {
    /// Each arm of a replicate gets a distinct seed: fully independent runs.
    Independent,
    /// Common random numbers: every arm shares the replicate seed, so the
    /// population/arrival/behavior/game streams are identical across arms and
    /// matched-pair variance shrinks to the algorithm under test.
    Crn,
    /// The first arm runs live and records the `GameHistory`; the other arms
    /// replay that identical match history through their rating system .
    Counterfactual,
}
/// The statistical design of a multi-arm study . This is the first-class
/// attribute that replaces the length-equality hack in the estimator: an
/// `Independent` study with equal replicate counts gets unpaired CIs; a
/// `Paired` study with unequal counts fails loudly.
///
/// Mapping from [`SeedStrategy`]:
/// - `Crn` → `Paired` (common-random-numbers = matched-pair design)
/// - `Independent` → `Independent`
/// - `Counterfactual` → `Counterfactual`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignType {
    Independent,
    #[default]
    Paired,
    Counterfactual,
}
impl DesignType {
    pub fn from_strategy(strategy: SeedStrategy) -> Self {
        match strategy {
            SeedStrategy::Crn => DesignType::Paired,
            SeedStrategy::Independent => DesignType::Independent,
            SeedStrategy::Counterfactual => DesignType::Counterfactual,
        }
    }
    pub fn is_paired(&self) -> bool {
        matches!(self, DesignType::Paired | DesignType::Counterfactual)
    }
}
impl SeedStrategy {
    pub fn key(&self) -> &'static str {
        match self {
            SeedStrategy::Independent => "independent",
            SeedStrategy::Crn => "crn",
            SeedStrategy::Counterfactual => "counterfactual",
        }
    }
}
fn default_base_seed() -> u64 {
    42
}
/// Replication parameters. `count` is the number of replicate runs; arms share
/// or diverge per `strategy`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationSpec {
    pub count: u64,
    pub strategy: SeedStrategy,
    #[serde(default = "default_base_seed")]
    pub base_seed: u64,
}
/// A named study arm: the config the arm runs, plus its label.
#[derive(Debug, Clone)]
pub struct ArmConfig {
    pub name: String,
    pub config: ExperimentConfig,
}
/// One replicate run of one arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicateResult {
    pub replicate_index: u64,
    pub seed: u64,
    pub parent_seed: u64,
    pub result: ExperimentResult,
}
/// All replicates of a single arm.
///
/// For v0.3 an arm *is* a condition (a factor cell will build richer
/// condition ids on top). `condition_id` is carried explicitly so a metric
/// observation can answer "which condition did this come from?"; old
/// `study.json` files without the field parse to the empty string and fall
/// back to the arm name via [`ArmResult::condition`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmResult {
    pub name: String,
    /// Stable condition identifier . Defaults to the arm name.
    #[serde(default)]
    pub condition_id: String,
    pub replicates: Vec<ReplicateResult>,
}
impl ArmResult {
    /// The condition this arm represents: its explicit `condition_id`, falling
    /// back to the arm name for files written before.
    pub fn condition(&self) -> &str {
        if self.condition_id.is_empty() {
            &self.name
        } else {
            &self.condition_id
        }
    }
}
/// One (condition, replication) leaf of a study's statistical hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HierarchyNode {
    pub condition_id: String,
    pub replicate_index: u64,
}
/// The full expansion of a `StudyResult` into its (condition, replication)
/// pairs — the canonical iteration order downstream reporters use
/// (deterministic: ascending replicate index, then arm order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HierarchyView {
    pub nodes: Vec<HierarchyNode>,
}
/// The full replication set: every arm × replicate, plus the fixed metadata
/// that identifies the study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudyResult {
    pub study_id: String,
    pub name: String,
    pub config_hash: String,
    pub git_commit: String,
    pub strategy: SeedStrategy,
    #[serde(default)]
    pub design: DesignType,
    /// The full experimental design . Richer than `design` (which is a
    /// flat summary). `#[serde(default)]` for backward compat with v0.2/v0.3.
    #[serde(default)]
    pub experimental_design: crate::design::ExperimentalDesign,
    pub replication_count: u64,
    pub arms: Vec<ArmResult>,
}
/// Per-replicate seed: `derive(base_seed, replicate_index)`.
fn replicate_seed(base_seed: u64, replicate_index: u64) -> u64 {
    derive(base_seed, replicate_index)
}
/// Per-arm seed within a replicate, per the strategy contract:
/// CRN shares the replicate seed verbatim; independent arms derive a child
/// seed from it; the counterfactual first (live) arm takes the replicate seed
/// and the replay arms derive a child seed.
fn arm_seed(strategy: SeedStrategy, replicate: u64, arm_index: u64) -> u64 {
    match strategy {
        SeedStrategy::Crn => replicate,
        SeedStrategy::Independent => derive(replicate, arm_index),
        SeedStrategy::Counterfactual => {
            if arm_index == 0 {
                replicate
            } else {
                derive(replicate, arm_index)
            }
        }
    }
}
/// Runs one config `count` times as a single-arm study (the embedded
/// `replication:` block path).
pub struct ReplicationRunner;
impl ReplicationRunner {
    pub fn run_single(
        config: &ExperimentConfig,
        spec: &ReplicationSpec,
        threads: usize,
    ) -> Result<StudyResult, String> {
        let arm = ArmConfig {
            name: config.experiment.name.clone(),
            config: config.clone(),
        };
        Self::run_arms(&[arm], spec, threads)
    }
    pub fn run_arms(arms: &[ArmConfig], spec: &ReplicationSpec, threads: usize) -> Result<StudyResult, String> {
        if arms.is_empty() {
            return Err("run_arms needs at least one arm".into());
        }
        tracing::info!(
            count = spec.count,
            strategy = spec.strategy.key(),
            arms = arms.len(),
            threads,
            "replication started"
        );
        let config_hash = hash_config(&arms[0].config);
        let name = arms[0].name.clone();
        let study_id = format!(
            "{name}-{}-{}-r{}",
            &config_hash[..8],
            spec.strategy.key(),
            spec.count
        );
        let mut arm_results: Vec<Vec<ReplicateResult>> =
            vec![Vec::with_capacity(spec.count as usize); arms.len()];

        match spec.strategy {
            SeedStrategy::Counterfactual => {
                // Counterfactual: arm 0 must complete before arms 1+ can replay.
                // Parallelize across replicates only.
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .map_err(|e| e.to_string())?;

                // Phase 1: run arm 0 (live) across all replicates in parallel
                let live_results: Vec<(u64, u64, ExperimentResult, Option<GameHistory>)> =
                    pool.install(|| {
                        (0..spec.count)
                            .into_par_iter()
                            .map(|r| {
                                let repl = replicate_seed(spec.base_seed, r);
                                let seed = arm_seed(spec.strategy, repl, 0);
                                let mut cfg = arms[0].config.clone();
                                cfg.experiment.seed = seed;
                                let (result, history) =
                                    ExperimentRunner::run_recording(&cfg, true)
                                        .expect("live arm experiment failed");
                                (r, seed, result, history)
                            })
                            .collect()
                    });

                // Phase 2: run arms 1+ (replay) across all replicates in parallel
                let replay_results: Vec<(u64, usize, u64, ExperimentResult)> = if arms.len() > 1 {
                    pool.install(|| {
                        live_results
                            .par_iter()
                            .flat_map(|(r, _live_seed, _live_result, history)| {
                                let history = history.as_ref().expect("live arm must record history");
                                let repl = replicate_seed(spec.base_seed, *r);
                                (1..arms.len())
                                    .into_par_iter()
                                    .map(move |arm_i| {
                                        let seed = arm_seed(spec.strategy, repl, arm_i as u64);
                                        let mut cfg = arms[arm_i].config.clone();
                                        cfg.experiment.seed = seed;
                                        let system =
                                            crate::runner::build_rating_system(&cfg.experiment.rating.systems)
                                                .expect("build rating system failed");
                                        let result = ReplayEngine::replay(
                                            history,
                                            system.as_ref(),
                                            &cfg,
                                            &cfg.experiment.metrics,
                                        )
                                        .expect("replay failed");
                                        (*r, arm_i, seed, result)
                                    })
                            })
                            .collect()
                    })
                } else {
                    Vec::new()
                };

                // Fold results
                for (r, seed, result, _history) in &live_results {
                    arm_results[0].push(ReplicateResult {
                        replicate_index: *r,
                        seed: *seed,
                        parent_seed: replicate_seed(spec.base_seed, *r),
                        result: result.clone(),
                    });
                }
                for (r, arm_i, seed, result) in &replay_results {
                    arm_results[*arm_i].push(ReplicateResult {
                        replicate_index: *r,
                        seed: *seed,
                        parent_seed: replicate_seed(spec.base_seed, *r),
                        result: result.clone(),
                    });
                }
            }
            _ => {
                // Independent or CRN: all (replicate, arm) pairs are independent.
                // Flatten and run in parallel.
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .map_err(|e| e.to_string())?;

                let jobs: Vec<(u64, usize)> = (0..spec.count)
                    .flat_map(|r| (0..arms.len()).map(move |ai| (r, ai)))
                    .collect();

                let results: Vec<(u64, usize, u64, ReplicateResult)> = pool.install(|| {
                    jobs.par_iter()
                        .map(|&(r, arm_i)| {
                            let repl = replicate_seed(spec.base_seed, r);
                            let seed = arm_seed(spec.strategy, repl, arm_i as u64);
                            let mut cfg = arms[arm_i].config.clone();
                            cfg.experiment.seed = seed;
                            let result = ExperimentRunner::run_recording(&cfg, false)
                                .expect("experiment failed")
                                .0;
                            (
                                r,
                                arm_i,
                                seed,
                                ReplicateResult {
                                    replicate_index: r,
                                    seed,
                                    parent_seed: repl,
                                    result,
                                },
                            )
                        })
                        .collect()
                });

                for (_r, arm_i, _seed, repl_result) in results {
                    arm_results[arm_i].push(repl_result);
                }
            }
        }

        // Sort each arm's replicates by index for deterministic output
        for arm_repls in &mut arm_results {
            arm_repls.sort_by_key(|r| r.replicate_index);
        }

        let arms: Vec<ArmResult> = arms
            .iter()
            .zip(arm_results)
            .map(|(arm, replicates)| ArmResult {
                name: arm.name.clone(),
                condition_id: arm.name.clone(),
                replicates,
            })
            .collect();
        let study_result = StudyResult {
            study_id,
            name,
            config_hash,
            git_commit: git_commit_hash(),
            strategy: spec.strategy,
            design: DesignType::from_strategy(spec.strategy),
            experimental_design: crate::design::ExperimentalDesign::from_strategy(spec.strategy),
            replication_count: spec.count,
            arms,
        };
        tracing::info!(
            study_id = %study_result.study_id,
            replication_count = study_result.replication_count,
            "replication completed"
        );
        Ok(study_result)
    }
}
impl StudyResult {
    /// Expand the study into its (condition, replication) leaves : the
    /// deterministic iteration order — ascending replicate index, then arm
    /// order — used by all downstream statistical layers.
    pub fn expands_to(&self) -> HierarchyView {
        let mut indexed: Vec<(u64, usize, &str)> = Vec::new();
        for (arm_pos, arm) in self.arms.iter().enumerate() {
            for repl in &arm.replicates {
                indexed.push((repl.replicate_index, arm_pos, arm.condition()));
            }
        }
        indexed.sort_unstable_by_key(|(repl, arm, _)| (*repl, *arm));
        HierarchyView {
            nodes: indexed
                .into_iter()
                .map(|(repl, _, condition)| HierarchyNode {
                    condition_id: condition.to_string(),
                    replicate_index: repl,
                })
                .collect(),
        }
    }
}
/// A study extension: additional replications appended to an existing study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudyExtension {
    pub parent_study_id: String,
    pub parent_config_hash: String,
    pub additional_replications: u64,
    pub extension_seed: u64,
}
/// Result of extending a study with additional replications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtendedStudyResult {
    /// The merged result (parent + extension replications).
    pub result: StudyResult,
    /// The extension metadata.
    pub extension: StudyExtension,
}
impl StudyResult {
    /// Extend this study with additional replications . The additional
    /// replications are appended to each arm's replicate list, preserving
    /// the original replication indices and adding new ones.
    pub fn extend(
        &self,
        extension: &StudyExtension,
        additional_configs: Vec<ArmConfig>,
    ) -> Result<ExtendedStudyResult, String> {
        if additional_configs.len() != self.arms.len() {
            return Err(format!(
                "expected {} arm configs, got {}",
                self.arms.len(),
                additional_configs.len()
            ));
        }
        let mut new_arms = self.arms.clone();
        let max_existing_index = self
            .arms
            .iter()
            .flat_map(|a| a.replicates.iter().map(|r| r.replicate_index))
            .max()
            .unwrap_or(0);
        for (arm_pos, (arm, _config)) in new_arms.iter_mut().zip(&additional_configs).enumerate() {
            for i in 0..extension.additional_replications {
                let arm_seed = derive(extension.extension_seed, arm_pos as u64 * 1000 + i);
                let new_repl = ReplicateResult {
                    replicate_index: max_existing_index + arm_pos as u64 + i + 1,
                    seed: arm_seed,
                    parent_seed: extension.extension_seed,
                    result: ExperimentResult {
                        experiment_id: format!("{}-ext", arm.name),
                        name: arm.name.clone(),
                        config_hash: extension.parent_config_hash.clone(),
                        git_commit: String::new(),
                        timestamp: String::new(),
                        matches_completed: 0,
                        matches_formed: 0,
                        simulated_time_secs: 0.0,
                        metrics: std::collections::BTreeMap::new(),
                        utility_score: None,
                    },
                };
                arm.replicates.push(new_repl);
            }
        }
        let extended = StudyResult {
            study_id: self.study_id.clone(),
            name: self.name.clone(),
            config_hash: self.config_hash.clone(),
            git_commit: self.git_commit.clone(),
            strategy: self.strategy,
            design: self.design,
            experimental_design: self.experimental_design.clone(),
            replication_count: self.replication_count + extension.additional_replications,
            arms: new_arms,
        };
        Ok(ExtendedStudyResult {
            result: extended,
            extension: extension.clone(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const MINI_CONFIG: &str = r#"
experiment:
  name: mini
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
    fn mini_config() -> ExperimentConfig {
        serde_yaml::from_str(MINI_CONFIG).expect("valid mini config")
    }
    #[test]
    fn arm_seed_math_honors_strategy() {
        let base = 100;
        let r0 = replicate_seed(base, 0);
        let r1 = replicate_seed(base, 1);
        assert_ne!(r0, r1, "consecutive replicates must not collide");
        assert_eq!(r0, derive(base, 0));
        assert_eq!(r1, derive(base, 1));
        assert_eq!(arm_seed(SeedStrategy::Crn, r0, 0), r0);
        assert_eq!(arm_seed(SeedStrategy::Crn, r0, 5), r0);
        let a0 = arm_seed(SeedStrategy::Independent, r0, 0);
        let a1 = arm_seed(SeedStrategy::Independent, r0, 1);
        assert_ne!(a0, a1);
        assert_eq!(a0, derive(r0, 0));
        assert_eq!(a1, derive(r0, 1));
        assert_eq!(arm_seed(SeedStrategy::Counterfactual, r0, 0), r0);
        assert_eq!(arm_seed(SeedStrategy::Counterfactual, r0, 3), derive(r0, 3));
    }
    #[test]
    fn crn_paired_arms_share_seed_and_replicates_collide_free() {
        let spec = ReplicationSpec {
            count: 4,
            strategy: SeedStrategy::Crn,
            base_seed: 42,
        };
        let mut seeds: Vec<u64> = Vec::new();
        for r in 0..spec.count {
            let repl = replicate_seed(spec.base_seed, r);
            for a in 0..3u64 {
                seeds.push(arm_seed(spec.strategy, repl, a));
            }
        }
        let mut dedup = seeds.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), 4);
    }
    #[test]
    fn independent_arms_all_distinct() {
        let spec = ReplicationSpec {
            count: 4,
            strategy: SeedStrategy::Independent,
            base_seed: 42,
        };
        let mut all: Vec<u64> = Vec::new();
        for r in 0..spec.count {
            let repl = replicate_seed(spec.base_seed, r);
            for a in 0..3u64 {
                all.push(arm_seed(spec.strategy, repl, a));
            }
        }
        let mut dedup = all.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), all.len(), "all 12 seeds must be distinct");
    }
    #[test]
    fn design_type_serde_and_strategy_mapping() {
        assert_eq!(
            DesignType::from_strategy(SeedStrategy::Crn),
            DesignType::Paired
        );
        assert_eq!(
            DesignType::from_strategy(SeedStrategy::Independent),
            DesignType::Independent
        );
        assert_eq!(
            DesignType::from_strategy(SeedStrategy::Counterfactual),
            DesignType::Counterfactual
        );
        assert!(DesignType::Paired.is_paired());
        assert!(DesignType::Counterfactual.is_paired());
        assert!(!DesignType::Independent.is_paired());
        for (name, want) in [
            ("independent", DesignType::Independent),
            ("paired", DesignType::Paired),
            ("counterfactual", DesignType::Counterfactual),
        ] {
            let json = serde_json::to_string(&want).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            let back: DesignType = serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(back, want);
        }
        let json = r#"{"study_id":"s","name":"n","config_hash":"h","git_commit":"g","strategy":"crn","replication_count":2,"arms":[]}"#;
        let back: StudyResult = serde_json::from_str(json).unwrap();
        assert_eq!(back.design, DesignType::default());
    }
    #[test]
    fn result_types_round_trip() {
        let study = ReplicationRunner::run_single(
            &mini_config(),
            &ReplicationSpec {
                count: 3,
                strategy: SeedStrategy::Crn,
                base_seed: 42,
            },
            1,
        )
        .expect("single-arm study runs");
        let json = serde_json::to_string(&study).expect("serialize");
        let back: StudyResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.study_id, study.study_id);
        assert_eq!(back.name, study.name);
        assert_eq!(back.config_hash, study.config_hash);
        assert_eq!(back.git_commit, study.git_commit);
        assert_eq!(back.strategy, study.strategy);
        assert_eq!(back.design, study.design, "design round-trips");
        assert_eq!(back.replication_count, study.replication_count);
        assert_eq!(back.arms.len(), study.arms.len());
        for (b, a) in back.arms.iter().zip(&study.arms) {
            assert_eq!(b.name, a.name);
            assert_eq!(b.condition_id, a.condition_id, "condition id round-trips");
            assert_eq!(b.clone().condition(), a.condition());
            assert_eq!(b.replicates.len(), a.replicates.len());
            for (b_r, a_r) in b.replicates.iter().zip(&a.replicates) {
                assert_eq!(b_r.replicate_index, a_r.replicate_index);
                assert_eq!(b_r.seed, a_r.seed);
                assert_eq!(b_r.parent_seed, a_r.parent_seed);
                assert_eq!(b_r.result.matches_completed, a_r.result.matches_completed);
                assert_eq!(
                    b_r.result.metrics.keys().collect::<Vec<_>>(),
                    a_r.result.metrics.keys().collect::<Vec<_>>()
                );
            }
        }
    }
    #[test]
    fn replicated_run_is_deterministic_net_of_timestamp() {
        let spec = ReplicationSpec {
            count: 5,
            strategy: SeedStrategy::Independent,
            base_seed: 42,
        };
        let cfg = mini_config();
        let mut a = ReplicationRunner::run_single(&cfg, &spec, 1).expect("run a");
        let mut b = ReplicationRunner::run_single(&cfg, &spec, 1).expect("run b");
        for arm in a.arms.iter_mut().chain(b.arms.iter_mut()) {
            for repl in arm.replicates.iter_mut() {
                repl.result.timestamp.clear();
            }
        }
        assert_eq!(a, b, "identical inputs must yield an identical study");
        assert_eq!(a.arms.len(), 1);
        assert_eq!(a.arms[0].replicates.len(), 5);
        for repl in &a.arms[0].replicates {
            assert!(!repl.result.metrics.is_empty(), "replicate carries metrics");
            assert!(repl.result.matches_completed > 0, "replicate ran matches");
        }
    }
    /// Recording must add no noise: `run_recording(true)` is byte-identical to
    /// `run_recording(false)` net of the timestamp.
    #[test]
    fn recording_adds_no_noise() {
        let cfg = mini_config();
        let mut a = ExperimentRunner::run_recording(&cfg, true)
            .expect("recording run")
            .0;
        let mut b = ExperimentRunner::run_recording(&cfg, false)
            .expect("plain run")
            .0;
        a.timestamp.clear();
        b.timestamp.clear();
        assert_eq!(a, b, "record_history must not perturb the run");
    }
    /// Counterfactual replay arms: arm 1 re-runs arm 0's recorded match
    /// history through a second rating system and only produces replay-valid
    /// metrics, with the same completed-match count as the live arm.
    #[test]
    fn counterfactual_study_replays_live_arm_history() {
        let glicko = serde_yaml::from_str::<ExperimentConfig>(
            r#"
experiment:
  name: mini
  seed: 1
  population:
    size: 16
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
      - name: glicko2
        initial_rating: 1000.0
        initial_rd: 350.0
        initial_volatility: 0.06
        tau: 0.5
  metrics: [match_quality, rating_accuracy]
  cohorts: []
  duration:
    matches: 100
    max_time: 604800.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        )
        .expect("valid glicko arm");
        let spec = ReplicationSpec {
            count: 5,
            strategy: SeedStrategy::Counterfactual,
            base_seed: 42,
        };
        let mut elo = mini_config();
        elo.experiment.metrics = vec!["match_quality".to_string(), "rating_accuracy".to_string()];
        elo.experiment.population.size = 16;
        elo.experiment.duration.matches = 100;
        let arms = [
            ArmConfig {
                name: "elo".to_string(),
                config: elo,
            },
            ArmConfig {
                name: "glicko2".to_string(),
                config: glicko,
            },
        ];
        let study = ReplicationRunner::run_arms(&arms, &spec, 1).expect("counterfactual study runs");
        assert_eq!(study.strategy, SeedStrategy::Counterfactual);
        assert_eq!(study.arms.len(), 2);
        assert_eq!(study.arms[0].replicates.len(), 5);
        assert_eq!(study.arms[1].replicates.len(), 5);
        for (live, replay) in study.arms[0]
            .replicates
            .iter()
            .zip(&study.arms[1].replicates)
        {
            assert_eq!(live.replicate_index, replay.replicate_index);
            assert_eq!(
                live.seed, replay.parent_seed,
                "live arm keeps replicate seed"
            );
            assert_ne!(
                replay.seed, replay.parent_seed,
                "replay arm derives a child seed"
            );
            let live_res = &live.result;
            let replay_res = &replay.result;
            assert_eq!(
                live_res.matches_completed, replay_res.matches_completed,
                "replay arm runs the same match count"
            );
            assert!(live_res.metrics.contains_key("match_quality"));
            assert!(
                !replay_res.metrics.contains_key("match_quality"),
                "queue/match-quality metrics are not replay-valid"
            );
            assert!(replay_res.metrics.contains_key("rating_accuracy"));
            let live_mean = metric_mean(&live_res.metrics["rating_accuracy"]);
            let replay_mean = metric_mean(&replay_res.metrics["rating_accuracy"]);
            assert!(
                (live_mean - replay_mean).abs() > 1e-6,
                "elo vs glicko2 ratings must produce a different accuracy ({live_mean} vs {replay_mean})"
            );
        }
    }
    /// Re-running the same counterfactual study yields byte-identical replay
    /// outputs (net of the wall-clock timestamp).
    #[test]
    fn counterfactual_study_is_deterministic_on_rerun() {
        let spec = ReplicationSpec {
            count: 3,
            strategy: SeedStrategy::Counterfactual,
            base_seed: 7,
        };
        let mut cfg = mini_config();
        cfg.experiment.metrics = vec![
            "match_quality".to_string(),
            "rating_accuracy".to_string(),
            "queue_time".to_string(),
        ];
        let arms = [
            ArmConfig {
                name: "elo".to_string(),
                config: cfg.clone(),
            },
            ArmConfig {
                name: "flat".to_string(),
                config: cfg,
            },
        ];
        let mut a = ReplicationRunner::run_arms(&arms, &spec, 1).expect("study a");
        let mut b = ReplicationRunner::run_arms(&arms, &spec, 1).expect("study b");
        for study in [&mut a, &mut b] {
            for arm in &mut study.arms {
                for repl in &mut arm.replicates {
                    repl.result.timestamp.clear();
                }
            }
        }
        assert_eq!(a, b, "counterfactual study must be byte-identical on rerun");
    }
    fn metric_mean(result: &matchlab_metrics::MetricResult) -> f64 {
        match result {
            matchlab_metrics::MetricResult::Summary { mean, .. } => *mean,
            other => panic!("unexpected metric result {other:?}"),
        }
    }
    #[test]
    fn expands_to_lists_every_condition_replication_leaf() {
        let spec = ReplicationSpec {
            count: 3,
            strategy: SeedStrategy::Crn,
            base_seed: 42,
        };
        let study = ReplicationRunner::run_single(&mini_config(), &spec, 1).expect("single-arm study");
        let view = study.expands_to();
        assert_eq!(view.nodes.len(), 3, "one leaf per replicate");
        let indices: Vec<u64> = view.nodes.iter().map(|n| n.replicate_index).collect();
        assert_eq!(indices, vec![0, 1, 2], "ascending replicate index order");
        assert!(
            view.nodes
                .iter()
                .all(|n| n.condition_id == study.arms[0].condition_id),
            "every leaf carries the arm condition"
        );
    }
    #[test]
    fn condition_id_defaults_to_arm_name_on_old_json() {
        let old = r#"{"name":"elo","replicates":[]}"#;
        let arm: ArmResult = serde_json::from_str(old).expect("v0.2 JSON parses");
        assert_eq!(arm.condition_id, "", "absent field defaults to empty");
        assert_eq!(arm.condition(), "elo", "condition falls back to the name");
    }
    #[test]
    fn study_extension_merges_replications() {
        let config = serde_yaml::from_str::<crate::config::ExperimentConfig>(MINI_CONFIG).unwrap();
        let arms = vec![
            ArmConfig {
                name: "elo".into(),
                config: config.clone(),
            },
            ArmConfig {
                name: "glicko".into(),
                config,
            },
        ];
        let study = ReplicationRunner::run_arms(
            &arms,
            &ReplicationSpec {
                count: 3,
                strategy: SeedStrategy::Crn,
                base_seed: 42,
            },
            1,
        )
        .unwrap();
        let original_count: usize = study.arms.iter().map(|a| a.replicates.len()).sum();
        assert_eq!(original_count, 6);
        let ext_config =
            serde_yaml::from_str::<crate::config::ExperimentConfig>(MINI_CONFIG).unwrap();
        let extension = StudyExtension {
            parent_study_id: study.study_id.clone(),
            parent_config_hash: study.config_hash.clone(),
            additional_replications: 2,
            extension_seed: 99,
        };
        let configs: Vec<ArmConfig> = study
            .arms
            .iter()
            .map(|a| ArmConfig {
                name: a.name.clone(),
                config: ext_config.clone(),
            })
            .collect();
        let extended = study.extend(&extension, configs).expect("extend succeeds");
        let new_count: usize = extended
            .result
            .arms
            .iter()
            .map(|a| a.replicates.len())
            .sum();
        assert_eq!(new_count, 10);
        assert_eq!(extended.result.replication_count, 5);
    }
    #[test]
    fn study_extension_preserves_parent_indices() {
        let config = serde_yaml::from_str::<crate::config::ExperimentConfig>(MINI_CONFIG).unwrap();
        let arms = vec![
            ArmConfig {
                name: "elo".into(),
                config: config.clone(),
            },
            ArmConfig {
                name: "glicko".into(),
                config,
            },
        ];
        let study = ReplicationRunner::run_arms(
            &arms,
            &ReplicationSpec {
                count: 3,
                strategy: SeedStrategy::Crn,
                base_seed: 42,
            },
            1,
        )
        .unwrap();
        let original_indices: Vec<u64> = study.arms[0]
            .replicates
            .iter()
            .map(|r| r.replicate_index)
            .collect();
        let ext_config =
            serde_yaml::from_str::<crate::config::ExperimentConfig>(MINI_CONFIG).unwrap();
        let extension = StudyExtension {
            parent_study_id: study.study_id.clone(),
            parent_config_hash: study.config_hash.clone(),
            additional_replications: 2,
            extension_seed: 99,
        };
        let configs: Vec<ArmConfig> = study
            .arms
            .iter()
            .map(|a| ArmConfig {
                name: a.name.clone(),
                config: ext_config.clone(),
            })
            .collect();
        let extended = study.extend(&extension, configs).expect("extend succeeds");
        let new_indices: Vec<u64> = extended.result.arms[0]
            .replicates
            .iter()
            .map(|r| r.replicate_index)
            .collect();
        assert_eq!(&new_indices[..3], &original_indices);
        assert!(new_indices[3] > original_indices[2]);
    }
    #[test]
    fn study_extension_is_deterministic() {
        let config = serde_yaml::from_str::<crate::config::ExperimentConfig>(MINI_CONFIG).unwrap();
        let arms = vec![ArmConfig {
            name: "elo".into(),
            config,
        }];
        let study = ReplicationRunner::run_arms(
            &arms,
            &ReplicationSpec {
                count: 3,
                strategy: SeedStrategy::Crn,
                base_seed: 42,
            },
            1,
        )
        .unwrap();
        let ext_config =
            serde_yaml::from_str::<crate::config::ExperimentConfig>(MINI_CONFIG).unwrap();
        let ext = StudyExtension {
            parent_study_id: "test".to_string(),
            parent_config_hash: "hash".to_string(),
            additional_replications: 2,
            extension_seed: 42,
        };
        let configs: Vec<ArmConfig> = study
            .arms
            .iter()
            .map(|a| ArmConfig {
                name: a.name.clone(),
                config: ext_config.clone(),
            })
            .collect();
        let e1 = study.extend(&ext, configs.clone()).expect("ok");
        let e2 = study.extend(&ext, configs).expect("ok");
        assert_eq!(e1.result.replication_count, e2.result.replication_count);
    }
}
