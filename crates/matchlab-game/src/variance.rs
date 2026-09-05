//! Variance outcome model (§6.3): same as logistic but with a larger noise
//! envelope, so upsets are more common at a given skill gap.

use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;

use crate::hooks::LuaHooks;
use crate::outcome::OutcomeModel;

pub struct VarianceOutcomeModel {
    pub beta: f64,
    pub noise: f64,
    pub variance_multiplier: f64,
    hooks: Option<LuaHooks>,
}

impl VarianceOutcomeModel {
    pub fn new(beta: f64, noise: f64, variance_multiplier: f64) -> Self {
        Self {
            beta,
            noise,
            variance_multiplier,
            hooks: None,
        }
    }

    pub fn with_hooks(beta: f64, noise: f64, variance_multiplier: f64, hooks: LuaHooks) -> Self {
        Self {
            beta,
            noise,
            variance_multiplier,
            hooks: Some(hooks),
        }
    }

    fn effective_skill(&self, obs: &PlayerObservation) -> f64 {
        if let Some(ref hooks) = self.hooks {
            if let Some(skill) =
                hooks.call_effective_skill(obs.rating, obs.rating_deviation, obs.games_played)
            {
                return skill;
            }
        }
        let overall = obs.skill_vector.overall();
        if obs.skill_vector.dimensions.is_empty() {
            obs.rating
        } else {
            overall
        }
    }
}

impl OutcomeModel for VarianceOutcomeModel {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let avg_a: f64 =
            team_a.iter().map(|p| self.effective_skill(p)).sum::<f64>() / team_a.len() as f64;
        let avg_b: f64 =
            team_b.iter().map(|p| self.effective_skill(p)).sum::<f64>() / team_b.len() as f64;
        let diff = avg_a - avg_b;
        1.0 / (1.0 + (-diff / self.beta).exp())
    }

    fn simulate(
        &self,
        match_id: MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult {
        let base_p = self.win_probability(team_a, team_b);
        let noise = rng.gen_range(-self.noise, self.noise) * self.variance_multiplier;
        let adjusted_p = (base_p + noise).clamp(0.01, 0.99);
        let team_a_wins = rng.gen_bool(adjusted_p);
        let winner = if team_a_wins { Team::A } else { Team::B };

        let team_a_ids: Vec<PlayerId> = team_a.iter().map(|p| p.id).collect();
        let team_b_ids: Vec<PlayerId> = team_b.iter().map(|p| p.id).collect();

        let mut performances = Vec::new();
        for obs in team_a.iter().chain(team_b.iter()) {
            let perf_variance = rng.gen_range(0.0, 1.0);
            let skill = self.effective_skill(obs);
            performances.push(PlayerPerformance {
                player_id: obs.id,
                kills: (skill / 100.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                deaths: (5.0 - (skill / 1000.0) * 1.5 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                assists: (3.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                objective_score: rng.gen_range(0.0, 100.0) * (1.0 + skill / 3000.0),
                impact: rng.gen_range(-1.0, 1.0) + (skill - 1000.0) / 1500.0,
                variance: perf_variance,
            });
        }

        MatchResult {
            match_id,
            winner,
            team_a: team_a_ids,
            team_b: team_b_ids,
            team_a_score: if team_a_wins {
                13.0
            } else {
                rng.gen_range(4.0, 12.0)
            },
            team_b_score: if team_a_wins {
                rng.gen_range(4.0, 12.0)
            } else {
                13.0
            },
            player_performances: performances,
            duration: SimTime::from_secs(rng.gen_range(1200.0, 2400.0)),
            disconnected: false,
            forfeited: false,
            variance: noise.abs(),
            unexpected_events: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{SkillVector, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank { tier: "unranked".into(), division: 1 },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
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
    fn high_multiplier_yields_more_upsets() {
        let team_a = vec![obs(1, 1600.0), obs(2, 1500.0)];
        let weak_b = vec![obs(3, 900.0), obs(4, 950.0)];
        let mut rng = SimRng::from_seed(42);

        let low = VarianceOutcomeModel::new(400.0, 0.05, 0.1);
        let high = VarianceOutcomeModel::new(400.0, 0.05, 5.0);

        let count_upsets = |model: &VarianceOutcomeModel, games: u64| -> u64 {
            let mut r = SimRng::from_seed(42);
            let mut upsets = 0;
            for i in 0..games {
                let result = model.simulate(MatchId(i), &team_a, &weak_b, &mut r);
                if result.winner == Team::B {
                    upsets += 1;
                }
            }
            upsets
        };

        let low_upsets = count_upsets(&low, 10_000);
        let high_upsets = count_upsets(&high, 10_000);
        assert!(
            high_upsets > low_upsets,
            "high multiplier upsets {high_upsets} should exceed low {low_upsets}"
        );
        let _ = &mut rng;
    }

    #[test]
    fn win_probability_in_range() {
        let model = VarianceOutcomeModel::new(400.0, 0.05, 1.0);
        let p = model.win_probability(&[obs(1, 1200.0)], &[obs(2, 1100.0)]);
        assert!(p > 0.0 && p < 1.0);
    }

    #[test]
    fn simulate_builds_well_formed_result() {
        let model = VarianceOutcomeModel::new(400.0, 0.05, 1.0);
        let team_a = vec![obs(1, 1000.0)];
        let team_b = vec![obs(2, 1000.0)];
        let mut rng = SimRng::from_seed(7);
        let result = model.simulate(MatchId(1), &team_a, &team_b, &mut rng);
        assert_eq!(result.team_a.len(), 1);
        assert_eq!(result.team_b.len(), 1);
        assert_eq!(result.player_performances.len(), 2);
    }
}