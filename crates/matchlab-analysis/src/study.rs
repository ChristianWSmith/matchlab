//! Study report + export (spec §14.8,).
//!
//! Renders a `StudyResult` () plus its aggregate statistics
//! into the human-readable v0.2 deliverable and wires the export path for
//! `matchlab study` / embedded-`replication:` runs.
//!
//! Everything is deterministic: the bootstrap seeds below derive from the
//! report config's study seed, so a fixed `(StudyResult, StudyReportConfig)`
//! pair always renders the same numbers.
use crate::aggregation::build_replication_table;
use crate::effect::effect_sizes;
use crate::hierarchy::ReplicationScalar;
use matchlab_experiments::StudyResult;
use matchlab_experiments::seed::derive;
use std::collections::BTreeSet;
use tracing;
/// Report knobs. `conf` is the CI coverage (default 0.95); `seed` seeds the
/// bootstrap streams deterministically.
#[derive(Debug, Clone)]
pub struct StudyReportConfig {
    pub conf: f64,
    pub seed: u64,
}
impl Default for StudyReportConfig {
    fn default() -> Self {
        Self {
            conf: 0.95,
            seed: 0,
        }
    }
}
/// Per-arm aggregate of one metric across its replicates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArmMetricStat {
    pub metric: String,
    pub arm: String,
    pub n: usize,
    pub mean: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
    pub ci_method: crate::effect::CiMethod,
}
/// Effect size of one arm pair on one metric .
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PairEffect {
    pub metric: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub mean_delta: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
    pub cohen_d: f64,
    pub relative_diff: f64,
    pub percent_improvement: f64,
    pub ci_method: crate::effect::CiMethod,
}
/// Machine-readable aggregate statistics (the JSON report).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyStats {
    pub name: String,
    pub study_id: String,
    pub config_hash: String,
    pub git_commit: String,
    pub strategy: String,
    #[serde(default = "default_design")]
    pub design: matchlab_experiments::DesignType,
    /// The full experimental design .
    #[serde(default)]
    pub experimental_design: matchlab_experiments::ExperimentalDesign,
    pub replication_count: u64,
    pub engine_version: String,
    pub metrics: BTreeSet<String>,
    pub arms: Vec<ArmMetricStat>,
    pub effects: Vec<PairEffect>,
}
fn default_design() -> matchlab_experiments::DesignType {
    matchlab_experiments::DesignType::default()
}
fn mean_of(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
/// Compute the aggregate statistics for a study. The table is built once and
/// consumed directly — the statistical unit is the replication . The
/// metric discovery gate uses [`crate::aggregation::replication_scalar`], which
/// extends v0.2's [`extract_scalar`](crate::effect::extract_scalar) to include
/// `Distribution` metrics (mean-of-sample). The `StudyStats` output is
/// byte-identical for existing fixtures where all metrics are `Summary`/`Scalar`
/// (regressions hold).
pub fn compute_study_stats(study: &StudyResult, cfg: &StudyReportConfig) -> StudyStats {
    tracing::info!(arms = study.arms.len(), "computing study statistics");
    let table = build_replication_table(study);
    let mut metrics = BTreeSet::new();
    for ((_, metric), obs) in &table {
        if obs.iter().any(|o| o.replication_scalar().is_some()) {
            metrics.insert(metric.clone());
        }
    }
    let mut arms_stats = Vec::new();
    let mut effects = Vec::new();
    let n_arms = study.arms.len();
    for (metric_index, metric) in metrics.iter().enumerate() {
        let metric_seed = derive(cfg.seed, metric_index as u64);
        let per_arm: Vec<Vec<ReplicationScalar>> = study
            .arms
            .iter()
            .map(|arm| {
                let key = (arm.condition_id.clone(), metric.clone());
                table
                    .get(&key)
                    .map(|col| col.iter().filter_map(|o| o.replication_scalar()).collect())
                    .unwrap_or_default()
            })
            .collect();
        for (arm_index, arm) in study.arms.iter().enumerate() {
            let scalars = &per_arm[arm_index];
            if scalars.is_empty() {
                continue;
            }
            let values: Vec<f64> = scalars.iter().map(ReplicationScalar::value).collect();
            let arm_ci = if values.len() >= 30 {
                crate::effect::student_t_ci(&values, cfg.conf)
            } else {
                crate::effect::bootstrap_ci(
                    &values,
                    cfg.conf,
                    derive(metric_seed, arm_index as u64),
                )
            };
            arms_stats.push(ArmMetricStat {
                metric: metric.clone(),
                arm: arm.name.clone(),
                n: scalars.len(),
                mean: mean_of(&values),
                ci_lo: arm_ci.lower,
                ci_hi: arm_ci.upper,
                ci_method: arm_ci.method,
            });
        }
        for i in 0..n_arms {
            for j in (i + 1)..n_arms {
                let control = &per_arm[i];
                let treatment = &per_arm[j];
                if control.is_empty() || treatment.is_empty() {
                    continue;
                }
                let seed = derive(metric_seed, (n_arms + j) as u64);
                if let Ok(es) =
                    effect_sizes(control, treatment, study.design.is_paired(), cfg.conf, seed)
                {
                    effects.push(PairEffect {
                        metric: metric.clone(),
                        control_arm: study.arms[i].name.clone(),
                        treatment_arm: study.arms[j].name.clone(),
                        mean_delta: es.mean_delta,
                        ci_lo: es.ci_lo,
                        ci_hi: es.ci_hi,
                        cohen_d: es.cohen_d,
                        relative_diff: es.relative_diff,
                        percent_improvement: es.percent_improvement,
                        ci_method: es.ci_method,
                    });
                }
            }
        }
    }
    StudyStats {
        name: study.name.clone(),
        study_id: study.study_id.clone(),
        config_hash: study.config_hash.clone(),
        git_commit: study.git_commit.clone(),
        strategy: study.strategy.key().to_string(),
        design: study.design,
        experimental_design: study.experimental_design.clone(),
        replication_count: study.replication_count,
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        metrics,
        arms: arms_stats,
        effects,
    }
}
/// Markdown study report in the exit-criterion shape (spec §14.8). Enabled
/// metrics only (scalarizable per).
pub fn generate_study_report(study: &StudyResult, cfg: &StudyReportConfig) -> String {
    let stats = compute_study_stats(study, cfg);
    let mut out = String::new();
    out.push_str(&format!("# {} — study\n\n", study.name));
    out.push_str(&format!(
        "Replications: N = {} ({})\n\n",
        stats.replication_count, stats.strategy
    ));
    out.push_str(&format!(
        "Config: `{}` · Git: {} · engine: {}\n\n",
        stats.config_hash, stats.git_commit, stats.engine_version
    ));
    for metric in &stats.metrics {
        out.push_str(&format!("## {metric}\n\n"));
        out.push_str("| arm | mean | 95% CI |\n");
        out.push_str("|-----|------|--------|\n");
        for arm in stats.arms.iter().filter(|a| &a.metric == metric) {
            out.push_str(&format!(
                "| {} | {:.2} | [{:.2}, {:.2}] |\n",
                arm.arm, arm.mean, arm.ci_lo, arm.ci_hi
            ));
        }
        out.push('\n');
        for eff in stats.effects.iter().filter(|e| &e.metric == metric) {
            out.push_str(&format!(
                "### {} vs {}\n",
                eff.control_arm, eff.treatment_arm
            ));
            out.push_str(&format!(
                "Δ = {:+.2}  95% CI [{:.2}, {:.2}]   Cohen's d = {:.2}\n",
                eff.mean_delta, eff.ci_lo, eff.ci_hi, eff.cohen_d
            ));
            out.push_str(&format!(
                "relative Δ = {:+.1}%   percent improvement = {:+.1}%\n\n",
                eff.relative_diff * 100.0,
                eff.percent_improvement
            ));
        }
    }
    out
}
/// The JSON report: the aggregate table plus per-arm distributions.
pub fn generate_study_report_json(study: &StudyResult, cfg: &StudyReportConfig) -> String {
    let stats = compute_study_stats(study, cfg);
    serde_json::to_string_pretty(&stats).expect("study stats serialize")
}
/// Write the full `StudyResult` as `<study_id>.json` plus the aggregate
/// `study_stats.json` under `directory`.
pub fn write_study_result_json(study: &StudyResult, directory: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let study_path = std::path::Path::new(directory).join(format!("{}.json", study.study_id));
    std::fs::write(
        &study_path,
        serde_json::to_string_pretty(study).map_err(std::io::Error::other)?,
    )?;
    let stats_path = std::path::Path::new(directory).join("study_stats.json");
    std::fs::write(
        &stats_path,
        generate_study_report_json(
            study,
            &StudyReportConfig {
                seed: 0,
                ..StudyReportConfig::default()
            },
        ),
    )?;
    tracing::info!(directory, "study result written");
    Ok(())
}
/// Sectioned research report (ticket, v2): structured Markdown with
/// Study, Design, Conditions, Primary outcomes, Effect estimates, Uncertainty,
/// and Reproducibility sections. Distinguishes descriptive and inferential
/// claims. The old `generate_study_report` is preserved for backward compat.
pub fn generate_research_report(study: &StudyResult, cfg: &StudyReportConfig) -> String {
    let stats = compute_study_stats(study, cfg);
    let mut out = String::new();
    out.push_str("## Study\n\n");
    out.push_str(&format!("- **Name:** {}\n", study.name));
    out.push_str(&format!("- **ID:** `{}`\n", stats.study_id));
    out.push_str(&format!("- **Config:** `{}`\n", stats.config_hash));
    out.push_str(&format!("- **Git:** {}\n", stats.git_commit));
    out.push_str(&format!("- **Engine:** {}\n", stats.engine_version));
    out.push_str("\n## Design\n\n");
    out.push_str(&format!("- **Type:** {:?}\n", stats.design));
    out.push_str(&format!("- **Strategy:** {}\n", stats.strategy));
    out.push_str(&format!(
        "- **Replications:** {}\n",
        stats.replication_count
    ));
    out.push_str("\n> No causal claim is made. This is an observational/simulated comparison.\n");
    out.push_str("\n## Conditions\n\n");
    out.push_str("| Arm | N | Mean | CI |\n");
    out.push_str("|-----|---|------|----|\n");
    for arm in &stats.arms {
        out.push_str(&format!(
            "| {} | {} | {:.4} | [{:.4}, {:.4}] |\n",
            arm.arm, arm.n, arm.mean, arm.ci_lo, arm.ci_hi
        ));
    }
    out.push_str("\n## Primary Outcomes (Descriptive)\n\n");
    out.push_str("Per-arm summary statistics at the replication level.\n\n");
    for arm in &stats.arms {
        out.push_str(&format!(
            "- **{}**: n={}, mean={:.4}, 95% CI [{:.4}, {:.4}]\n",
            arm.arm, arm.n, arm.mean, arm.ci_lo, arm.ci_hi
        ));
    }
    out.push_str("\n## Effect Estimates (Inferential)\n\n");
    if stats.effects.is_empty() {
        out.push_str("No pairwise effects computed.\n");
    } else {
        for eff in &stats.effects {
            out.push_str(&format!(
                "### {} — {}\n\n",
                eff.control_arm, eff.treatment_arm
            ));
            out.push_str(&format!(
                "- **Δ** = {:.4}  95% CI [{:.4}, {:.4}]\n",
                eff.mean_delta, eff.ci_lo, eff.ci_hi
            ));
            out.push_str(&format!(
                "- **Relative:** {:.2}%\n",
                eff.relative_diff * 100.0
            ));
            out.push_str(&format!("- **Cohen's d:** {:.4}\n", eff.cohen_d));
            out.push_str(&format!("- **Method:** {:?}\n", eff.ci_method));
            out.push('\n');
        }
    }
    out.push_str("\n## Reproducibility\n\n");
    out.push_str(&format!(
        "Config hash: `{}` · Git: `{}` · Engine: {}\n",
        stats.config_hash, stats.git_commit, stats.engine_version
    ));
    out.push_str(&format!(
        "Strategy: {} · Replications: {}\n",
        stats.strategy, stats.replication_count
    ));
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_experiments::{
        ArmResult, ExperimentResult, ReplicateResult, SeedStrategy, StudyResult,
    };
    use matchlab_metrics::MetricResult;
    fn summary_result(name: &str, values: &[f64]) -> ExperimentResult {
        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "rating_accuracy".to_string(),
            MetricResult::Summary {
                mean: mean_of(values),
                median: 0.0,
                p75: 0.0,
                p90: 0.0,
                p95: 0.0,
                p99: 0.0,
                stddev: 0.0,
            },
        );
        ExperimentResult {
            experiment_id: format!("{name}-x"),
            name: name.to_string(),
            config_hash: "hash".to_string(),
            git_commit: "commit".to_string(),
            timestamp: "t".to_string(),
            matches_completed: 10,
            matches_formed: 10,
            simulated_time_secs: 100.0,
            metrics,
            utility_score: None,
        }
    }
    fn arm(name: &str, rows: &[f64]) -> ArmResult {
        ArmResult {
            name: name.to_string(),
            condition_id: name.to_string(),
            replicates: rows
                .iter()
                .enumerate()
                .map(|(i, v)| ReplicateResult {
                    replicate_index: i as u64,
                    seed: i as u64,
                    parent_seed: 0,
                    result: summary_result(name, &[*v]),
                })
                .collect(),
        }
    }
    fn fixture() -> StudyResult {
        StudyResult {
            study_id: "elo-vs-glicko-abc-crn-r3".to_string(),
            name: "elo_vs_glicko".to_string(),
            config_hash: "deadbeef1234".to_string(),
            git_commit: "beefcafe".to_string(),
            strategy: SeedStrategy::Crn,
            design: matchlab_experiments::DesignType::Paired,
            experimental_design: matchlab_experiments::ExperimentalDesign::default(),
            replication_count: 3,
            arms: vec![
                arm("elo", &[157.0, 160.0, 159.0]),
                arm("glicko", &[144.0, 142.0, 145.0]),
            ],
        }
    }
    #[test]
    fn report_renders_the_exit_criterion_block() {
        let report = generate_study_report(&fixture(), &StudyReportConfig::default());
        assert!(report.contains("Replications: N = 3 (crn)"));
        assert!(report.contains("Config: `deadbeef1234` · Git: beefcafe"));
        assert!(report.contains("## rating_accuracy"));
        assert!(report.contains("| elo |"));
        assert!(report.contains("| glicko |"));
        assert!(report.contains("| elo | 158.67 |"), "elo mean pinned");
        assert!(report.contains("| glicko | 143.67 |"), "glicko mean pinned");
        assert!(report.contains("### elo vs glicko"));
        assert!(report.contains("Δ = -15.00  95% CI ["));
        assert!(report.contains("Cohen's d = "));
        assert!(report.contains("relative Δ = -9.5%"));
    }
    #[test]
    fn json_report_serializes_aggregates() {
        let json = generate_study_report_json(&fixture(), &StudyReportConfig::default());
        let stats: StudyStats = serde_json::from_str(&json).expect("JSON report round-trips");
        assert_eq!(stats.replication_count, 3);
        assert_eq!(stats.metrics.len(), 1);
        assert_eq!(stats.arms.len(), 2);
        assert_eq!(stats.effects.len(), 1);
        assert_eq!(stats.arms[0].arm, "elo");
        assert_eq!(stats.arms[1].arm, "glicko");
        for arm in &stats.arms {
            assert_eq!(arm.n, 3);
        }
        assert_eq!(stats.effects[0].control_arm, "elo");
        assert_eq!(stats.effects[0].treatment_arm, "glicko");
    }
    #[test]
    fn single_arm_study_has_no_pairwise_section() {
        let mut one = fixture();
        one.arms.truncate(1);
        one.replication_count = 3;
        let report = generate_study_report(&one, &StudyReportConfig::default());
        assert!(report.contains("Replications: N = 3 (crn)"));
        assert!(!report.contains(" vs glicko"));
        assert!(!report.contains("Δ = "));
    }
    #[test]
    fn distribution_metric_is_now_aggregated_in_study_stats() {
        let mut study = fixture();
        let mut result = summary_result("elo", &[1.0]);
        result.metrics.insert(
            "queue_time".to_string(),
            MetricResult::Distribution(vec![1.0, 2.0]),
        );
        study.arms[0].replicates[0].result = result;
        let stats = compute_study_stats(&study, &StudyReportConfig::default());
        assert_eq!(stats.metrics.len(), 2);
        assert!(stats.metrics.contains("rating_accuracy"));
        assert!(
            stats.metrics.contains("queue_time"),
            "Distribution metric now aggregated to scalar"
        );
        let qt_stat = stats
            .arms
            .iter()
            .find(|a| a.metric == "queue_time")
            .expect("queue_time arm stat present");
        assert!((qt_stat.mean - 1.5).abs() < 1e-12, "mean of [1.0, 2.0]");
        assert_eq!(qt_stat.n, 1, "only one replicate has queue_time");
    }
    #[test]
    fn write_study_result_is_deterministic_net_of_timestamps() {
        let dir_a = std::env::temp_dir().join("matchlab-study-a");
        let dir_b = std::env::temp_dir().join("matchlab-study-b");
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
        let a = fixture();
        let b = fixture();
        write_study_result_json(&a, dir_a.to_string_lossy().as_ref()).unwrap();
        write_study_result_json(&b, dir_b.to_string_lossy().as_ref()).unwrap();
        let sa = std::fs::read_to_string(dir_a.join("study_stats.json")).unwrap();
        let sb = std::fs::read_to_string(dir_b.join("study_stats.json")).unwrap();
        assert_eq!(sa, sb, "study_stats.json must be byte-identical");
        let mut ja: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir_a.join("elo-vs-glicko-abc-crn-r3.json")).unwrap(),
        )
        .unwrap();
        let mut jb: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir_b.join("elo-vs-glicko-abc-crn-r3.json")).unwrap(),
        )
        .unwrap();
        for j in [&mut ja, &mut jb] {
            for arm in j["arms"].as_array_mut().unwrap() {
                for repl in arm["replicates"].as_array_mut().unwrap() {
                    repl["result"]["timestamp"] = serde_json::Value::Null;
                }
            }
        }
        assert_eq!(ja, jb, "study.json identical net of timestamps");
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }
    #[test]
    fn report_is_deterministic_across_equivalent_studies() {
        let a = generate_study_report(&fixture(), &StudyReportConfig { conf: 0.9, seed: 5 });
        let b = generate_study_report(&fixture(), &StudyReportConfig { conf: 0.9, seed: 5 });
        assert_eq!(a, b);
    }
    #[test]
    fn ci_method_is_recorded_in_pair_effect() {
        let stats = compute_study_stats(&fixture(), &StudyReportConfig::default());
        let eff = stats
            .effects
            .iter()
            .find(|e| e.metric == "rating_accuracy")
            .expect("pairwise effect present");
        assert_eq!(
            eff.ci_method,
            crate::effect::CiMethod::PairedBootstrap,
            "CRN study produces paired bootstrap CI"
        );
    }
    #[test]
    fn ci_method_is_recorded_in_arm_stat() {
        let stats = compute_study_stats(&fixture(), &StudyReportConfig::default());
        for arm_stat in &stats.arms {
            assert_eq!(
                arm_stat.ci_method,
                crate::effect::CiMethod::Bootstrap,
                "n=3 replicates → bootstrap CI for arm stat"
            );
        }
    }
    #[test]
    fn pair_effect_has_no_utility_score() {
        let stats = compute_study_stats(&fixture(), &StudyReportConfig::default());
        for eff in &stats.effects {
            assert!(
                !eff.metric.contains("utility"),
                "utility_score must not appear in statistical effects"
            );
        }
        let json = serde_json::to_string(&stats).unwrap();
        assert!(
            !json.contains("utility_score"),
            "utility_score must not serialize into PairEffect JSON"
        );
    }
    #[test]
    fn research_report_has_required_sections() {
        let report = generate_research_report(&fixture(), &StudyReportConfig::default());
        assert!(report.contains("## Study"));
        assert!(report.contains("## Design"));
        assert!(report.contains("## Conditions"));
        assert!(report.contains("## Primary Outcomes"));
        assert!(report.contains("## Effect Estimates"));
        assert!(report.contains("## Reproducibility"));
        assert!(report.contains("No causal claim is made"));
    }
    #[test]
    fn research_report_is_deterministic() {
        let r1 = generate_research_report(&fixture(), &StudyReportConfig::default());
        let r2 = generate_research_report(&fixture(), &StudyReportConfig::default());
        assert_eq!(r1, r2);
    }
    #[test]
    fn old_report_still_works() {
        let report = generate_study_report(&fixture(), &StudyReportConfig::default());
        assert!(report.contains("Replications: N = 3 (crn)"));
        assert!(report.contains("## rating_accuracy"));
    }
}
