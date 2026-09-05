//! matchlab-matchmaking: queue, matchmaker, constraints, and search strategies.
//!
//! Implements the `Queue` (spec §7.1), the `Matchmaker` trait and
//! `ProposedMatch` (spec §7.2), and the `Constraint` trait (spec §7.3).
//! Matchmakers are Lua scripts under `plugins/matchmaking/` implementing the
//! `find_matches` contract (see `lua.rs`): batch (spec §7.8), expanding_window
//! (spec §7.6), strict (spec §7.7), and hub_spoke (spec §7.9).

pub mod constraint;
pub mod lua;
pub mod matchmaker;
pub mod objective;
pub mod queue;
pub mod search;

pub use lua::LuaMatchmaker;
pub use matchmaker::{Matchmaker, ProposedMatch};
pub use objective::MatchObjective;
pub use queue::{Queue, QueueEntry};
pub use search::{
    BeamSearch, GreedySearch, RandomSamplingSearch, SearchStrategy, SearchStrategyKind,
};
