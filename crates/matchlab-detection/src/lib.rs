//! matchlab-detection: detection systems and interventions.
//!
//! Implements the `DetectionSystem` trait (spec §9) and the ecosystem-level
//! `EcosystemDetector` trait . Detection algorithms are Lua scripts under
//! `plugins/detection/` (the smurf detector ships as `smurf.lua`); the
//! `InterventionAction` and `EcosystemIntervention` enums stay in Rust.
pub mod detector;
pub mod intervention;
pub mod lua;
pub use detector::{DetectionResult, DetectionSystem, EcosystemDetectionResult, EcosystemDetector};
pub use intervention::{EcosystemIntervention, InterventionAction};
pub use lua::LuaDetectionSystem;
