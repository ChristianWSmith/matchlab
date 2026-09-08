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
            SkillVector { dimensions: dims }
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
        session_history: std::collections::VecDeque::new(),
        quit_history: std::collections::VecDeque::new(),
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
    assert!(obs.skill_vector.dimensions.contains_key("overall"));
    assert!(!obs.skill_vector.dimensions.contains_key("aim"));
}
/// The reality holds the full multidimensional skill.
#[test]
fn reality_holds_full_skill_vector() {
    let world = make_world_with_multidim_skill();
    let reality = world.reality(PlayerId(0)).unwrap();
    assert_eq!(reality.skill.ndim(), 3);
    assert!(reality.skill.dimensions.contains_key("aim"));
    assert!(reality.skill.dimensions.contains_key("movement"));
    assert!(reality.skill.dimensions.contains_key("game_sense"));
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
    let aim_skill = reality.skill.dimensions.get("aim").unwrap();
    assert_eq!(*aim_skill, 1500.0);
}
/// The information budget concept: a struct that controls what systems can see.
#[test]
fn information_budget_concept() {
    let budget_standard = InformationBudget {
        include_skill_vector: true,
        include_skill_dimensions: false,
        include_performance: false,
    };
    let budget_full = InformationBudget {
        include_skill_vector: true,
        include_skill_dimensions: true,
        include_performance: true,
    };
    assert!(budget_standard.include_skill_vector);
    assert!(!budget_standard.include_skill_dimensions);
    assert!(budget_full.include_skill_dimensions);
    assert!(budget_full.include_performance);
}
/// Information budget struct controlling what systems can observe.
#[derive(Debug, Clone)]
pub struct InformationBudget {
    /// Whether the overall skill_vector is included.
    pub include_skill_vector: bool,
    /// Whether individual skill dimensions (aim, movement, etc.) are included.
    pub include_skill_dimensions: bool,
    /// Whether realized performance is included.
    pub include_performance: bool,
}
impl Default for InformationBudget {
    fn default() -> Self {
        Self {
            include_skill_vector: true,
            include_skill_dimensions: false,
            include_performance: false,
        }
    }
}
/// Build an observation table respecting the information budget.
pub fn build_observation_table(
    reality: &matchlab_core::player::PlayerReality,
    budget: &InformationBudget,
) -> std::collections::HashMap<String, f64> {
    let mut table = std::collections::HashMap::new();
    if budget.include_skill_vector {
        table.insert("skill_overall".to_string(), reality.skill.overall());
    }
    if budget.include_skill_dimensions {
        for (dim, &val) in &reality.skill.dimensions {
            table.insert(format!("skill_{dim}"), val);
        }
    }
    table
}
#[test]
fn build_observation_table_respects_budget() {
    let world = make_world_with_multidim_skill();
    let reality = world.reality(PlayerId(0)).unwrap();
    let standard = InformationBudget::default();
    let table = build_observation_table(reality, &standard);
    assert!(table.contains_key("skill_overall"));
    assert!(!table.contains_key("skill_aim"));
    assert!(!table.contains_key("skill_movement"));
    let full = InformationBudget {
        include_skill_dimensions: true,
        ..Default::default()
    };
    let table = build_observation_table(reality, &full);
    assert!(table.contains_key("skill_overall"));
    assert!(table.contains_key("skill_aim"));
    assert!(table.contains_key("skill_movement"));
    assert!(table.contains_key("skill_game_sense"));
}
