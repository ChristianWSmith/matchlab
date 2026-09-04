//! matchlab-rating: rating systems (Elo, Glicko-2, TrueSkill, Flat).
//!
//! v0.1 implements `EloRatingSystem` and `FlatPointsRatingSystem` behind the
//! `RatingSystem` trait (spec §8). Glicko-2 and TrueSkill are out of scope for
//! v0.1; the plugin registry only exposes what is implemented.

pub mod elo;
pub mod filter;
pub mod flat;
pub mod plugins;
pub mod system;

pub use elo::{EloConfig, EloRatingSystem};
pub use filter::{FilteredMatchResult, filter_match_result};
pub use flat::{FlatPointsConfig, FlatPointsRatingSystem};
pub use plugins::registry;
pub use system::{ObservationType, RatingState, RatingSystem};
