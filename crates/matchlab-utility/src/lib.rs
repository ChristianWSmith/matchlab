//! matchlab-utility: player satisfaction / retention model.
//!
//! Models player retention probability from observable experience proxies
//! (§16). `SatisfactionModel` combines match quality, queue time, win/loss
//! history, streaks, rank progression, fairness, and rematch behavior into a
//! satisfaction score, then converts it to retention and rematch probabilities.

pub mod satisfaction;

pub use satisfaction::{PlayerExperience, SatisfactionModel, SatisfactionWeights};
