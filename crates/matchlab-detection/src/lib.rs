//! matchlab-detection: smurf detection, interventions, and anomaly detection.
//!
//! Implements the `DetectionSystem` trait (spec §9) with a concrete
//! `SmurfDetector` that tracks consecutive anomalous performances and
//! recommends intervention actions via an `InterventionPolicy`.

pub mod detector;
pub mod intervention;
pub mod smurf;

pub use detector::{DetectionResult, DetectionSystem};
pub use intervention::{InterventionAction, InterventionPolicy, PlayerInterventionState};
pub use smurf::SmurfDetector;
