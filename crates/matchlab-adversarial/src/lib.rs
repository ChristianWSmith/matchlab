//! matchlab-adversarial: adversarial player agents.
//!
//! Players that actively try to exploit or manipulate the rating system
//! (§15). Each agent implements the `AdversarialAgent` trait and is called on
//! every tick to decide its action. Agents act as the player's behavior
//! controller (like the outcome model), so they may adjust reality behavior
//! parameters (e.g. quit probability) and observable signals.

pub mod afk;
pub mod agent;
pub mod booster;
pub mod deranker;
pub mod rating_farmer;
pub mod win_trader;

pub use afk::AfkAgent;
pub use agent::{AdversarialAgent, AdversarialObjective};
pub use booster::BoosterAgent;
pub use deranker::DerankerAgent;
pub use rating_farmer::RatingFarmerAgent;
pub use win_trader::WinTraderAgent;
