//! matchlab-ranking: rank mapping and leaderboard.
//!
//! Implements the `RankMapper` trait (spec §10.1) with a concrete
//! `BracketRankMapper` that maps ratings to rank tiers/divisions, plus a
//! `Leaderboard` (spec §10.2) that maintains rating-sorted player rankings.

pub mod leaderboard;
pub mod ranker;

pub use leaderboard::{Leaderboard, LeaderboardEntry};
pub use ranker::{BracketRankMapper, Rank, RankBracket, RankMapper};
