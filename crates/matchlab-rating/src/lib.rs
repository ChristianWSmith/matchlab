//! matchlab-rating: Lua-native rating systems.
//!
//! Rating systems are Lua scripts under `plugins/rating/` implementing the
//! `initialize` / `predict` / `update` contract (see `lua.rs`). Field-gating
//! is handled by `_fair` serialization in `matchlab-lua::convert`.
pub mod filter;
pub mod lua;
pub mod plugins;
pub mod system;
pub use lua::LuaRatingSystem;
pub use plugins::registry;
pub use system::{RatingState, RatingSystem};
