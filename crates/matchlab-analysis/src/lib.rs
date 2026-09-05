//! matchlab-analysis: statistics, Pareto frontier, cohorts, comparison,
//! reporting, and raw-data export.

pub mod cohorts;
pub mod comparator;
pub mod export;
pub mod pareto;
pub mod report;
pub mod stats;

pub use cohorts::{CohortResult, analyze_cohort};
pub use comparator::{Comparator, MetricComparison};
pub use export::{ExportFormat, RawDataExporter};
pub use pareto::{ParetoPoint, pareto_front};
pub use report::{ReportConfig, ReportFormat, generate_report};
pub use stats::{Summary, summary, summary_to_result};
