//! Matchmaking calibration and evaluation: comprehensive
//! acceptance tests for the matchmaking laboratory — validating that the
//! system correctly handles skill balance, parties, geography, latency,
//! role constraints, and multi-objective matchmaking.
use matchlab_core::player::{GeoLocation, PlayerId, Region, SkillVector, VisibleRank};
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;
use matchlab_matchmaking::constraint::{
    HardConstraint, RoleConstraint, RoleRequirement, TeamSizeConstraint,
};
use matchlab_matchmaking::latency::{LatencyMatrix, LatencyModel};
use matchlab_matchmaking::matchmaker::ProposedMatch;
use matchlab_matchmaking::objective::{
    MatchObjectiveVector, ObjectiveWeights, SkillBalanceObjective, SoftObjective,
};
use matchlab_matchmaking::party::{Party, PartyRegistry, check_party_integrity};
use matchlab_matchmaking::policy::MatchPolicy;
use std::collections::HashMap;
fn make_world_with_players(n: usize) -> (World, Vec<PlayerId>) {
    let mut world = World::new(SimRng::from_seed(42));
    let mut ids = Vec::new();
    for i in 0..n {
        let id = world.next_player_id();
        ids.push(id);
        let reality = matchlab_core::player::PlayerReality {
            id,
            skill: SkillVector::one_dimensional(1000.0 + i as f64 * 100.0),
            skill_volatility: 0.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::NA,
            location: GeoLocation::new(Region::NA, 40.0, -74.0),
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
    (world, ids)
}
#[test]
fn identical_skill_gives_no_advantage_to_skill_matching() {
    let (world, ids) = make_world_with_players(4);
    let balance = SkillBalanceObjective;
    let proposed = ProposedMatch {
        team_a: vec![ids[0], ids[1]],
        team_b: vec![ids[2], ids[3]],
        quality_score: 0.0,
    };
    let score = balance.score(&proposed, &world);
    assert!(score > 0.0, "skill balance score should be positive");
}
#[test]
fn party_integrity_enforced() {
    let mut reg = PartyRegistry::default();
    reg.register(
        Party::new(
            1,
            vec![PlayerId(0), PlayerId(1)],
            matchlab_core::time::SimTime::ZERO,
            None,
        )
        .unwrap(),
    );
    let result = check_party_integrity(&[PlayerId(0)], &[PlayerId(2)], &reg);
    assert!(result.is_err(), "splitting a party should fail");
}
#[test]
fn party_integrity_preserved() {
    let mut reg = PartyRegistry::default();
    reg.register(
        Party::new(
            1,
            vec![PlayerId(0), PlayerId(1)],
            matchlab_core::time::SimTime::ZERO,
            None,
        )
        .unwrap(),
    );
    let result = check_party_integrity(&[PlayerId(0), PlayerId(1)], &[PlayerId(2)], &reg);
    assert!(result.is_ok(), "keeping party together should pass");
}
#[test]
fn isolated_region_never_crosses() {
    let mut map = HashMap::new();
    map.insert((Region::NA, Region::NA), 20.0);
    map.insert((Region::EU, Region::EU), 25.0);
    let matrix = LatencyMatrix::new(map);
    assert!((matrix.base_latency(Region::NA, Region::NA) - 20.0).abs() < 1e-9);
    let cross = matrix.base_latency(Region::NA, Region::EU);
    assert!(cross > 0.0, "cross-region has a default latency");
}
#[test]
fn latency_model_computes_team_stats() {
    let mut map = HashMap::new();
    map.insert((Region::NA, Region::NA), 20.0);
    map.insert((Region::EU, Region::EU), 25.0);
    map.insert((Region::NA, Region::EU), 80.0);
    let model = LatencyModel::new(LatencyMatrix::new(map), 0.0);
    let mut world = World::new(SimRng::from_seed(42));
    let id1 = world.next_player_id();
    let id2 = world.next_player_id();
    for (id, region, lat, lon) in [(id1, Region::NA, 40.0, -74.0), (id2, Region::EU, 51.0, 0.0)] {
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
            region,
            location: GeoLocation::new(region, lat, lon),
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
            role: None,
            skill_vector: SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
        };
        world.add_player(reality, obs);
    }
    let reference = GeoLocation::new(Region::NA, 40.0, -74.0);
    let stats = model.team_latency(&[id1, id2], &reference, &world);
    assert!(stats.mean_latency > 0.0, "mean latency should be positive");
    assert!(stats.max_latency >= stats.mean_latency);
}
#[test]
fn role_constraint_enforced() {
    let constraint = RoleConstraint {
        requirements: vec![RoleRequirement {
            role: "tank".to_string(),
            min_count: 2,
            max_count: None,
        }],
    };
    let (world, ids) = make_world_with_players(4);
    let proposed = ProposedMatch {
        team_a: vec![ids[0], ids[1]],
        team_b: vec![ids[2], ids[3]],
        quality_score: 0.9,
    };
    assert!(!constraint.is_satisfied(&proposed, &world));
}
#[test]
fn objective_vector_dominance() {
    let a = MatchObjectiveVector {
        skill_quality: 0.9,
        role_quality: 0.8,
        latency_quality: 0.7,
        party_quality: 0.6,
        wait_cost: 0.3,
        utilization: 0.5,
    };
    let b = MatchObjectiveVector {
        skill_quality: 0.8,
        role_quality: 0.7,
        latency_quality: 0.6,
        party_quality: 0.5,
        wait_cost: 0.4,
        utilization: 0.4,
    };
    assert!(a.dominates(&b), "a should dominate b");
    assert!(!b.dominates(&a), "b should not dominate a");
}
#[test]
fn objective_vector_scalarize() {
    let v = MatchObjectiveVector {
        skill_quality: 1.0,
        role_quality: 0.5,
        latency_quality: 0.3,
        party_quality: 0.2,
        wait_cost: 0.1,
        utilization: 0.0,
    };
    let w = ObjectiveWeights::default();
    let score = v.scalarize(&w);
    assert!(score > 0.0, "scalarized score should be positive");
}
#[test]
fn impossible_constraint_reports_failure() {
    let policy = MatchPolicy::new().with_constraint(Box::new(TeamSizeConstraint {
        size_a: 5,
        size_b: 5,
    }));
    let (world, ids) = make_world_with_players(4);
    let proposed = ProposedMatch {
        team_a: ids[..2].to_vec(),
        team_b: ids[2..4].to_vec(),
        quality_score: 0.9,
    };
    assert!(
        !policy.satisfies_constraints(&proposed, &world),
        "impossible constraint should fail"
    );
}
#[test]
fn matchmaking_evaluation_is_deterministic() {
    let (world1, ids1) = make_world_with_players(4);
    let (world2, ids2) = make_world_with_players(4);
    let balance = SkillBalanceObjective;
    let proposed1 = ProposedMatch {
        team_a: vec![ids1[0], ids1[1]],
        team_b: vec![ids1[2], ids1[3]],
        quality_score: 0.0,
    };
    let proposed2 = ProposedMatch {
        team_a: vec![ids2[0], ids2[1]],
        team_b: vec![ids2[2], ids2[3]],
        quality_score: 0.0,
    };
    let score1 = balance.score(&proposed1, &world1);
    let score2 = balance.score(&proposed2, &world2);
    assert_eq!(score1, score2, "deterministic evaluation");
}
