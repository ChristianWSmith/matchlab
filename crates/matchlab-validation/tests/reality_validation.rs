//! Reality model validation: comprehensive acceptance tests for
//! the new reality layer — distribution tests, correlation tests, dynamics
//! tests, noise tests, dimensionality tests, information-boundary tests, and
//! team-model tests.
use matchlab_core::player::{PlayerId, SkillVector};
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;
use matchlab_game::performance::{GaussianNoiseModel, PerformanceContext, PerformanceModel};
use matchlab_game::team::{AdditiveTeamModel, ComplementaryTeamModel, TeamContext, TeamModel};
use matchlab_players::distribution::{
    DistributionConfig, Marginal, SkillDistribution, draw_skill_vector, pearson_correlation,
};
use matchlab_players::dynamics::{
    DecayDynamics, DynamicsContext, ExperienceDynamics, LinearDynamics, SkillDynamics,
    StationaryDynamics,
};
use std::collections::HashMap;
#[test]
fn population_converges_to_configured_mean_and_stddev() {
    let dist = SkillDistribution::Independent(vec![Marginal {
        name: "skill".to_string(),
        distribution: DistributionConfig::Normal {
            mean: 1200.0,
            stddev: 200.0,
        },
    }]);
    let mut rng = SimRng::from_seed(42);
    let mut values = Vec::new();
    for _ in 0..10000 {
        let sv = draw_skill_vector(&dist, &mut rng);
        values.push(sv.dimensions["skill"]);
    }
    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let variance: f64 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();
    assert!(
        (mean - 1200.0).abs() < 20.0,
        "mean should be near 1200: {mean}"
    );
    assert!(
        (stddev - 200.0).abs() < 30.0,
        "stddev should be near 200: {stddev}"
    );
}
#[test]
fn independent_dimensions_produce_near_zero_correlation() {
    let dist = SkillDistribution::Independent(vec![
        Marginal {
            name: "a".to_string(),
            distribution: DistributionConfig::Normal {
                mean: 0.0,
                stddev: 10.0,
            },
        },
        Marginal {
            name: "b".to_string(),
            distribution: DistributionConfig::Normal {
                mean: 0.0,
                stddev: 10.0,
            },
        },
    ]);
    let mut rng = SimRng::from_seed(42);
    let mut a_vals = Vec::new();
    let mut b_vals = Vec::new();
    for _ in 0..1000 {
        let sv = draw_skill_vector(&dist, &mut rng);
        a_vals.push(sv.dimensions["a"]);
        b_vals.push(sv.dimensions["b"]);
    }
    let r = pearson_correlation(&a_vals, &b_vals);
    assert!(r.abs() < 0.1, "independent dimensions: r = {r}");
}
#[test]
fn correlated_dimensions_produce_configured_correlation() {
    let dist = SkillDistribution::MultivariateNormal {
        means: vec![0.0, 0.0],
        covariance: vec![vec![100.0, 60.0], vec![60.0, 100.0]],
        dimension_names: vec!["a".to_string(), "b".to_string()],
    };
    let mut rng = SimRng::from_seed(42);
    let mut a_vals = Vec::new();
    let mut b_vals = Vec::new();
    for _ in 0..5000 {
        let sv = draw_skill_vector(&dist, &mut rng);
        a_vals.push(sv.dimensions["a"]);
        b_vals.push(sv.dimensions["b"]);
    }
    let r = pearson_correlation(&a_vals, &b_vals);
    assert!(
        (r - 0.6).abs() < 0.05,
        "configured correlation 0.6, got r = {r}"
    );
}
#[test]
fn linear_dynamics_produces_expected_trajectory() {
    let dyn_model = LinearDynamics {
        improvement_rate: 5.0,
        decline_rate: 0.0,
        volatility: 0.0,
    };
    let sv = SkillVector::one_dimensional(100.0);
    let ctx = DynamicsContext::default();
    let mut rng = SimRng::from_seed(42);
    let mut skill = sv.clone();
    for _ in 0..10 {
        skill = dyn_model.advance(&skill, &ctx, &mut rng);
    }
    assert!((skill.overall() - 150.0).abs() < 1e-9);
}
#[test]
fn experience_dynamics_converges_to_plateau() {
    let dyn_model = ExperienceDynamics {
        learning_rate: 10.0,
        plateau_games: 100,
        volatility: 0.0,
    };
    let sv = SkillVector::one_dimensional(100.0);
    let mut rng = SimRng::from_seed(42);
    let mut skill = sv.clone();
    for i in 0..200 {
        let ctx = DynamicsContext {
            games_played: i,
            ..Default::default()
        };
        skill = dyn_model.advance(&skill, &ctx, &mut rng);
    }
    assert!(skill.overall() > 500.0);
}
#[test]
fn decay_dynamics_reduces_after_inactivity() {
    let dyn_model = DecayDynamics {
        decay_rate: 0.1,
        inactive_threshold: 0,
        min_skill: 100.0,
    };
    let sv = SkillVector::one_dimensional(1000.0);
    let ctx = DynamicsContext {
        time_inactive: 10,
        ..Default::default()
    };
    let mut rng = SimRng::from_seed(42);
    let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
    assert!(new_sv.overall() < 500.0, "decay should reduce skill");
}
#[test]
fn stationary_dynamics_preserves_skill() {
    let dyn_model = StationaryDynamics;
    let sv = SkillVector::one_dimensional(1200.0);
    let ctx = DynamicsContext::default();
    let mut rng = SimRng::from_seed(42);
    let new_sv = dyn_model.advance(&sv, &ctx, &mut rng);
    assert!((new_sv.overall() - 1200.0).abs() < 1e-9);
}
#[test]
fn zero_noise_deterministic() {
    let model = GaussianNoiseModel::deterministic();
    let sv = SkillVector::one_dimensional(1500.0);
    let ctx = PerformanceContext::default();
    let mut rng = SimRng::from_seed(42);
    let perf1 = model.realize(&sv, &ctx, &mut rng);
    let mut rng = SimRng::from_seed(42);
    let perf2 = model.realize(&sv, &ctx, &mut rng);
    assert!((perf1.overall() - perf2.overall()).abs() < 1e-12);
}
#[test]
fn increasing_noise_increases_outcome_variance() {
    let sv = SkillVector::one_dimensional(1000.0);
    let ctx = PerformanceContext::default();
    let mut rng = SimRng::from_seed(42);
    let low_noise = GaussianNoiseModel::new(0.01);
    let high_noise = GaussianNoiseModel::new(0.1);
    let low_perfs: Vec<f64> = (0..1000)
        .map(|_| low_noise.realize(&sv, &ctx, &mut rng).overall())
        .collect();
    let high_perfs: Vec<f64> = (0..1000)
        .map(|_| high_noise.realize(&sv, &ctx, &mut rng).overall())
        .collect();
    let low_var: f64 = {
        let mean = low_perfs.iter().sum::<f64>() / low_perfs.len() as f64;
        low_perfs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / low_perfs.len() as f64
    };
    let high_var: f64 = {
        let mean = high_perfs.iter().sum::<f64>() / high_perfs.len() as f64;
        high_perfs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / high_perfs.len() as f64
    };
    assert!(
        high_var > low_var,
        "higher noise should produce higher variance: low={low_var}, high={high_var}"
    );
}
#[test]
fn scalar_skill_vector_matches_existing_behavior() {
    let sv = SkillVector::one_dimensional(1200.0);
    assert_eq!(sv.ndim(), 1);
    assert_eq!(sv.overall(), 1200.0);
    assert_eq!(sv.dimensions["overall"], 1200.0);
}
#[test]
fn multidim_overall_is_mean_across_dimensions() {
    let mut dims = HashMap::new();
    dims.insert("a".to_string(), 100.0);
    dims.insert("b".to_string(), 200.0);
    dims.insert("c".to_string(), 300.0);
    let sv = SkillVector { dimensions: dims };
    assert!((sv.overall() - 200.0).abs() < 1e-9);
}
#[test]
fn additive_team_model_recovers_sum() {
    let model = AdditiveTeamModel;
    let ctx = TeamContext {
        team_size: 3,
        role_requirements: None,
    };
    let players = vec![
        SkillVector::one_dimensional(100.0),
        SkillVector::one_dimensional(200.0),
        SkillVector::one_dimensional(300.0),
    ];
    assert!((model.team_strength(&players, &ctx) - 600.0).abs() < 1e-9);
}
#[test]
fn complementary_team_model_rewards_diversity() {
    let model = ComplementaryTeamModel {
        synergy_bonus: 0.2,
        complementarity_threshold: 0.3,
    };
    let ctx = TeamContext {
        team_size: 2,
        role_requirements: None,
    };
    let diverse = vec![
        SkillVector::one_dimensional(100.0),
        SkillVector::one_dimensional(300.0),
    ];
    let similar = vec![
        SkillVector::one_dimensional(200.0),
        SkillVector::one_dimensional(210.0),
    ];
    let s_diverse = model.team_strength(&diverse, &ctx);
    let s_similar = model.team_strength(&similar, &ctx);
    assert!(s_diverse > s_similar, "diverse should have higher strength");
}
#[test]
fn world_records_skill_history() {
    let mut world = World::new(SimRng::from_seed(42));
    let id = PlayerId(0);
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
        rating: 1000.0,
        hidden_mmr: 1000.0,
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
        skill_vector: SkillVector::one_dimensional(1000.0),
        detection_flags: Vec::new(),
    };
    world.add_player(reality, obs);
    world.record_skill_snapshot(id, SkillVector::one_dimensional(1100.0));
    assert!(world.has_history(id));
    assert_eq!(world.skill_history(id).len(), 1);
}
