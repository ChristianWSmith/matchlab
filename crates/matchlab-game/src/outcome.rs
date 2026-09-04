//! Outcome model trait (spec §6.1).
//!
//! An outcome model turns two teams of `PlayerObservation` into a win
//! probability and, given an RNG, a concrete `MatchResult`. It consumes
//! observations only — it never looks up `PlayerReality` (truth separation).

use matchlab_core::match_::{MatchId, MatchResult};
use matchlab_core::player::PlayerObservation;
use matchlab_core::rng::SimRng;

pub trait OutcomeModel: Send + Sync {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64;

    fn simulate(
        &self,
        match_id: MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult;
}
