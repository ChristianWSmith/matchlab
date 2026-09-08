//! matchlab-adversarial: strategic player agents and adversarial behavior.
//!
//! Players that actively try to exploit or manipulate the rating system (§15).
//! Each agent is a Lua script under `plugins/adversarial/` implementing the
//! `tick` / `objective` contract (see `lua.rs`). Agents act as the player's
//! behavior controller (like the outcome model), so they may adjust reality
//! behavior parameters (e.g. quit probability) and observable signals.
pub mod agent;
pub mod lua;
pub mod manipulation;
pub mod objectives;
pub mod policy;
pub use agent::{
    AdversarialAgent, AdversarialObjective, AgentAction, AgentObservations, AgentOutcome,
    MatchSummary, NullObjective, OrdinaryPlayer, PlayerObjective, StrategicAgent,
};
pub use lua::LuaAdversarialAgent;
pub use manipulation::{
    InformationInferenceStrategy, ManipulationAction, ManipulationStrategy, PartyExploitStrategy,
    QueueGamingStrategy, RatingDumpStrategy, RegionHoppingStrategy,
};
pub use objectives::{
    CompositeObjective, LatencyObjective, MatchQualityObjective, QueueTimeObjective,
    RatingGainObjective, RatingVarianceObjective, TimeSpentObjective, WinRateObjective,
};
pub use policy::{
    AdaptivePolicy, LossAversionPolicy, PartyFormationPolicy, PatternDetectionPolicy,
    RegionSwitchPolicy, ThresholdQueuePolicy,
};
