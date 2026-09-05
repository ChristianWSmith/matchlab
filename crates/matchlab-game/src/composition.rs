//! Composition outcome model (§6.3): effective team skill is the weighted sum
//! of each player's SkillVector dimensions plus a synergy bonus. The key model
//! for the multidimensional-skill research question: can a 1D rating represent
//! multidimensional skill?

use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use std::collections::HashMap;

use crate::outcome::OutcomeModel;

pub struct CompositionOutcomeModel {
    pub dimension_weights: HashMap<String, f64>,
    pub synergy_bonus: f64,
    pub beta: f64,
}

impl CompositionOutcomeModel {
    pub fn new(dimension_weights: HashMap<String, f64>, synergy_bonus: f64, beta: f64) -> Self {
        Self {
            dimension_weights,
            synergy_bonus,
            beta,
        }
    }

    fn effective_skill(&self, obs: &PlayerObservation) -> f64 {
        if obs.skill_vector.dimensions.is_empty() {
            obs.rating
        } else {
            obs.skill_vector.weighted_overall(&self.dimension_weights)
        }
    }
}

impl OutcomeModel for CompositionOutcomeModel {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let sum_a: f64 = team_a.iter().map(|p| self.effective_skill(p)).sum();
        let sum_b: f64 = team_b.iter().map(|p| self.effective_skill(p)).sum();
        let effective_a = sum_a + self.synergy_bonus * team_a.len() as f64;
        let effective_b = sum_b + self.synergy_bonus * team_b.len() as f64;
        let diff = effective_a - effective_b;
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
            let skill = self.effective_skill(obs);
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
            visible_rank: VisibleRank {
                tier: "unranked".into(),
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
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
        }
    }

    #[test]
    fn synergy_bonus_amplifies_larger_team_advantage() {
        let mut weights = HashMap::new();
        weights.insert("overall".to_string(), 1.0);
        let no_synergy = CompositionOutcomeModel::new(weights.clone(), 0.0, 400.0);
        let synergic = CompositionOutcomeModel::new(weights, 200.0, 400.0);

        // Unequal teams: A has 2 players, B has 1. The per-player synergy bonus
        // compounds with team size, so it doesn't cancel.
        let a = vec![obs(1, 1000.0), obs(2, 1000.0)];
        let b = vec![obs(3, 1000.0)];

        let p0 = no_synergy.win_probability(&a, &b);
        let p1 = synergic.win_probability(&a, &b);
        // Both favor A (2v1), but synergy makes it strictly more decisive.
        assert!(p0 > 0.5);
        assert!(p1 > p0, "synergy should amplify the larger-team advantage");
    }

    #[test]
    fn weighted_dimensions_used_for_skill() {
        let mut weights = HashMap::new();
        weights.insert("aim".to_string(), 3.0);
        let model = CompositionOutcomeModel::new(weights, 0.0, 400.0);

        let mut a = obs(1, 1000.0);
        a.skill_vector = SkillVector {
            dimensions: HashMap::from([
                ("aim".to_string(), 1600.0),
                ("movement".to_string(), 800.0),
            ]),
        };
        let mut b = obs(2, 1000.0);
        b.skill_vector = SkillVector {
            dimensions: HashMap::from([
                ("aim".to_string(), 800.0),
                ("movement".to_string(), 1600.0),
            ]),
        };
        // A weights aim 3:1 → effective 1400; B → 1000; A favored (p ≈ 0.73).
        let p = model.win_probability(&[a], &[b]);
        assert!(p > 0.7, "aim-weighted team should be favored: {p}");
    }

    #[test]
    fn simulate_well_formed() {
        let model = CompositionOutcomeModel::new(HashMap::new(), 0.0, 400.0);
        let a = vec![obs(1, 1000.0)];
        let b = vec![obs(2, 1000.0)];
        let mut rng = SimRng::from_seed(3);
        let result = model.simulate(MatchId(1), &a, &b, &mut rng);
        assert_eq!(result.player_performances.len(), 2);
    }
}
