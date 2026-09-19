//! Information boundary tests: verify that rating, matchmaking,
//! and detection systems cannot access latent reality (individual skill
//! dimensions) unless explicitly permitted by the information budget.
use matchlab_core::player::{PlayerId, SkillVector};
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;
fn make_world_with_multidim_skill() -> World {
    let mut world = World::new(SimRng::from_seed(42));
    let id = PlayerId(0);
    let reality = matchlab_core::player::PlayerReality {
        id,
        skill: {
            let mut dims = std::collections::HashMap::new();
            dims.insert("aim".to_string(), 1500.0);
            dims.insert("movement".to_string(), 1100.0);
            dims.insert("game_sense".to_string(), 1300.0);
            SkillVector::multidimensional(dims)
        },
        skill_volatility: 5.0,
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
    let observation = matchlab_core::player::PlayerObservation {
        id,
        rating: 1200.0,
        hidden_mmr: 1300.0,
        visible_rank: matchlab_core::player::VisibleRank {
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
        tilt_level: 0.0,
        game_mode: "ranked".to_string(),
        role: None,
        skill_vector: SkillVector::one_dimensional(1300.0),
        detection_flags: Vec::new(),
    };
    world.add_player(reality, observation);
    world
}
/// The observation must NOT expose individual skill dimensions.
/// The observation's skill_vector should only have "overall" (the 1D view).
#[test]
fn observation_exposes_overall_only() {
    let world = make_world_with_multidim_skill();
    let obs = world.observe(PlayerId(0)).unwrap();
    assert_eq!(obs.skill_vector.ndim(), 1);
    assert!(obs.skill_vector.contains_dimension("overall"));
    assert!(!obs.skill_vector.contains_dimension("aim"));
}
/// The reality holds the full multidimensional skill.
#[test]
fn reality_holds_full_skill_vector() {
    let world = make_world_with_multidim_skill();
    let reality = world.reality(PlayerId(0)).unwrap();
    assert_eq!(reality.skill.ndim(), 3);
    assert!(reality.skill.contains_dimension("aim"));
    assert!(reality.skill.contains_dimension("movement"));
    assert!(reality.skill.contains_dimension("game_sense"));
}
/// Rating/matchmaking systems must use `world.observe()`, never `world.reality()`.
/// This test demonstrates that the observation layer is the correct access path.
#[test]
fn observe_returns_observation_not_reality() {
    let world = make_world_with_multidim_skill();
    let obs = world.observe(PlayerId(0)).unwrap();
    assert_eq!(obs.rating, 1200.0);
    assert_eq!(obs.hidden_mmr, 1300.0);
    let reality = world.reality(PlayerId(0)).unwrap();
    assert_eq!(reality.skill.overall(), (1500.0 + 1100.0 + 1300.0) / 3.0);
}
/// The simulation/metric layer CAN access reality directly.
/// This is the authorized access path for the outcome model and metrics.
#[test]
fn simulation_can_access_reality() {
    let world = make_world_with_multidim_skill();
    let reality = world.reality(PlayerId(0)).unwrap();
    let aim_skill = reality.skill.get_dimension("aim").unwrap();
    assert_eq!(aim_skill, 1500.0);
}
