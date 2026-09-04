//! matchlab-matchmaking: queue, matchmaker, constraints, and search strategies.
//!
//! v0.1 implements the `Queue` (spec §7.1), the `Matchmaker` trait and
//! `ProposedMatch` (spec §7.2), the `Constraint` trait (spec §7.3, no concrete
//! constraints yet), and the `BatchMatchmaker` (spec §7.8). The other
//! matchmakers (ExpandingWindow, Strict, HubSpoke) are out of scope for v0.1.
//!
//! Lua hooks allow runtime customization of matchmaking algorithms via scripts
//! in `plugins/matchmaking/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod batch;
pub mod constraint;
pub mod hooks;
pub mod loader;
pub mod matchmaker;
pub mod queue;

pub use hooks::LuaHooks;
pub use loader::{ScriptLoader, ScriptValidationResult};
