//! Recorded game history for counterfactual replay (spec §13.8,).
//!
//! `GameHistory` is the offline trace captured by a live recording run: every
//! match, plus the observation snapshot and the ground-truth binding in effect
//! when it resolved. Replay arms rerun the identical trace through a different
//! rating system and produce real metric output. The reality snapshot is a
//! metrics-only binding — replay needs `true_skill` to compute
//! `rating_accuracy` — and never feeds a rating update.
use matchlab_core::match_::MatchResult;
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::world::World;
use std::collections::HashMap;
/// The ground-truth fields replay collectors need (metrics only).
#[derive(Debug, Clone)]
pub struct RealitySnapshot {
    pub true_skill: f64,
    pub improvement_rate: f64,
    pub games_played: u64,
}
/// A recorded game history: match sequence + per-match observation snapshots.
pub struct GameHistory {
    /// The matches in resolution order.
    pub matches: Vec<MatchResult>,
    /// What the rating systems saw at each match (pre-update observations).
    pub player_snapshots: Vec<HashMap<PlayerId, PlayerObservation>>,
    /// Ground-truth bindings for the same participants (metrics only).
    pub reality_snapshots: Vec<HashMap<PlayerId, RealitySnapshot>>,
    /// Wall-clock (sim) time in seconds when each match resolved.
    pub times_secs: Vec<f64>,
}
impl GameHistory {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            player_snapshots: Vec::new(),
            reality_snapshots: Vec::new(),
            times_secs: Vec::new(),
        }
    }
    /// Record one match along with the observation + reality state in effect
    /// when it resolved. Snapshot stores only the participants.
    pub fn record(&mut self, match_result: &MatchResult, world: &World) {
        let mut obs = HashMap::new();
        let mut reality = HashMap::new();
        for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
            if let Some(o) = world.observations.get(pid) {
                obs.insert(*pid, o.clone());
            }
            if let Some(r) = world.reality(*pid) {
                reality.insert(
                    *pid,
                    RealitySnapshot {
                        true_skill: r.skill.overall(),
                        improvement_rate: r.improvement_rate,
                        games_played: r.games_played,
                    },
                );
            }
        }
        self.matches.push(match_result.clone());
        self.player_snapshots.push(obs);
        self.reality_snapshots.push(reality);
        self.times_secs.push(world.time.as_secs_f64());
    }
    pub fn len(&self) -> usize {
        self.matches.len()
    }
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}
impl Default for GameHistory {
    fn default() -> Self {
        Self::new()
    }
}
