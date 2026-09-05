//! Momentum outcome model (§6.3): wraps a base model and slightly adjusts
//! effective skill based on recent results — players on a win streak get a
//! small boost, players on a losing streak a small penalty. `win_rate` is the
//! observable streak proxy (truth separation: never reads reality).

use matchlab_core::match_::{MatchId, MatchResult};
use matchlab_core::player::{PlayerObservation, SkillVector};
use matchlab_core::rng::SimRng;

use crate::outcome::OutcomeModel;

pub struct MomentumOutcomeModel {
    pub base_model: Box<dyn OutcomeModel>,
    pub momentum_factor: f64,
}

impl MomentumOutcomeModel {
    pub fn new(base_model: Box<dyn OutcomeModel>, momentum_factor: f64) -> Self {
        Self {
            base_model,
            momentum_factor,
        }
    }

    fn momentum_observations(&self, team: &[PlayerObservation]) -> Vec<PlayerObservation> {
        team.iter()
            .map(|obs| {
                let mut adjusted = obs.clone();
                let base = obs.skill_vector.overall();
                let boost = 1.0 + self.momentum_factor * (obs.win_rate - 0.5);
                adjusted.skill_vector = SkillVector::one_dimensional(base * boost);
                adjusted
            })
            .collect()
    }
}

impl OutcomeModel for MomentumOutcomeModel {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let a = self.momentum_observations(team_a);
        let b = self.momentum_observations(team_b);
        self.base_model.win_probability(&a, &b)
    }

    fn simulate(
        &self,
        match_id: MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult {
        let a = self.momentum_observations(team_a);
        let b = self.momentum_observations(team_b);
        self.base_model.simulate(match_id, &a, &b, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistic::LogisticOutcomeModel;
    use matchlab_core::player::{PlayerId, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64, win_rate: f64) -> PlayerObservation {
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
            games_played: 10,
            win_rate,
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
    fn winning_streak_boosts_win_probability() {
        let base = Box::new(LogisticOutcomeModel::new(400.0, 0.05));
        let model = MomentumOutcomeModel::new(base, 1.0);
        let hot = obs(1, 1000.0, 1.0); // 100% win rate → boost 1.5×
        let cold = obs(2, 1000.0, 0.0); // 0% win rate → boost 0.5×
        let p = model.win_probability(&[hot], &[cold]);
        assert!(p > 0.8, "hot streak should be favored: {p}");
    }

    #[test]
    fn zero_momentum_is_neutral() {
        let base = Box::new(LogisticOutcomeModel::new(400.0, 0.05));
        let model = MomentumOutcomeModel::new(base, 0.0);
        let a = obs(1, 1000.0, 1.0);
        let b = obs(2, 1000.0, 0.0);
        let p = model.win_probability(&[a], &[b]);
        assert!((p - 0.5).abs() < 1e-9);
    }

    #[test]
    fn simulate_well_formed() {
        let base = Box::new(LogisticOutcomeModel::new(400.0, 0.05));
        let model = MomentumOutcomeModel::new(base, 1.0);
        let a = vec![obs(1, 1000.0, 0.9)];
        let b = vec![obs(2, 1000.0, 0.1)];
        let mut rng = SimRng::from_seed(3);
        let result = model.simulate(MatchId(1), &a, &b, &mut rng);
        assert_eq!(result.player_performances.len(), 2);
    }
}
