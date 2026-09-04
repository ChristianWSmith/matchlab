//! matchlab-experiments: runner, YAML config, inheritance, factorial design.

pub mod config;
pub mod inherit;
pub mod runner;
pub mod seed;

pub use config::ExperimentConfig;
pub use runner::{ExperimentResult, ExperimentRunner};
pub use seed::SeedManager;
