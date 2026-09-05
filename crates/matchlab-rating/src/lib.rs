//! matchlab-rating: rating systems (Elo, Glicko-2, TrueSkill, Flat).
//!
//! v0.1 implements `EloRatingSystem` and `FlatPointsRatingSystem` behind the
//! `RatingSystem` trait (spec §8); Glicko-2 is implemented behind the same
//! trait. TrueSkill is out of scope. The plugin registry only exposes what is
//! implemented.
//!
//! Lua hooks allow runtime customization of rating algorithms via scripts
//! in `plugins/rating/`. See `docs/spec.md` §3.3 for hook signatures.

pub mod elo;
pub mod filter;
pub mod flat;
pub mod glicko;
pub mod hooks;
pub mod loader;
pub mod plugins;
pub mod system;

pub use elo::{EloConfig, EloRatingSystem};
pub use filter::{FilteredMatchResult, filter_match_result};
pub use flat::{FlatPointsConfig, FlatPointsRatingSystem};
pub use glicko::{Glicko2RatingSystem, GlickoConfig};
pub use hooks::LuaHooks;
pub use loader::{ScriptLoader, ScriptValidationResult};
pub use plugins::registry;
pub use system::{ObservationType, RatingState, RatingSystem};
