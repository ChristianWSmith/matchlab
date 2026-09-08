//! Match policy: combines hard constraints and soft objectives.
use crate::constraint::HardConstraint;
use crate::matchmaker::ProposedMatch;
use crate::objective::SoftObjective;
use matchlab_core::world::World;
/// A matchmaking policy that combines hard constraints with weighted soft
/// objectives. A proposed match must satisfy all hard constraints before
/// soft objectives are evaluated.
pub struct MatchPolicy {
    pub hard_constraints: Vec<Box<dyn HardConstraint>>,
    pub soft_objectives: Vec<(Box<dyn SoftObjective>, f64)>,
}
impl MatchPolicy {
    pub fn new() -> Self {
        Self {
            hard_constraints: Vec::new(),
            soft_objectives: Vec::new(),
        }
    }
    /// Add a hard constraint.
    pub fn with_constraint(mut self, constraint: Box<dyn HardConstraint>) -> Self {
        self.hard_constraints.push(constraint);
        self
    }
    /// Add a soft objective with a weight.
    pub fn with_objective(mut self, objective: Box<dyn SoftObjective>, weight: f64) -> Self {
        self.soft_objectives.push((objective, weight));
        self
    }
    /// Check if a proposed match satisfies all hard constraints.
    pub fn satisfies_constraints(&self, proposed: &ProposedMatch, world: &World) -> bool {
        self.hard_constraints
            .iter()
            .all(|c| c.is_satisfied(proposed, world))
    }
    /// Score a proposed match using the soft objectives (must satisfy constraints).
    pub fn score(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        if !self.satisfies_constraints(proposed, world) {
            return f64::NEG_INFINITY;
        }
        self.soft_objectives
            .iter()
            .map(|(obj, weight)| weight * obj.score(proposed, world))
            .sum()
    }
}
impl Default for MatchPolicy {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::TeamSizeConstraint;
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
    fn policy_rejects_constraint_violation() {
        let policy = MatchPolicy::new().with_constraint(Box::new(TeamSizeConstraint {
            size_a: 2,
            size_b: 2,
        }));
        let world = make_world();
        let proposed = ProposedMatch {
            team_a: vec![PlayerId(0), PlayerId(1)],
            team_b: vec![PlayerId(2)],
            quality_score: 0.9,
        };
        assert!(!policy.satisfies_constraints(&proposed, &world));
        assert_eq!(policy.score(&proposed, &world), f64::NEG_INFINITY);
    }
    #[test]
    fn policy_scores_with_objectives() {
        let policy = MatchPolicy::new()
            .with_constraint(Box::new(TeamSizeConstraint {
                size_a: 2,
                size_b: 2,
            }))
            .with_objective(Box::new(SkillBalanceObjective), 1.0);
        let world = make_world();
        let proposed = ProposedMatch {
            team_a: vec![PlayerId(0), PlayerId(1)],
            team_b: vec![PlayerId(2), PlayerId(3)],
            quality_score: 0.9,
        };
        let score = policy.score(&proposed, &world);
        assert!(score > 0.0, "score should be positive");
    }
    #[test]
    fn empty_policy_allows_all_matches() {
        let policy = MatchPolicy::new();
        let world = make_world();
        let proposed = ProposedMatch {
            team_a: vec![PlayerId(0)],
            team_b: vec![PlayerId(1)],
            quality_score: 0.5,
        };
        assert!(policy.satisfies_constraints(&proposed, &world));
        assert_eq!(policy.score(&proposed, &world), 0.0);
    }
}
