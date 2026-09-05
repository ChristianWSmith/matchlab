//! matchlab-experiments: runner, YAML config, inheritance, factorial design.

pub mod config;
pub mod counterfactual;
pub mod factorial;
pub mod inherit;
pub mod runner;
pub mod seed;

pub use config::ExperimentConfig;
pub use counterfactual::{GameHistory, counterfactual_eval};
pub use factorial::{Factor, FactorialDesign};
pub use runner::{ExperimentResult, ExperimentRunner};
pub use seed::SeedManager;
