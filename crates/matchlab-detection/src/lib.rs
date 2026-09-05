//! matchlab-detection: Lua-native detection systems.
//!
//! Implements the `DetectionSystem` trait (spec §9). Detection algorithms are
//! Lua scripts under `plugins/detection/` (the smurf detector ships as
//! `smurf.lua`); the `InterventionAction` enum stays in Rust.

pub mod detector;
pub mod intervention;
pub mod lua;

pub use detector::{DetectionResult, DetectionSystem};
pub use intervention::InterventionAction;
pub use lua::LuaDetectionSystem;
