//! matchlab-metrics: metric collectors (accuracy, quality, queue time, etc.).
//!
//! Lua hooks allow runtime customization of metric collectors via scripts
//! in `plugins/metrics/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod accuracy;
pub mod collector;
pub mod engine;
pub mod hooks;
pub mod quality;
pub mod queue;
pub mod stats;

pub use accuracy::RatingAccuracyCollector;
pub use collector::{MetricCollector, MetricResult};
pub use engine::MetricsEngine;
pub use hooks::LuaHooks;
pub use quality::MatchQualityCollector;
pub use queue::QueueTimeCollector;
pub use stats::{Summary, summary, summary_to_result};
