//! matchlab-adversarial: Lua-native adversarial player agents.
//!
//! Players that actively try to exploit or manipulate the rating system (§15).
//! Each agent is a Lua script under `plugins/adversarial/` implementing the
//! `tick` / `objective` contract (see `lua.rs`). Agents act as the player's
//! behavior controller (like the outcome model), so they may adjust reality
//! behavior parameters (e.g. quit probability) and observable signals.

pub mod agent;
pub mod lua;

pub use agent::{AdversarialAgent, AdversarialObjective};
pub use lua::LuaAdversarialAgent;
