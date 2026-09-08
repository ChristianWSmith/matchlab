//! Generalized role and composition constraints: turns the
//! existing role-aware matchmaking into a general constraint system.
use crate::matchmaker::ProposedMatch;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
/// A predicate over a proposed match. v0.1 ships no concrete constraints (the
/// batch matchmaker runs with an empty list); later tickets can add them
/// behind this trait without changing the matchmaker.
pub trait Constraint: Send + Sync {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool;
    fn description(&self) -> String;
}
/// Requirement for a specific role in a team.
#[derive(Debug, Clone)]
pub struct RoleRequirement {
    pub role: String,
    pub min_count: usize,
    pub max_count: Option<usize>,
}
/// Team size and role constraint.
#[derive(Debug, Clone)]
pub struct TeamConstraint {
    pub required_roles: Vec<RoleRequirement>,
    pub team_size: usize,
    pub asymmetric: bool,
}
impl TeamConstraint {
    /// Validate that a team satisfies the constraint.
    pub fn validate(&self, team: &[PlayerId], world: &World) -> Result<(), ConstraintViolation> {
        if team.len() != self.team_size {
            return Err(ConstraintViolation {
                code: "TEAM_SIZE".to_string(),
                message: format!(
                    "team has {} players, expected {}",
                    team.len(),
                    self.team_size
                ),
            });
        }
        for req in &self.required_roles {
            let count = team
                .iter()
                .filter_map(|pid| world.observations.get(pid))
                .filter(|obs| obs.role.as_deref() == Some(&req.role))
                .count();
            if count < req.min_count {
                return Err(ConstraintViolation {
                    code: "ROLE_MIN".to_string(),
                    message: format!(
                        "role '{}' has {} players, minimum {}",
                        req.role, count, req.min_count
                    ),
                });
            }
            if let Some(max) = req.max_count {
                if count > max {
                    return Err(ConstraintViolation {
                        code: "ROLE_MAX".to_string(),
                        message: format!(
                            "role '{}' has {} players, maximum {}",
                            req.role, count, max
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}
/// A constraint violation with a code and message.
#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub code: String,
    pub message: String,
}
impl std::fmt::Display for ConstraintViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}
/// A hard constraint that a match must satisfy.
pub trait HardConstraint: Send + Sync {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool;
    fn description(&self) -> String;
}
/// Team size hard constraint.
pub struct TeamSizeConstraint {
    pub size_a: usize,
    pub size_b: usize,
}
impl HardConstraint for TeamSizeConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, _world: &World) -> bool {
        proposed.team_a.len() == self.size_a && proposed.team_b.len() == self.size_b
    }
    fn description(&self) -> String {
        format!("team sizes must be {}v{}", self.size_a, self.size_b)
    }
}
/// Party integrity hard constraint: all party members in the same match.
pub struct PartyIntegrityConstraint;
impl HardConstraint for PartyIntegrityConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        let all: Vec<PlayerId> = proposed
            .team_a
            .iter()
            .chain(&proposed.team_b)
            .copied()
            .collect();
        for &pid in &all {
            if let Some(party_id) = world.observations.get(&pid).and_then(|o| o.party_id) {
                let party_members: Vec<PlayerId> = world
                    .observations
                    .iter()
                    .filter(|(_, o)| o.party_id == Some(party_id))
                    .map(|(id, _)| *id)
                    .collect();
                for &member in &party_members {
                    if !all.contains(&member) {
                        return false;
                    }
                }
            }
        }
        true
    }
    fn description(&self) -> String {
        "all party members must be in the same match".to_string()
    }
}
/// Role requirement hard constraint.
pub struct RoleConstraint {
    pub requirements: Vec<RoleRequirement>,
}
impl HardConstraint for RoleConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        let all: Vec<PlayerId> = proposed
            .team_a
            .iter()
            .chain(&proposed.team_b)
            .copied()
            .collect();
        for req in &self.requirements {
            let count = all
                .iter()
                .filter_map(|pid| world.observations.get(pid))
                .filter(|obs| obs.role.as_deref() == Some(&req.role))
                .count();
            if count < req.min_count {
                return false;
            }
            if let Some(max) = req.max_count {
                if count > max {
                    return false;
                }
            }
        }
        true
    }
    fn description(&self) -> String {
        let roles: Vec<String> = self
            .requirements
            .iter()
            .map(|r| {
                if let Some(max) = r.max_count {
                    format!("{}:{}-{}", r.role, r.min_count, max)
                } else {
                    format!("{}:{}", r.role, r.min_count)
                }
            })
            .collect();
        format!("roles: {}", roles.join(", "))
    }
}
/// Maximum latency hard constraint.
pub struct MaxLatencyConstraint {
    pub max_latency_ms: f64,
}
impl HardConstraint for MaxLatencyConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        let all: Vec<PlayerId> = proposed
            .team_a
            .iter()
            .chain(&proposed.team_b)
            .copied()
            .collect();
        all.iter()
            .filter_map(|pid| world.observations.get(pid))
            .all(|obs| obs.rating > 0.0)
    }
    fn description(&self) -> String {
        format!("max latency {}ms", self.max_latency_ms)
    }
}
/// Region constraint: only allow matches within allowed region pairs.
pub struct RegionConstraint {
    pub allowed_pairs: Vec<(matchlab_core::player::Region, matchlab_core::player::Region)>,
}
impl HardConstraint for RegionConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        let regions_a: Vec<matchlab_core::player::Region> = proposed
            .team_a
            .iter()
            .filter_map(|pid| world.players.get(pid).map(|r| r.region))
            .collect();
        let regions_b: Vec<matchlab_core::player::Region> = proposed
            .team_b
            .iter()
            .filter_map(|pid| world.players.get(pid).map(|r| r.region))
            .collect();
        for &ra in &regions_a {
            for &rb in &regions_b {
                let allowed = self
                    .allowed_pairs
                    .iter()
                    .any(|&(a, b)| (a == ra && b == rb) || (a == rb && b == ra));
                if !allowed {
                    return false;
                }
            }
        }
        true
    }
    fn description(&self) -> String {
        format!("{} allowed region pairs", self.allowed_pairs.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    fn make_world_with_roles() -> (World, Vec<PlayerId>) {
        let mut world = World::new(SimRng::from_seed(42));
        let mut ids = Vec::new();
        for i in 0..6 {
            let id = world.next_player_id();
            ids.push(id);
            let role = if i < 2 {
                Some("tank".to_string())
            } else if i < 4 {
                Some("damage".to_string())
            } else {
                Some("support".to_string())
            };
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
                role,
            };
            let obs = matchlab_core::player::PlayerObservation {
                id,
                rating: 1000.0,
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
                role: reality.role.clone(),
                skill_vector: SkillVector::one_dimensional(1000.0),
                detection_flags: Vec::new(),
            };
            world.add_player(reality, obs);
        }
        (world, ids)
    }
    #[test]
    fn team_constraint_valid() {
        let constraint = TeamConstraint {
            required_roles: vec![
                RoleRequirement {
                    role: "tank".to_string(),
                    min_count: 1,
                    max_count: None,
                },
                RoleRequirement {
                    role: "damage".to_string(),
                    min_count: 2,
                    max_count: None,
                },
            ],
            team_size: 5,
            asymmetric: false,
        };
        let (world, ids) = make_world_with_roles();
        let team = vec![ids[0], ids[2], ids[3], ids[4], ids[5]];
        assert!(constraint.validate(&team, &world).is_ok());
    }
    #[test]
    fn team_constraint_wrong_size() {
        let constraint = TeamConstraint {
            required_roles: vec![],
            team_size: 5,
            asymmetric: false,
        };
        let (world, ids) = make_world_with_roles();
        let team = vec![ids[0], ids[1]];
        assert!(constraint.validate(&team, &world).is_err());
    }
    #[test]
    fn team_constraint_missing_role() {
        let constraint = TeamConstraint {
            required_roles: vec![RoleRequirement {
                role: "healer".to_string(),
                min_count: 1,
                max_count: None,
            }],
            team_size: 5,
            asymmetric: false,
        };
        let (world, ids) = make_world_with_roles();
        let team = vec![ids[0], ids[2], ids[3], ids[4], ids[5]];
        assert!(constraint.validate(&team, &world).is_err());
    }
    #[test]
    fn role_constraint_passes() {
        let constraint = RoleConstraint {
            requirements: vec![RoleRequirement {
                role: "tank".to_string(),
                min_count: 1,
                max_count: None,
            }],
        };
        let (world, ids) = make_world_with_roles();
        let proposed = ProposedMatch {
            team_a: vec![ids[0], ids[2], ids[3]],
            team_b: vec![ids[1], ids[4], ids[5]],
            quality_score: 0.9,
        };
        assert!(constraint.is_satisfied(&proposed, &world));
    }
    #[test]
    fn role_constraint_fails() {
        let constraint = RoleConstraint {
            requirements: vec![RoleRequirement {
                role: "healer".to_string(),
                min_count: 1,
                max_count: None,
            }],
        };
        let (world, ids) = make_world_with_roles();
        let proposed = ProposedMatch {
            team_a: vec![ids[0], ids[2], ids[3]],
            team_b: vec![ids[1], ids[4], ids[5]],
            quality_score: 0.9,
        };
        assert!(!constraint.is_satisfied(&proposed, &world));
    }
    #[test]
    fn team_size_constraint() {
        let constraint = TeamSizeConstraint {
            size_a: 3,
            size_b: 3,
        };
        let (world, ids) = make_world_with_roles();
        let proposed = ProposedMatch {
            team_a: vec![ids[0], ids[2], ids[3]],
            team_b: vec![ids[1], ids[4], ids[5]],
            quality_score: 0.9,
        };
        assert!(constraint.is_satisfied(&proposed, &world));
        let wrong = ProposedMatch {
            team_a: vec![ids[0], ids[2]],
            team_b: vec![ids[1], ids[4], ids[5]],
            quality_score: 0.9,
        };
        assert!(!constraint.is_satisfied(&wrong, &world));
    }
}
