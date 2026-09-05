//! matchlab-metrics: metric collectors (accuracy, quality, queue time, etc.).
//!
//! Lua hooks allow runtime customization of metric collectors via scripts
//! in `plugins/metrics/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod accuracy;
pub mod cohort;
pub mod collector;
pub mod convergence;
pub mod dimensionality;
pub mod engine;
pub mod hooks;
pub mod inequality;
pub mod ndcg;
pub mod population;
pub mod quality;
pub mod queue;
pub mod responsiveness;
pub mod smurf;
pub mod stability;
pub mod stats;
pub mod streaks;

pub use accuracy::RatingAccuracyCollector;
pub use cohort::{CohortFilter, tier_for_skill};
pub use collector::{MetricCollector, MetricResult};
pub use convergence::ConvergenceCollector;
pub use dimensionality::DimensionalityFidelityCollector;
pub use engine::MetricsEngine;
pub use hooks::LuaHooks;
pub use inequality::MatchInequalityCollector;
pub use ndcg::NDCGCollector;
pub use population::PopulationHealthCollector;
pub use quality::MatchQualityCollector;
pub use queue::QueueTimeCollector;
pub use responsiveness::ResponsivenessCollector;
pub use smurf::SmurfMetricsCollector;
pub use stability::StabilityCollector;
pub use stats::{Summary, summary, summary_to_result};
pub use streaks::StreakCollector;
