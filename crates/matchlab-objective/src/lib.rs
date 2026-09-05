//! matchlab-objective: weighted utility, multi-objective scoring.
//!
//! Combines multiple raw metrics into a single utility score for comparing
//! experiments (§12). `ObjectiveFunction::evaluate` returns the aggregate
//! score AND the raw metrics — raw values are never discarded (§12.2).

pub mod utility;

pub use utility::{ObjectiveFunction, ObjectiveWeights};
