//! matchlab-analysis: statistics, reporting, and raw-data export.

pub mod export;
pub mod report;
pub mod stats;

pub use export::{ExportFormat, RawDataExporter};
pub use report::{ReportConfig, ReportFormat, generate_report};
pub use stats::{Summary, summary, summary_to_result};
