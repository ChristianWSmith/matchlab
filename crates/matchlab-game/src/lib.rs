//! matchlab-game: Lua-native outcome models.
//!
//! Outcome models are Lua scripts under `plugins/game/` implementing the
//! `win_probability` / `simulate` contract (see `lua.rs`). The classic variants
//! (logistic, variance, composition, performance, fatigue, momentum) ship as
//! scripts; the `OutcomeModel` trait stays in Rust.

pub mod lua;
pub mod outcome;

pub use lua::LuaOutcomeModel;
pub use outcome::OutcomeModel;
