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
/// Extended intervention actions for the ecosystem model .
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EcosystemIntervention {
    /// No action taken.
    None,
    /// Warning issued to the player.
    Warning { reason: String },
    /// Restrict the player's matchmaking for a duration.
    Restriction { duration_ticks: u64, reason: String },
    /// Modify queue parameters for this player.
    QueueModification { queue_penalty: f64, reason: String },
    /// Isolate from normal matchmaking pool.
    MatchmakingIsolation {
        isolation_duration: u64,
        reason: String,
    },
    /// Remove from the population entirely.
    RemovalFromPopulation { reason: String },
}
impl EcosystemIntervention {
    /// Whether this intervention affects the player's behavior.
    pub fn affects_behavior(&self) -> bool {
        !matches!(self, EcosystemIntervention::None)
    }
}
