//! Fatigue outcome model (§6.3): wraps a base model and reduces effective
//! skill the longer a player has been in the session. Games played serves as
//! the observable session-length proxy (truth separation: never reads reality).

use matchlab_core::match_::{MatchId, MatchResult};
use matchlab_core::player::{PlayerObservation, SkillVector};
use matchlab_core::rng::SimRng;

use crate::outcome::OutcomeModel;

pub struct FatigueOutcomeModel {
    pub base_model: Box<dyn OutcomeModel>,
    pub fatigue_decay_rate: f64,
}

impl FatigueOutcomeModel {
    pub fn new(base_model: Box<dyn OutcomeModel>, fatigue_decay_rate: f64) -> Self {
        Self {
            base_model,
            fatigue_decay_rate,
        }
    }

    fn fatigued_observations(&self, team: &[PlayerObservation]) -> Vec<PlayerObservation> {
        team.iter()
            .map(|obs| {
                let mut adjusted = obs.clone();
                let base = obs.skill_vector.overall();
                let decay = (1.0 - self.fatigue_decay_rate * obs.games_played as f64).max(0.5);
                adjusted.skill_vector = SkillVector::one_dimensional(base * decay);
                adjusted
            })
            .collect()
    }
}

impl OutcomeModel for FatigueOutcomeModel {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let a = self.fatigued_observations(team_a);
        let b = self.fatigued_observations(team_b);
        self.base_model.win_probability(&a, &b)
    }

    fn simulate(
        &self,
        match_id: MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult {
        let a = self.fatigued_observations(team_a);
        let b = self.fatigued_observations(team_b);
        self.base_model.simulate(match_id, &a, &b, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistic::LogisticOutcomeModel;
    use matchlab_core::player::{PlayerId, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64, games_played: u64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
        }
    }

    #[test]
    fn long_session_reduces_effective_skill() {
        let base = Box::new(LogisticOutcomeModel::new(400.0, 0.05));
        let model = FatigueOutcomeModel::new(base, 0.01);
        let fresh = obs(1, 1500.0, 0);
        let fatigued = obs(2, 1500.0, 50); // 50 games → decay 0.5
        // Equal ratings, but one player is fatigued → the fresh team favored.
        let p = model.win_probability(&[fresh], &[fatigued]);
        assert!(p > 0.8, "fresh player should be favored: {p}");
    }

    #[test]
    fn zero_decay_is_neutral() {
        let base = Box::new(LogisticOutcomeModel::new(400.0, 0.05));
        let model = FatigueOutcomeModel::new(base, 0.0);
        let a = obs(1, 1000.0, 100);
        let b = obs(2, 1000.0, 0);
        let p = model.win_probability(&[a], &[b]);
        assert!((p - 0.5).abs() < 1e-9);
    }

    #[test]
    fn simulate_well_formed() {
        let base = Box::new(LogisticOutcomeModel::new(400.0, 0.05));
        let model = FatigueOutcomeModel::new(base, 0.01);
        let a = vec![obs(1, 1000.0, 10)];
        let b = vec![obs(2, 1000.0, 0)];
        let mut rng = SimRng::from_seed(3);
        let result = model.simulate(MatchId(1), &a, &b, &mut rng);
        assert_eq!(result.player_performances.len(), 2);
    }
}
