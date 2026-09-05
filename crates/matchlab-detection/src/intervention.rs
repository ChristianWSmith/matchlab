use serde::{Deserialize, Serialize};

/// The action a detection system recommends for a player. The escalation
/// policy logic lives in the Lua detection script (e.g. `plugins/detection/
/// smurf.lua`); this enum is the Rust-side representation the loop acts on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterventionAction {
    None,
    AccelerateRating { multiplier: f64 },
    IncreaseKFactor { new_k: f64 },
    FlagForReview,
    RestrictQueue { duration_ticks: u64 },
    TempBan { duration_ticks: u64 },
    Probation { duration_ticks: u64 },
    Ban,
}
