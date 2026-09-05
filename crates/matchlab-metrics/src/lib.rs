//! matchlab-metrics: Lua-native metric collectors.
//!
//! Metric collectors are Lua scripts under `plugins/metrics/` implementing the
//! `on_record` / `compute` contract (see `lua.rs`). The engine, the
//! `MetricResult` enum, and the canonical summary statistics stay in Rust.

pub mod cohort;
pub mod collector;
pub mod engine;
pub mod lua;
pub mod stats;

pub use cohort::{CohortFilter, tier_for_skill};
pub use collector::{MetricCollector, MetricResult};
pub use engine::MetricsEngine;
pub use lua::LuaMetricCollector;
pub use stats::{Summary, summary, summary_to_result};
