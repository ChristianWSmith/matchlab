//! matchlab-game: outcome models and match execution.
//!
//! Defines the `OutcomeModel` trait (spec §6.1) and the concrete outcome
//! models: `LogisticOutcomeModel` (spec §6.2), `VarianceOutcomeModel`,
//! `CompositionOutcomeModel`, `PerformanceOutcomeModel`, `FatigueOutcomeModel`,
//! and `MomentumOutcomeModel` (spec §6.3).
//!
//! Lua hooks allow runtime customization of outcome models via scripts
//! in `plugins/game/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod composition;
pub mod fatigue;
pub mod hooks;
pub mod logistic;
pub mod momentum;
pub mod outcome;
pub mod performance;
pub mod variance;

pub use hooks::LuaHooks;
