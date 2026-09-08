//! Match selection policies: choosing among candidate matches
//! with baseline policies: greedy, weighted objective, lexicographic, and
//! threshold.
use crate::matchmaker::ProposedMatch;
use crate::objective::{ObjectiveWeights, SoftObjective};
use matchlab_core::world::World;
/// A policy for selecting matches from candidates.
pub trait MatchSelectionPolicy: Send + Sync {
    fn select(&self, candidates: Vec<ProposedMatch>, world: &World) -> Vec<ProposedMatch>;
}
/// Greedy selection: choose the single best candidate.
pub struct GreedySelection {
    pub objectives: Vec<(Box<dyn SoftObjective>, f64)>,
}
impl MatchSelectionPolicy for GreedySelection {
    fn select(&self, mut candidates: Vec<ProposedMatch>, world: &World) -> Vec<ProposedMatch> {
        if candidates.is_empty() {
            return candidates;
        }
        let scored: Vec<(usize, f64)> = candidates
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let score: f64 = self
                    .objectives
                    .iter()
                    .map(|(obj, weight)| weight * obj.score(m, world))
                    .sum();
                (i, score)
            })
            .collect();
        let best = scored.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some(&(idx, _)) = best {
            vec![candidates.remove(idx)]
        } else {
            Vec::new()
        }
    }
}
/// Weighted objective selection: minimize/maximize a scalarized objective.
pub struct WeightedObjectiveSelection {
    pub weights: ObjectiveWeights,
}
impl MatchSelectionPolicy for WeightedObjectiveSelection {
    fn select(&self, mut candidates: Vec<ProposedMatch>, world: &World) -> Vec<ProposedMatch> {
        if candidates.is_empty() {
            return candidates;
        }
        let mut scored: Vec<(usize, f64)> = candidates
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let avg_a = average_rating(&m.team_a, world);
                let avg_b = average_rating(&m.team_b, world);
                let skill_q = 1.0 - ((avg_a - avg_b).abs() / 400.0).min(1.0);
                let score = self.weights.skill_quality * skill_q;
                (i, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored
            .into_iter()
            .map(|(i, _)| candidates.remove(i))
            .collect()
    }
}
/// Lexicographic selection: optimize priorities in order.
pub struct LexicographicSelection {
    pub objectives: Vec<(Box<dyn SoftObjective>, f64)>,
}
impl MatchSelectionPolicy for LexicographicSelection {
    fn select(&self, mut candidates: Vec<ProposedMatch>, world: &World) -> Vec<ProposedMatch> {
        if candidates.is_empty() || self.objectives.is_empty() {
            return candidates;
        }
        for (obj, _) in &self.objectives {
            let scores: Vec<f64> = candidates.iter().map(|m| obj.score(m, world)).collect();
            let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let threshold = max_score - 0.001;
            candidates = candidates
                .into_iter()
                .zip(scores)
                .filter(|(_, s)| *s >= threshold)
                .map(|(m, _)| m)
                .collect();
            if candidates.len() == 1 {
                break;
            }
        }
        candidates
    }
}
/// Threshold selection: accept first candidate meeting minimum quality.
pub struct ThresholdSelection {
    pub min_quality: f64,
    pub objectives: Vec<(Box<dyn SoftObjective>, f64)>,
}
impl MatchSelectionPolicy for ThresholdSelection {
    fn select(&self, candidates: Vec<ProposedMatch>, world: &World) -> Vec<ProposedMatch> {
        for m in candidates {
            let score: f64 = self
                .objectives
                .iter()
                .map(|(obj, weight)| weight * obj.score(&m, world))
                .sum();
            if score >= self.min_quality {
                return vec![m];
            }
        }
        Vec::new()
    }
}
/// Helper to compute average rating for a team.
fn average_rating(team: &[matchlab_core::player::PlayerId], world: &World) -> f64 {
    let sum: f64 = team
        .iter()
        .filter_map(|pid| world.observations.get(pid))
        .map(|obs| obs.rating)
        .sum();
    sum / team.len().max(1) as f64
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::objective::SkillBalanceObjective;
    use matchlab_core::player::{PlayerId, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::world::World;
    fn make_world() -> World {
        let mut world = World::new(SimRng::from_seed(42));
        for i in 0..4 {
            let id = world.next_player_id();
            let reality = matchlab_core::player::PlayerReality {
                id,
                skill: SkillVector::one_dimensional(1000.0),
                skill_volatility: 0.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: matchlab_core::player::Region::NA,
                location: matchlab_core::player::GeoLocation::new(
                    matchlab_core::player::Region::NA,
                    0.0,
                    0.0,
                ),
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "test".to_string(),
                role: None,
            };
            let obs = matchlab_core::player::PlayerObservation {
                id,
                rating: 1000.0 + i as f64 * 100.0,
                hidden_mmr: 1000.0,
                visible_rank: VisibleRank {
                    tier: "gold".to_string(),
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
                session_history: std::collections::VecDeque::new(),
                quit_history: std::collections::VecDeque::new(),
                tilt_level: 0.0,
                game_mode: "ranked".to_string(),
                role: None,
                skill_vector: SkillVector::one_dimensional(1000.0),
                detection_flags: Vec::new(),
            };
            world.add_player(reality, obs);
        }
        world
    }
    #[test]
    fn greedy_selects_best() {
        let policy = GreedySelection {
            objectives: vec![(Box::new(SkillBalanceObjective), 1.0)],
        };
        let world = make_world();
        let candidates = vec![
            ProposedMatch {
                team_a: vec![PlayerId(0), PlayerId(1)],
                team_b: vec![PlayerId(2), PlayerId(3)],
                quality_score: 0.0,
            },
            ProposedMatch {
                team_a: vec![PlayerId(0), PlayerId(3)],
                team_b: vec![PlayerId(1), PlayerId(2)],
                quality_score: 0.0,
            },
        ];
        let selected = policy.select(candidates, &world);
        assert_eq!(selected.len(), 1);
    }
    #[test]
    fn threshold_selects_first_above_min() {
        let policy = ThresholdSelection {
            min_quality: 0.5,
            objectives: vec![(Box::new(SkillBalanceObjective), 1.0)],
        };
        let world = make_world();
        let candidates = vec![
            ProposedMatch {
                team_a: vec![PlayerId(0)],
                team_b: vec![PlayerId(3)],
                quality_score: 0.0,
            },
            ProposedMatch {
                team_a: vec![PlayerId(1)],
                team_b: vec![PlayerId(2)],
                quality_score: 0.0,
            },
        ];
        let selected = policy.select(candidates, &world);
        assert!(!selected.is_empty(), "should select at least one candidate");
    }
    #[test]
    fn empty_candidates_returns_empty() {
        let policy = GreedySelection {
            objectives: vec![(Box::new(SkillBalanceObjective), 1.0)],
        };
        let world = make_world();
        let selected = policy.select(vec![], &world);
        assert!(selected.is_empty());
    }
}
