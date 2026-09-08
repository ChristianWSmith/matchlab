//! matchlab-game: Lua-native outcome models.
//!
//! Outcome models are Lua scripts under `plugins/game/` implementing the
//! `win_probability` / `simulate` contract (see `lua.rs`). The classic variants
//! (logistic, variance, composition, performance, fatigue, momentum) ship as
//! scripts; the `OutcomeModel` trait stays in Rust.
pub mod lua;
pub mod outcome;
pub mod party_effect;
pub mod performance;
pub mod team;
pub use lua::LuaOutcomeModel;
pub use outcome::OutcomeModel;
pub use party_effect::{
    CommunicationBonus, CoordinationBonus, PartyEffect, PartyEffectContext, PartySizeEffect,
};
pub use performance::{
    DeterministicModel, GaussianNoiseModel, PerformanceContext, PerformanceModel,
};
pub use team::{
    AdditiveTeamModel, ComplementaryTeamModel, TeamContext, TeamModel, WeightedTeamModel,
};
