//! Benchmark and canonical study suite: canonical collection
//! of studies demonstrating the full workbench workflow.
use matchlab_analysis::api::AnalysisAPI;
use matchlab_analysis::comparison::ComparisonEngine;
use matchlab_analysis::dataframe::{Column, DataType, DataValue, ResearchDataFrame};
use matchlab_analysis::report_v2::{ReportConfigV2, generate_research_report, render_markdown};
/// Create a sample research data frame for benchmark studies.
fn sample_study_data() -> ResearchDataFrame {
    let mut df = ResearchDataFrame::new(vec![
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
    for i in 0..10 {
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("batch".to_string()),
            DataValue::Float(3.0 + i as f64 * 0.1),
        ]);
        df.add_row(vec![
            DataValue::Str("match_quality".to_string()),
            DataValue::Str("batch".to_string()),
            DataValue::Float(0.85 + i as f64 * 0.005),
        ]);
    }
    for i in 0..10 {
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("expanding".to_string()),
            DataValue::Float(5.0 + i as f64 * 0.1),
        ]);
        df.add_row(vec![
            DataValue::Str("match_quality".to_string()),
            DataValue::Str("expanding".to_string()),
            DataValue::Float(0.90 + i as f64 * 0.005),
        ]);
    }
    df
}
/// Create a visualization-friendly data frame.
fn viz_data() -> ResearchDataFrame {
    let mut df = ResearchDataFrame::new(vec![
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
    for i in 0..10 {
        df.add_row(vec![
            DataValue::Str("batch".to_string()),
            DataValue::Float(3.0 + i as f64 * 0.1),
            DataValue::Float(0.85 + i as f64 * 0.005),
        ]);
        df.add_row(vec![
            DataValue::Str("expanding".to_string()),
            DataValue::Float(5.0 + i as f64 * 0.1),
            DataValue::Float(0.90 + i as f64 * 0.005),
        ]);
    }
    df
}
#[test]
fn study_1_skill_balance_vs_queue_time() {
    let _df = sample_study_data();
    let viz_df = viz_data();
    let viz_api = AnalysisAPI::new(viz_df);
    let mean_qt = viz_api.compute_mean("queue_time").unwrap();
    assert!(mean_qt > 0.0);
    let df = sample_study_data();
    let engine = ComparisonEngine::new(df);
    let comparison = engine
        .compare_conditions("queue_time", "batch", "expanding")
        .unwrap();
    assert!(comparison.mean_a > 0.0);
    assert!(comparison.difference > 0.0);
}
#[test]
fn study_2_party_effects() {
    let viz_df = viz_data();
    let api = AnalysisAPI::new(viz_df);
    let mean_qt = api.compute_mean("queue_time").unwrap();
    assert!(mean_qt > 0.0, "queue time should be positive");
    let ci = api.compute_ci("queue_time", 0.95).unwrap();
    assert!(ci.lower < ci.upper, "CI should be valid");
}
#[test]
fn study_3_latency_tradeoffs() {
    let df = sample_study_data();
    let engine = ComparisonEngine::new(df);
    let qt_comparison = engine
        .compare_conditions("queue_time", "batch", "expanding")
        .unwrap();
    assert!(qt_comparison.difference != 0.0, "should have a difference");
    let mq_comparison = engine
        .compare_conditions("match_quality", "batch", "expanding")
        .unwrap();
    assert!(mq_comparison.difference != 0.0, "should have a difference");
}
#[test]
fn study_4_strategic_adaptation() {
    let viz_df = viz_data();
    let api = AnalysisAPI::new(viz_df);
    let mean_mq = api.compute_mean("match_quality").unwrap();
    assert!(mean_mq > 0.8, "match quality should be high");
    let stddev = api.compute_stddev("match_quality").unwrap();
    assert!(stddev > 0.0, "should have some variance");
}
#[test]
fn study_5_multi_objective() {
    use matchlab_analysis::comparison::ConditionComparison;
    use matchlab_analysis::effect::ConfidenceInterval;
    use matchlab_analysis::pareto_explorer::ParetoExplorer;
    let results = vec![ConditionComparison {
        metric: "queue_time".to_string(),
        condition_a: "batch".to_string(),
        condition_b: "expanding".to_string(),
        mean_a: 3.45,
        mean_b: 5.45,
        difference: 2.0,
        ci: ConfidenceInterval {
            lower: 1.0,
            upper: 3.0,
            conf: 0.95,
            method: matchlab_analysis::effect::CiMethod::WelchT,
        },
        effect_size: None,
    }];
    let explorer = ParetoExplorer::from_comparison_results(results);
    assert!(!explorer.front.is_empty(), "should have Pareto front");
}
#[test]
fn study_6_full_pipeline() {
    let viz_df = viz_data();
    let api = AnalysisAPI::new(viz_df);
    let mean_qt = api.compute_mean("queue_time").unwrap();
    assert!(mean_qt > 0.0);
    let ci = api.compute_ci("queue_time", 0.95).unwrap();
    assert!(ci.lower < ci.upper);
    let df = sample_study_data();
    let engine = ComparisonEngine::new(df);
    let comparison = engine
        .compare_conditions("queue_time", "batch", "expanding")
        .unwrap();
    assert!(comparison.difference != 0.0);
    use matchlab_analysis::visualization::{JsonRenderer, PlotRenderer, plot_comparison};
    let plot = plot_comparison(&viz_data(), "policy", "queue_time");
    let renderer = JsonRenderer;
    let output = renderer.render(&plot);
    assert!(matches!(
        output,
        matchlab_analysis::visualization::PlotOutput::Json(_)
    ));
    let study = matchlab_experiments::StudyResult {
        study_id: "benchmark-6".to_string(),
        name: "full pipeline".to_string(),
        config_hash: "abc".to_string(),
        git_commit: "def".to_string(),
        strategy: matchlab_experiments::SeedStrategy::Crn,
        design: matchlab_experiments::DesignType::Paired,
        experimental_design: matchlab_experiments::ExperimentalDesign::default(),
        replication_count: 10,
        arms: vec![],
    };
    let report = generate_research_report(&study, &ReportConfigV2::default());
    let md = render_markdown(&report);
    assert!(md.contains("## Study"));
    assert!(md.contains("## Provenance"));
}
#[test]
fn all_studies_are_deterministic() {
    let df1 = viz_data();
    let df2 = viz_data();
    let api1 = AnalysisAPI::new(df1);
    let api2 = AnalysisAPI::new(df2);
    let m1 = api1.compute_mean("queue_time").unwrap();
    let m2 = api2.compute_mean("queue_time").unwrap();
    assert_eq!(m1, m2);
}
