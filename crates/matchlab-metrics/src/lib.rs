//! matchlab-metrics: Lua-native metric collectors.
//!
//! Metric collectors are Lua scripts under `plugins/metrics/` implementing the
//! `on_record` / `compute` contract (see `lua.rs`). The engine, the
//! `MetricResult` enum, and the canonical summary statistics stay in Rust.
pub mod cohort;
pub mod collector;
pub mod engine;
pub mod incremental;
pub mod lua;
pub mod stability;
pub mod stats;
pub use cohort::CohortFilter;
pub use collector::{MetricCollector, MetricResult};
pub use engine::MetricsEngine;
pub use incremental::{
    AggregatedValue, AggregationStrategy, CountAggregator, IncrementalAggregator, WelfordMean,
    WelfordVariance,
};
pub use lua::LuaMetricCollector;
pub use stability::{PopulationSnapshot, StabilityMetrics, StabilityVerdict, compute_stability};
pub use stats::{Summary, summary, summary_to_result};
