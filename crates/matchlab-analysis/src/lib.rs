//! matchlab-analysis: statistics, Pareto frontier, cohorts, comparison,
//! reporting, and raw-data export.
pub mod aggregation;
pub mod api;
pub mod cohorts;
pub mod comparator;
pub mod comparison;
pub mod dataframe;
pub mod effect;
pub mod estimand;
pub mod export;
pub mod factors;
pub mod hierarchy;
pub mod multiple_comparisons;
pub mod navigator;
pub mod pareto;
pub mod pareto_explorer;
pub mod power;
pub mod query;
pub mod report;
pub mod report_v2;
pub mod result;
pub mod robustness;
pub mod stats;
pub mod study;
pub mod visualization;
pub use aggregation::{
    ReplicationObservation, ReplicationTable, build_replication_table, replication_scalar,
};
pub use cohorts::{CohortResult, analyze_cohort};
pub use comparator::{Comparator, MetricComparison};
pub use effect::{
    CiMethod, ConfidenceInterval, EffectSize, arm_stat, bootstrap_ci, ci, compare, effect_sizes,
    extract_scalar, paired_bootstrap_ci, paired_t_pvalue, student_t_ci, welch_ci, welch_pvalue,
    wilson_ci,
};
pub use estimand::{Estimand, EstimandDef, StatisticalEstimate, effect_for, estimate_point};
pub use export::{ExportFormat, RawDataExporter};
pub use hierarchy::{MetricObservation, MetricValue, ReplicationScalar, replication_scalars};
pub use multiple_comparisons::{Correction, benjamini_hochberg, holm};
pub use pareto::{ParetoPoint, pareto_front};
pub use power::{
    AllocationStrategy, PlanAssumptions, PowerSpec, StudyPlan, achieved_power, plan_study,
    required_replications,
};
pub use report::{ReportConfig, ReportFormat, generate_report};
pub use result::{EffectSizeSummary, SampleSummary, StatisticalResult, Uncertainty};
pub use stats::{Summary, summary, summary_to_result};
pub use study::{
    ArmMetricStat, PairEffect, StudyReportConfig, StudyStats, compute_study_stats,
    generate_research_report, generate_study_report, generate_study_report_json,
    write_study_result_json,
};
