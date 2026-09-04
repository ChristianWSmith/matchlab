//! Logistic outcome model (spec §6.2).
//!
//! Computes a team win probability as the logistic of the average-skill
//! difference and simulates the match by adding noise, picking a winner, and
//! building a fully-populated `MatchResult`.

use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;

use crate::outcome::OutcomeModel;

pub struct LogisticOutcomeModel {
    pub beta: f64,
    pub noise: f64,
    /// Present but inert in v0.1. When enabled, win probability is computed
    /// from each player's `SkillVector` (`weighted_overall`) rather than the
    /// flat `rating` scalar — the multidimensional-skill research path (§6.3).
    pub use_multidimensional: bool,
    pub dimension_weights: std::collections::HashMap<String, f64>,
}

impl LogisticOutcomeModel {
    pub fn new(beta: f64, noise: f64) -> Self {
        Self {
            beta,
            noise,
            use_multidimensional: false,
            dimension_weights: std::collections::HashMap::new(),
        }
    }

    fn effective_skill(&self, obs: &PlayerObservation) -> f64 {
        if self.use_multidimensional {
            obs.skill_vector.weighted_overall(&self.dimension_weights)
        } else {
            obs.rating
        }
    }
}

impl OutcomeModel for LogisticOutcomeModel {
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
        let noise = rng.gen_range(-self.noise, self.noise);
        let adjusted_p = (base_p + noise).clamp(0.01, 0.99);
        let team_a_wins = rng.gen_bool(adjusted_p);
        let winner = if team_a_wins { Team::A } else { Team::B };

        let team_a_ids: Vec<PlayerId> = team_a.iter().map(|p| p.id).collect();
        let team_b_ids: Vec<PlayerId> = team_b.iter().map(|p| p.id).collect();

        let mut performances = Vec::new();
        for obs in team_a.iter().chain(team_b.iter()) {
            let perf_variance = rng.gen_range(0.0, 1.0);
            let skill = self.effective_skill(obs);
            let aim = obs
                .skill_vector
                .dimensions
                .get("aim")
                .copied()
                .unwrap_or(skill);
            performances.push(PlayerPerformance {
                player_id: obs.id,
                kills: (aim / 100.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
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
            visible_rank: VisibleRank {
                tier: "unranked".to_string(),
                division: 1,
            },
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
            game_mode: "ranked".to_string(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
        }
    }

    fn team_a_players() -> Vec<PlayerObservation> {
        vec![obs(1, 1000.0), obs(2, 1000.0), obs(3, 1000.0)]
    }

    fn team_b_players() -> Vec<PlayerObservation> {
        vec![obs(4, 1000.0), obs(5, 1000.0), obs(6, 1000.0)]
    }

    #[test]
    fn equal_teams_have_win_probability_half() {
        let model = LogisticOutcomeModel::new(400.0, 0.05);
        let p = model.win_probability(&team_a_players(), &team_b_players());
        assert!((p - 0.5).abs() < 1e-9, "p drifted: {p}");
    }

    #[test]
    fn imbalance_shifts_probability_toward_favored_team() {
        let model = LogisticOutcomeModel::new(400.0, 0.05);
        let strong_a = vec![obs(1, 1600.0), obs(2, 1500.0)];
        let weak_b = vec![obs(3, 900.0), obs(4, 950.0)];
        let p = model.win_probability(&strong_a, &weak_b);
        assert!(p > 0.8, "favored team probability too low: {p}");
        assert!(p < 1.0);
        // The mirror matchup favors the other side equally.
        let p_reverse = model.win_probability(&weak_b, &strong_a);
        assert!((p + p_reverse - 1.0).abs() < 1e-9);
    }

    #[test]
    fn equal_teams_win_about_half_over_ten_thousand_games() {
        let model = LogisticOutcomeModel::new(400.0, 0.05);
        let team_a = team_a_players();
        let team_b = team_b_players();

        let mut rng = SimRng::from_seed(1234);
        let mut team_a_wins = 0u64;
        for i in 0..10_000 {
            let result = model.simulate(MatchId(i), &team_a, &team_b, &mut rng);
            if result.winner == Team::A {
                team_a_wins += 1;
            }
        }
        let rate = team_a_wins as f64 / 10_000.0;
        assert!(
            (rate - 0.5).abs() < 0.02,
            "win rate drifted from 50%: {rate}"
        );
    }

    #[test]
    fn favored_team_wins_more_often_and_tracks_win_probability() {
        let model = LogisticOutcomeModel::new(400.0, 0.05);
        let strong_a = vec![obs(1, 1600.0), obs(2, 1500.0)];
        let weak_b = vec![obs(3, 900.0), obs(4, 950.0)];
        let expected_p = model.win_probability(&strong_a, &weak_b);

        let mut rng = SimRng::from_seed(99);
        let mut team_a_wins = 0u64;
        let games = 10_000u64;
        for i in 0..games {
            let result = model.simulate(MatchId(i), &strong_a, &weak_b, &mut rng);
            if result.winner == Team::A {
                team_a_wins += 1;
            }
        }
        let rate = team_a_wins as f64 / games as f64;
        // Favored team wins decisively more than half the time.
        assert!(rate > 0.8, "favored win rate too low: {rate}");
        // Empirical rate tracks the analytic win probability within tolerance.
        assert!(
            (rate - expected_p).abs() < 0.05,
            "rate {rate} vs p {expected_p}"
        );
    }

    #[test]
    fn simulate_builds_well_formed_result_for_every_player() {
        let model = LogisticOutcomeModel::new(400.0, 0.05);
        let team_a = team_a_players();
        let team_b = team_b_players();
        let mut rng = SimRng::from_seed(7);

        let result = model.simulate(MatchId(5), &team_a, &team_b, &mut rng);

        assert_eq!(result.match_id, MatchId(5));
        assert_eq!(result.team_a.len(), 3);
        assert_eq!(result.team_b.len(), 3);
        assert_eq!(result.player_performances.len(), 6);
        assert!(!result.duration.as_secs_f64().is_nan());
        assert!(result.variance >= 0.0);

        let mut all_ids: Vec<u64> = result
            .player_performances
            .iter()
            .map(|p| p.player_id.0)
            .collect();
        all_ids.sort();
        assert_eq!(all_ids, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn simulation_is_deterministic_given_seed() {
        let model = LogisticOutcomeModel::new(400.0, 0.05);
        let team_a = team_a_players();
        let team_b = team_b_players();

        let mut rng_a = SimRng::from_seed(42);
        let mut rng_b = SimRng::from_seed(42);
        let a = model.simulate(MatchId(1), &team_a, &team_b, &mut rng_a);
        let b = model.simulate(MatchId(1), &team_a, &team_b, &mut rng_b);

        assert_eq!(a.winner, b.winner);
        assert_eq!(a.team_a_score, b.team_a_score);
        assert_eq!(a.team_b_score, b.team_b_score);
        assert_eq!(a.variance, b.variance);
        assert_eq!(a.duration, b.duration);
        for (pa, pb) in a
            .player_performances
            .iter()
            .zip(b.player_performances.iter())
        {
            assert_eq!(pa.kills, pb.kills);
            assert_eq!(pa.deaths, pb.deaths);
            assert_eq!(pa.impact, pb.impact);
        }
    }
}
