//! Performance outcome model (§6.3): individual performance stats slightly
//! affect the win probability. Players with higher impact shift their team's
//! odds, so performance-adjusted systems can be evaluated against it.

use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;

use crate::hooks::LuaHooks;
use crate::outcome::OutcomeModel;

pub struct PerformanceOutcomeModel {
    pub beta: f64,
    pub performance_weight: f64,
    hooks: Option<LuaHooks>,
}

impl PerformanceOutcomeModel {
    pub fn new(beta: f64, performance_weight: f64) -> Self {
        Self {
            beta,
            performance_weight,
            hooks: None,
        }
    }

    pub fn with_hooks(beta: f64, performance_weight: f64, hooks: LuaHooks) -> Self {
        Self {
            beta,
            performance_weight,
            hooks: Some(hooks),
        }
    }

    fn base_skill(&self, obs: &PlayerObservation) -> f64 {
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

    /// Recent performance proxy: recent_performances (a short deque of
    /// performance samples in [0, 1]) shifted to a mean-zero scale, scaled by
    /// `performance_weight × beta` so a full-swing streak shifts effective
    /// skill by `performance_weight` in beta units.
    fn performance_boost(&self, obs: &PlayerObservation) -> f64 {
        let recent = &obs.recent_performances;
        if recent.is_empty() {
            return 0.0;
        }
        let mean = recent.iter().sum::<f64>() / recent.len() as f64;
        (mean - 0.5) * self.performance_weight * self.beta
    }
}

impl OutcomeModel for PerformanceOutcomeModel {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let sum_a: f64 = team_a
            .iter()
            .map(|p| self.base_skill(p) + self.performance_boost(p))
            .sum();
        let sum_b: f64 = team_b
            .iter()
            .map(|p| self.base_skill(p) + self.performance_boost(p))
            .sum();
        let diff = sum_a - sum_b;
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
        let noise = rng.gen_range(-0.05, 0.05);
        let adjusted_p = (base_p + noise).clamp(0.01, 0.99);
        let team_a_wins = rng.gen_bool(adjusted_p);
        let winner = if team_a_wins { Team::A } else { Team::B };

        let team_a_ids: Vec<PlayerId> = team_a.iter().map(|p| p.id).collect();
        let team_b_ids: Vec<PlayerId> = team_b.iter().map(|p| p.id).collect();

        let mut performances = Vec::new();
        for obs in team_a.iter().chain(team_b.iter()) {
            let skill = self.base_skill(obs) + self.performance_boost(obs);
            performances.push(PlayerPerformance {
                player_id: obs.id,
                kills: (skill / 100.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                deaths: (5.0 - (skill / 1000.0) * 1.5 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                assists: (3.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                objective_score: rng.gen_range(0.0, 100.0) * (1.0 + skill / 3000.0),
                impact: rng.gen_range(-1.0, 1.0) + (skill - 1000.0) / 1500.0,
                variance: rng.gen_range(0.0, 1.0),
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
    fn high_performance_boosts_win_probability() {
        let model = PerformanceOutcomeModel::new(400.0, 1.0);
        let mut hot = obs(1, 1000.0);
        hot.recent_performances = vec![0.9, 0.9, 0.9]; // mean 0.9 → boost +160
        let mut cold = obs(2, 1000.0);
        cold.recent_performances = vec![0.1, 0.1, 0.1]; // mean 0.1 → boost −160

        let p = model.win_probability(&[hot], &[cold]);
        // diff 320 → logistic 0.69
        assert!(p > 0.6, "hot performer should be favored: {p}");
    }

    #[test]
    fn no_performance_data_is_neutral() {
        let model = PerformanceOutcomeModel::new(400.0, 1.0);
        let a = obs(1, 1000.0);
        let b = obs(2, 1000.0);
        let p = model.win_probability(&[a], &[b]);
        assert!((p - 0.5).abs() < 1e-9);
    }

    #[test]
    fn simulate_well_formed() {
        let model = PerformanceOutcomeModel::new(400.0, 1.0);
        let a = vec![obs(1, 1000.0)];
        let b = vec![obs(2, 1000.0)];
        let mut rng = SimRng::from_seed(3);
        let result = model.simulate(MatchId(1), &a, &b, &mut rng);
        assert_eq!(result.player_performances.len(), 2);
    }
}