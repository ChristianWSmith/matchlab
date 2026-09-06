//! T-05 dynamic-skill baselines: exact trajectory, response, determinism.
//!
//! These drive a `MatchLoop` directly (the validation crate is the legitimate
//! reality reader) and assert the periodic skill-advancement semantics of
//! spec §5.6 defined by `skill_update_interval_secs`.

use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerId, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;

/// The standard elo+logistic(noise 0)+batch stack.
fn build_stack() -> (
    Box<dyn matchlab_rating::system::RatingSystem>,
    LuaOutcomeModel,
    LuaMatchmaker,
) {
    let rating = registry::from_name(
        "elo",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .unwrap();
    let outcome = LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &serde_yaml::from_str("beta: 400.0\nnoise: 0.0").unwrap(),
    )
    .unwrap();
    let matchmaker =
        LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap();
    (rating, outcome, matchmaker)
}

/// Half the population is `improving` (skill `s0` drifting up `k`/interval,
/// volatility 0), half `stable` (never drifts). PopulationGenerator assigns
/// contiguous id ranges per archetype, so improvers own ids `0..size/2`.
fn mixed_population(
    total: u64,
    improving_proportion: f64,
    s0: f64,
    k: f64,
) -> Vec<(PlayerReality, matchlab_core::player::PlayerObservation)> {
    let config = PopulationConfig {
        size: total,
        archetypes: vec![
            ArchetypeConfig {
                name: "improving".to_string(),
                proportion: improving_proportion,
                skill_distribution: DistributionConfig::Normal {
                    mean: s0,
                    stddev: 0.0,
                },
                skill_volatility: 0.0,
                improvement_rate: k,
                play_frequency: 1.0,
                session_length: 1800.0,
                quit_probability: 0.0,
                initial_rating: Some(s0),
            },
            ArchetypeConfig {
                name: "stable".to_string(),
                proportion: 1.0 - improving_proportion,
                skill_distribution: DistributionConfig::Normal {
                    mean: s0,
                    stddev: 0.0,
                },
                skill_volatility: 0.0,
                improvement_rate: 0.0,
                play_frequency: 1.0,
                session_length: 1800.0,
                quit_probability: 0.0,
                initial_rating: Some(s0),
            },
        ],
    };
    let mut rng = SimRng::from_seed(7);
    let (r, o) = PopulationGenerator::generate(&config, &mut rng);
    r.into_iter().zip(o).collect()
}

fn new_loop(
    population: Vec<(PlayerReality, matchlab_core::player::PlayerObservation)>,
    interval_secs: f64,
    team_size: usize,
    max_matches: u64,
    max_time_secs: f64,
    seed: u64,
) -> MatchLoop {
    let (rating, outcome, matchmaker) = build_stack();
    let config = LoopConfig {
        teams: TeamComposition {
            team_size_a: team_size,
            team_size_b: team_size,
            role_a: None,
            role_b: None,
        },
        batch_interval_ticks: 1,
        rejoin_delay: SimTime::from_secs(0.0),
        max_matches,
        skill_update_interval: Some(SimTime::from_secs(interval_secs)),
    };
    let mut loop_ = MatchLoop::new(
        population,
        rating,
        Box::new(outcome),
        Box::new(matchmaker),
        MetricsEngine::new(),
        config,
        seed,
    );
    loop_.run_until(SimTime::from_secs(max_time_secs));
    loop_
}

fn improver_ids(total: u64) -> std::ops::Range<u64> {
    0..(total / 2)
}

fn mean_skill(loop_: &MatchLoop, ids: &std::ops::Range<u64>) -> f64 {
    let mut sum = 0.0;
    let mut n = 0u64;
    for pid in ids.clone() {
        if let Some(r) = loop_.world.players.get(&PlayerId(pid)) {
            sum += r.skill.overall();
            n += 1;
        }
    }
    sum / n.max(1) as f64
}

fn mean_rating(loop_: &MatchLoop, ids: &std::ops::Range<u64>) -> f64 {
    let mut sum = 0.0;
    let mut n = 0u64;
    for pid in ids.clone() {
        if let Some(o) = loop_.world.observe(PlayerId(pid)) {
            sum += o.rating;
            n += 1;
        }
    }
    sum / n.max(1) as f64
}

#[test]
fn dynamic_skill_tracks_exact_trajectory() {
    let total = 40u64;
    let s0 = 1000.0;
    let k = 2.0;
    let run = 40.0;
    let population = mixed_population(total, 0.5, s0, k);
    let loop_ = new_loop(population, 1.0, 1, 10_000, run, 9);

    for pid in improver_ids(total) {
        let r = loop_
            .world
            .players
            .get(&PlayerId(pid))
            .expect("improver present");
        assert_eq!(
            r.skill.overall(),
            s0 + k * run,
            "improving player {pid} should sit exactly at S0 + k*t"
        );
    }
    for pid in (total / 2)..total {
        let r = loop_
            .world
            .players
            .get(&PlayerId(pid))
            .expect("stable present");
        assert_eq!(r.skill.overall(), s0, "stable player {pid} must not drift");
    }
}

#[test]
fn dynamic_skill_rating_responds_to_drift() {
    // The improving class's rating must track its rising skill: improvers end
    // strictly above their starting rating, stables strictly below it (Elo
    // redistributes the pool as the improvers pull away). The measured
    // response lag (skill − rating) is recorded as a descriptive result: with
    // matches resolving every ~1800s and k=3/s, the rating cannot keep pace
    // with a continuously-moving target, so the lag grows — this is not a
    // failure of the assertion here, merely the documented behavior.
    let total = 40u64;
    let s0 = 1000.0;
    let k = 3.0;
    let horizon = 172_800.0; // 2 days of sim time
    let population = mixed_population(total, 0.5, s0, k);
    let loop_ = new_loop(population, 1.0, 1, 10_000, horizon, 5);

    let improvers = improver_ids(total);
    let stables = (total / 2)..total;

    let improv_skill = mean_skill(&loop_, &improvers);
    let improv_rating = mean_rating(&loop_, &improvers);
    let stable_rating = mean_rating(&loop_, &stables);

    assert!(
        (improv_skill - (s0 + k * horizon)).abs() < 1e-6,
        "improvers must reach exactly S0 + k*t, got {improv_skill}"
    );
    assert!(
        improv_rating > s0,
        "improver ratings must rise with skill: {improv_rating}"
    );
    assert!(
        stable_rating < s0,
        "stable ratings must fall as improvers pull away: {stable_rating}"
    );

    // Descriptive lag record (spec §18.6); no monotonicity assertion is made.
    let lag = improv_skill - improv_rating;
    assert!(lag > 0.0, "rating must trail rising skill, lag {lag}");
}

#[test]
fn dynamic_skill_is_deterministic() {
    let total = 40u64;
    let run = 30.0;
    let a = new_loop(
        mixed_population(total, 0.5, 1000.0, 2.0),
        1.0,
        1,
        10_000,
        run,
        11,
    );
    let b = new_loop(
        mixed_population(total, 0.5, 1000.0, 2.0),
        1.0,
        1,
        10_000,
        run,
        11,
    );
    for pid in 0..total {
        assert_eq!(
            a.world
                .players
                .get(&PlayerId(pid))
                .map(|r| r.skill.overall()),
            b.world
                .players
                .get(&PlayerId(pid))
                .map(|r| r.skill.overall())
        );
        assert_eq!(
            a.world.observe(PlayerId(pid)).map(|o| o.rating),
            b.world.observe(PlayerId(pid)).map(|o| o.rating)
        );
    }
}

#[test]
fn skill_stays_static_without_interval_flag() {
    // Negative control: improvement_rate is set but no interval flag ⇒ no drift.
    let total = 40u64;
    let s0 = 1000.0;
    let population = mixed_population(total, 0.5, s0, 5.0);
    let (rating, outcome, matchmaker) = build_stack();
    let config = LoopConfig {
        teams: TeamComposition {
            team_size_a: 1,
            team_size_b: 1,
            role_a: None,
            role_b: None,
        },
        batch_interval_ticks: 1,
        rejoin_delay: SimTime::from_secs(0.0),
        max_matches: 10_000,
        skill_update_interval: None,
    };
    let mut loop_ = MatchLoop::new(
        population,
        rating,
        Box::new(outcome),
        Box::new(matchmaker),
        MetricsEngine::new(),
        config,
        13,
    );
    loop_.run_until(SimTime::from_secs(30.0));
    for pid in 0..total {
        let r = loop_.world.players.get(&PlayerId(pid)).unwrap();
        assert_eq!(
            r.skill.overall(),
            s0,
            "no interval flag ⇒ skill must be static"
        );
    }
}
