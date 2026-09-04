//! matchlab-game: outcome models and match execution.
//!
//! Defines the `OutcomeModel` trait and the concrete `LogisticOutcomeModel`
//! (spec §6.1, §6.2). Additional outcome variants (Variance, Composition,
//! Fatigue, Momentum) are out of scope for v0.1.
//!
//! Lua hooks allow runtime customization of outcome models via scripts
//! in `plugins/game/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod hooks;
pub mod logistic;
pub mod outcome;

pub use hooks::LuaHooks;
