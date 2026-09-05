//! matchlab-matchmaking: queue, matchmaker, constraints, and search strategies.
//!
//! Implements the `Queue` (spec §7.1), the `Matchmaker` trait and
//! `ProposedMatch` (spec §7.2), the `Constraint` trait (spec §7.3, no concrete
//! constraints), and the matchmakers: `BatchMatchmaker` (spec §7.8),
//! `ExpandingWindowMatchmaker` (spec §7.6), `StrictMatchmaker` (spec §7.7),
//! and `HubSpokeMatchmaker` (spec §7.9).
//!
//! Lua hooks allow runtime customization of matchmaking algorithms via scripts
//! in `plugins/matchmaking/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod batch;
pub mod constraint;
pub mod expanding;
pub mod hooks;
pub mod hub_spoke;
pub mod loader;
pub mod matchmaker;
pub mod queue;
pub mod strict;

pub use hooks::LuaHooks;
pub use loader::{ScriptLoader, ScriptValidationResult};
