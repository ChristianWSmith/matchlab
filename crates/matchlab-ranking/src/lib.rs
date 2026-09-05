//! matchlab-ranking: Lua-native rank mapping and leaderboard.
//!
//! Implements the `RankMapper` trait (spec §10.1); the bracket mapping is a
//! Lua script (`plugins/ranking/brackets.lua`). The `Leaderboard` (spec §10.2)
//! maintains rating-sorted player rankings in Rust.

pub mod leaderboard;
pub mod lua;
pub mod ranker;

pub use leaderboard::{Leaderboard, LeaderboardEntry};
pub use lua::LuaRankMapper;
pub use ranker::{Rank, RankMapper};
