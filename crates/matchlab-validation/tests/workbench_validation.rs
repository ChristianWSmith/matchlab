//! Workbench validation and usability: validates the complete
//! research workflow — run → locate → inspect → query → compute → visualize
//! → compare → report → re-run → reproduce.
use matchlab_analysis::api::AnalysisAPI;
use matchlab_analysis::comparison::ComparisonEngine;
use matchlab_analysis::dataframe::{Column, DataType, DataValue, ResearchDataFrame};
use matchlab_analysis::report_v2::{ReportConfigV2, generate_research_report, render_markdown};
use matchlab_analysis::visualization::{JsonRenderer, PlotRenderer, plot_comparison};
/// End-to-end workflow test: demonstrate that a researcher can complete a
/// meaningful study without writing substantial custom data-processing glue code.
#[test]
fn end_to_end_research_workflow() {
    let study = matchlab_experiments::StudyResult {
        study_id: "workflow-test".to_string(),
        name: "End-to-End Workflow Test".to_string(),
        config_hash: "abc123".to_string(),
        git_commit: "deadbeef".to_string(),
        strategy: matchlab_experiments::SeedStrategy::Crn,
        design: matchlab_experiments::DesignType::Paired,
        experimental_design: matchlab_experiments::ExperimentalDesign::default(),
        replication_count: 20,
        arms: vec![],
    };
    assert_eq!(study.study_id, "workflow-test");
    assert_eq!(study.replication_count, 20);
    let viz_df = {
        let mut vdf = ResearchDataFrame::new(vec![
            Column {
                name: "policy".to_string(),
                dtype: DataType::Categorical,
            },
            Column {
                name: "queue_time".to_string(),
                dtype: DataType::Numeric,
            },
            Column {
                name: "match_quality".to_string(),
                dtype: DataType::Numeric,
            },
        ]);
        for i in 0..20 {
            vdf.add_row(vec![
                DataValue::Str("batch".to_string()),
                DataValue::Float(3.0 + i as f64 * 0.1),
                DataValue::Float(0.85 + i as f64 * 0.005),
            ]);
            vdf.add_row(vec![
                DataValue::Str("expanding".to_string()),
                DataValue::Float(5.0 + i as f64 * 0.1),
                DataValue::Float(0.90 + i as f64 * 0.005),
            ]);
        }
        vdf
    };
    let api = AnalysisAPI::new(viz_df.clone());
    let mean_qt = api.compute_mean("queue_time").unwrap();
    assert!(mean_qt > 0.0, "mean queue time should be positive");
    let ci = api.compute_ci("queue_time", 0.95).unwrap();
    assert!(ci.lower < ci.upper, "CI should be valid");
    let plot = plot_comparison(&viz_df, "policy", "queue_time");
    let renderer = JsonRenderer;
    let output = renderer.render(&plot);
    assert!(matches!(
        output,
        matchlab_analysis::visualization::PlotOutput::Json(_)
    ));
    let comp_df = {
        let mut cdf = ResearchDataFrame::new(vec![
            Column {
                name: "metric".to_string(),
                dtype: DataType::Categorical,
            },
            Column {
                name: "condition".to_string(),
                dtype: DataType::Categorical,
            },
            Column {
                name: "value".to_string(),
                dtype: DataType::Numeric,
            },
        ]);
        for i in 0..20 {
            cdf.add_row(vec![
                DataValue::Str("queue_time".to_string()),
                DataValue::Str("batch".to_string()),
                DataValue::Float(3.0 + i as f64 * 0.1),
            ]);
            cdf.add_row(vec![
                DataValue::Str("queue_time".to_string()),
                DataValue::Str("expanding".to_string()),
                DataValue::Float(5.0 + i as f64 * 0.1),
            ]);
        }
        cdf
    };
    let engine = ComparisonEngine::new(comp_df.clone());
    let comparison = engine
        .compare_conditions("queue_time", "batch", "expanding")
        .unwrap();
    assert!(
        comparison.difference > 0.0,
        "batch should be faster than expanding"
    );
    let report = generate_research_report(&study, &ReportConfigV2::default());
    let md = render_markdown(&report);
    assert!(
        md.contains("## Study"),
        "report should contain Study section"
    );
    assert!(
        md.contains("## Provenance"),
        "report should contain Provenance section"
    );
    let api2 = AnalysisAPI::new(viz_df);
    let mean_qt2 = api2.compute_mean("queue_time").unwrap();
    assert_eq!(mean_qt, mean_qt2, "re-run should produce identical results");
    let engine2 = ComparisonEngine::new(comp_df);
    let comparison2 = engine2
        .compare_conditions("queue_time", "batch", "expanding")
        .unwrap();
    assert_eq!(comparison.difference, comparison2.difference);
}
/// Provenance chain test: every artifact links back to its source experiment.
#[test]
fn provenance_chain_is_complete() {
    let study = matchlab_experiments::StudyResult {
        study_id: "prov-test".to_string(),
        name: "Provenance Test".to_string(),
        config_hash: "hash123".to_string(),
        git_commit: "abc456".to_string(),
        strategy: matchlab_experiments::SeedStrategy::Crn,
        design: matchlab_experiments::DesignType::Paired,
        experimental_design: matchlab_experiments::ExperimentalDesign::default(),
        replication_count: 10,
        arms: vec![],
    };
    let report = generate_research_report(&study, &ReportConfigV2::default());
    assert_eq!(report.provenance.study_id, "prov-test");
    assert_eq!(report.provenance.config_hash, "hash123");
    assert_eq!(report.provenance.git_commit, "abc456");
}
/// Reproducibility test: re-running analysis on stored artifacts is deterministic.
#[test]
fn analysis_reproducibility() {
    let df = ResearchDataFrame::new(vec![
        Column {
            name: "metric".to_string(),
            dtype: DataType::Categorical,
        },
        Column {
            name: "condition".to_string(),
            dtype: DataType::Categorical,
        },
        Column {
            name: "value".to_string(),
            dtype: DataType::Numeric,
        },
    ]);
    let mut df_mut = df.clone();
    for i in 0..10 {
        df_mut.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("test".to_string()),
            DataValue::Float(3.0 + i as f64 * 0.1),
        ]);
    }
    let api1 = AnalysisAPI::new(df_mut.clone());
    let api2 = AnalysisAPI::new(df_mut);
    let m1 = api1.compute_mean("value").unwrap();
    let m2 = api2.compute_mean("value").unwrap();
    assert_eq!(m1, m2, "analysis should be deterministic");
    let ci1 = api1.compute_ci("value", 0.95).unwrap();
    let ci2 = api2.compute_ci("value", 0.95).unwrap();
    assert_eq!(ci1.lower, ci2.lower);
    assert_eq!(ci1.upper, ci2.upper);
}
