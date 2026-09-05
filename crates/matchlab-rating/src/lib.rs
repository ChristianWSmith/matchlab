//! matchlab-rating: Lua-native rating systems.
//!
//! Rating systems are Lua scripts under `plugins/rating/` implementing the
//! `initialize` / `predict` / `update` contract (see `lua.rs`). The classic
//! systems (Elo, FlatPoints, Glicko-2, TrueSkill) ship as scripts; the
//! `RatingSystem` trait, `RatingState`, and the `information_budget`
//! sanitization in `filter.rs` stay in Rust.

pub mod filter;
pub mod lua;
pub mod plugins;
pub mod system;

pub use filter::{FilteredMatchResult, filter_match_result};
pub use lua::LuaRatingSystem;
pub use plugins::registry;
pub use system::{ObservationType, RatingState, RatingSystem};
