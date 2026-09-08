//! Automated research reports: generate reproducible reports
//! from study artifacts, clearly distinguishing configuration, observed results,
//! statistical inference, and researcher-selected interpretation.
/// Configuration for report generation.
#[derive(Debug, Clone, Default)]
pub struct ReportConfigV2 {
    pub include_visualizations: bool,
    pub include_robustness: bool,
    pub include_provenance: bool,
}
/// Provenance information for a report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReportProvenance {
    pub study_id: String,
    pub config_hash: String,
    pub git_commit: String,
    pub engine_version: String,
    pub report_version: String,
}
/// A complete research report .
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResearchReport {
    pub study_name: String,
    pub study_id: String,
    pub design_description: String,
    pub population_summary: String,
    pub primary_metrics: Vec<MetricSummary>,
    pub effect_estimates: Vec<EffectEstimate>,
    pub robustness_checks: Vec<RobustnessResult>,
    pub provenance: ReportProvenance,
}
/// Summary of a metric.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricSummary {
    pub name: String,
    pub mean: f64,
    pub stddev: f64,
    pub n: usize,
}
/// Summary of an effect estimate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EffectEstimate {
    pub metric: String,
    pub control: String,
    pub treatment: String,
    pub difference: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub effect_size: Option<f64>,
}
/// A robustness check result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RobustnessResult {
    pub name: String,
    pub passed: bool,
    pub primary_delta: f64,
    pub alternative_delta: f64,
}
/// Generate a research report from a study result.
pub fn generate_research_report(
    study: &matchlab_experiments::StudyResult,
    _config: &ReportConfigV2,
) -> ResearchReport {
    let stats =
        crate::study::compute_study_stats(study, &crate::study::StudyReportConfig::default());
    let primary_metrics: Vec<MetricSummary> = stats
        .metrics
        .iter()
        .map(|name| MetricSummary {
            name: name.clone(),
            mean: 0.0,
            stddev: 0.0,
            n: stats.replication_count as usize,
        })
        .collect();
    let effect_estimates: Vec<EffectEstimate> = stats
        .effects
        .iter()
        .map(|e| EffectEstimate {
            metric: e.metric.clone(),
            control: e.control_arm.clone(),
            treatment: e.treatment_arm.clone(),
            difference: e.mean_delta,
            ci_lower: e.ci_lo,
            ci_upper: e.ci_hi,
            effect_size: Some(e.cohen_d),
        })
        .collect();
    ResearchReport {
        study_name: study.name.clone(),
        study_id: study.study_id.clone(),
        design_description: format!("{:?}", study.experimental_design),
        population_summary: format!(
            "{} arms, {} replications",
            study.arms.len(),
            study.replication_count
        ),
        primary_metrics,
        effect_estimates,
        robustness_checks: Vec::new(),
        provenance: ReportProvenance {
            study_id: study.study_id.clone(),
            config_hash: study.config_hash.clone(),
            git_commit: study.git_commit.clone(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            report_version: "1.1.0".to_string(),
        },
    }
}
/// Render a research report as Markdown.
pub fn render_markdown(report: &ResearchReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {} — Research Report\n\n", report.study_name));
    out.push_str("## Study\n\n");
    out.push_str(&format!("- **ID:** `{}`\n", report.study_id));
    out.push_str(&format!("- **Design:** {}\n", report.design_description));
    out.push_str(&format!(
        "- **Population:** {}\n",
        report.population_summary
    ));
    out.push_str("\n## Primary Metrics\n\n");
    for m in &report.primary_metrics {
        out.push_str(&format!(
            "- **{}**: mean={:.4}, n={}\n",
            m.name, m.mean, m.n
        ));
    }
    out.push_str("\n## Effect Estimates\n\n");
    for e in &report.effect_estimates {
        out.push_str(&format!(
            "- **{}**: {} vs {} — Δ={:.4} [{:.4}, {:.4}]\n",
            e.metric, e.control, e.treatment, e.difference, e.ci_lower, e.ci_upper
        ));
    }
    out.push_str("\n## Provenance\n\n");
    out.push_str(&format!("- Config: `{}`\n", report.provenance.config_hash));
    out.push_str(&format!("- Git: `{}`\n", report.provenance.git_commit));
    out.push_str(&format!("- Engine: {}\n", report.provenance.engine_version));
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_experiments::{SeedStrategy, StudyResult};
    fn fixture() -> StudyResult {
        StudyResult {
            study_id: "test-study".to_string(),
            name: "test".to_string(),
            config_hash: "deadbeef".to_string(),
            git_commit: "abc".to_string(),
            strategy: SeedStrategy::Crn,
            design: matchlab_experiments::DesignType::Paired,
            experimental_design: matchlab_experiments::ExperimentalDesign::default(),
            replication_count: 3,
            arms: vec![],
        }
    }
    #[test]
    fn report_contains_required_sections() {
        let report = generate_research_report(&fixture(), &ReportConfigV2::default());
        assert!(!report.study_name.is_empty());
        assert!(!report.study_id.is_empty());
        assert!(!report.provenance.config_hash.is_empty());
    }
    #[test]
    fn render_markdown_works() {
        let report = generate_research_report(&fixture(), &ReportConfigV2::default());
        let md = render_markdown(&report);
        assert!(md.contains("## Study"));
        assert!(md.contains("## Primary Metrics"));
        assert!(md.contains("## Effect Estimates"));
        assert!(md.contains("## Provenance"));
    }
    #[test]
    fn report_is_deterministic() {
        let r1 = generate_research_report(&fixture(), &ReportConfigV2::default());
        let r2 = generate_research_report(&fixture(), &ReportConfigV2::default());
        assert_eq!(r1.study_id, r2.study_id);
        assert_eq!(r1.provenance.config_hash, r2.provenance.config_hash);
    }
}
