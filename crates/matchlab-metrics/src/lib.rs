//! matchlab-metrics: metric collectors (accuracy, quality, queue time, etc.).

pub mod accuracy;
pub mod collector;
pub mod engine;
pub mod quality;
pub mod queue;
pub mod stats;

pub use accuracy::RatingAccuracyCollector;
pub use collector::{MetricCollector, MetricResult};
pub use engine::MetricsEngine;
pub use quality::MatchQualityCollector;
pub use queue::QueueTimeCollector;
pub use stats::{Summary, summary, summary_to_result};
