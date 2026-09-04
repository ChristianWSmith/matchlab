//! matchlab-game: outcome models and match execution.
//!
//! Defines the `OutcomeModel` trait and the concrete `LogisticOutcomeModel`
//! (spec §6.1, §6.2). Additional outcome variants (Variance, Composition,
//! Fatigue, Momentum) are out of scope for v0.1.

pub mod logistic;
pub mod outcome;
