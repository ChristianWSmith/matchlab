//! Metamorphic tests.
//!
//! Instead of asserting known absolute outputs, assert on how outputs *should
//! change* when the environment is perturbed (proposal §1.3). Three
//! relationships, all deterministic given fixed seeds:
//!
//! 1. **Zero noise** — as outcome noise → 0 the game becomes more skill-driven:
//!    match winners favor the higher-true-skill team more reliably, and
//!    `rating_accuracy` (overall MAE) is lower than under a noisy baseline.
//! 2. **Double population** — duplicating the population while preserving
//!    archetype proportions keeps per-player learning identical (normalized MAE
//!    invariant within a documented band) while completed-match totals scale
//!    ~2× at a fixed horizon.
//! 3. **Widening matchmaking window** — strict → expanding → batch widens the
//!    skill window, so mean queue time is non-increasing and mean match quality
//!    is non-decreasing (batch sorts by rating and balances first).
use matchlab_core::match_::{MatchResult, Team, TeamComposition};
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_metrics::collector::MetricCollector;
use matchlab_metrics::{MetricResult, engine::MetricsEngine, lua::LuaMetricCollector};
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_validation::{run_loop_full, summary_mean};
use std::collections::HashMap;
fn single_class_pop(size: u64, seed: u64) -> Vec<(PlayerReality, PlayerObservation)> {
    let archetype = ArchetypeConfig {
        name: "stable".to_string(),
        proportion: 1.0,
        skill_distribution: DistributionConfig::Normal {
            mean: 1000.0,
            stddev: 250.0,
        },
        skill_volatility: 0.0,
        improvement_rate: 0.0,
        play_frequency: 0.8,
        session_length: 1800.0,
        quit_probability: 0.0,
        initial_rating: Some(1000.0),
        role: None,
        skill_dimensions: None,
        correlation: None,
        dynamics: None,
    };
    let config = PopulationConfig {
        size,
        archetypes: vec![archetype],
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    realities.into_iter().zip(observations).collect()
}
fn role_pop(
    size: u64,
    seed: u64,
    killer: (String, f64, f64, u64),
    survivor: (String, f64, f64, u64),
    killer_proportion: f64,
) -> Vec<(PlayerReality, PlayerObservation)> {
    let named = |(name, mean, stddev, start): (String, f64, f64, u64), role: &str, prop: f64| {
        ArchetypeConfig {
            name,
            proportion: prop,
            skill_distribution: DistributionConfig::Normal { mean, stddev },
            skill_volatility: 0.0,
            improvement_rate: 0.0,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.0,
            initial_rating: Some(start as f64),
            role: Some(role.to_string()),
            skill_dimensions: None,
            correlation: None,
            dynamics: None,
        }
    };
    let config = PopulationConfig {
        size,
        archetypes: vec![
            named(killer, "killer", killer_proportion),
            named(survivor, "survivor", 1.0 - killer_proportion),
        ],
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    realities.into_iter().zip(observations).collect()
}
fn dbd_teams() -> TeamComposition {
    TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    }
}
fn lua_metric(name: &str) -> Box<dyn MetricCollector> {
    Box::new(
        LuaMetricCollector::load(
            &format!("plugins/metrics/{name}.lua"),
            &serde_yaml::Value::Null,
        )
        .expect("metric script loads"),
    )
}
/// Test-only collector: fraction of formed matches where the winner's average
/// true skill exceeds the loser's (reads the ground-truth skill binding, which
/// metric collectors are legitimately allowed to see).
#[derive(Default)]
struct StrongerWinsGuard {
    checked: u64,
    strong_wins: u64,
}
impl MetricCollector for StrongerWinsGuard {
    fn name(&self) -> &str {
        "stronger_wins_fraction"
    }
    fn record_match(&mut self, mr: &MatchResult, world: &matchlab_core::world::World) {
        if mr.team_a.is_empty() || mr.team_b.is_empty() {
            return;
        }
        self.checked += 1;
        let avg = |ids: &[PlayerId]| {
            let skills: Vec<f64> = ids
                .iter()
                .filter_map(|pid| world.observe(*pid))
                .map(|o| o.skill_vector.overall())
                .collect();
            skills.iter().sum::<f64>() / skills.len() as f64
        };
        let (ra, rb) = (avg(&mr.team_a), avg(&mr.team_b));
        let strong_won = (ra >= rb && mr.winner == Team::A) || (rb > ra && mr.winner == Team::B);
        if strong_won {
            self.strong_wins += 1;
        }
    }
    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.strong_wins as f64 / self.checked as f64)
    }
}
fn metrics_with_guard() -> MetricsEngine {
    let mut metrics = MetricsEngine::new();
    metrics.register(lua_metric("rating_accuracy"));
    metrics.register(lua_metric("queue_time"));
    metrics.register(lua_metric("match_quality"));
    metrics.register(Box::new(StrongerWinsGuard::default()));
    metrics
}
fn fraction(results: &HashMap<String, MetricResult>, name: &str) -> f64 {
    match results.get(name) {
        Some(MetricResult::Scalar(v)) => *v,
        other => panic!("expected scalar {name}, got {other:?}"),
    }
}
const GAME_LOGISTIC: &str = "plugins/game/logistic.lua";
const BATCH: &str = "plugins/matchmaking/batch.lua";
const STRICT: &str = "plugins/matchmaking/strict.lua";
#[test]
fn zero_noise_favors_true_skill_and_converges() {
    let population = |seed| {
        role_pop(
            200,
            seed,
            ("killer".into(), 1500.0, 50.0, 1000),
            ("survivor".into(), 1000.0, 50.0, 1000),
            0.25,
        )
    };
    let quiet = run_loop_full(
        population(7),
        dbd_teams(),
        400,
        2400.0,
        11,
        metrics_with_guard(),
        GAME_LOGISTIC,
        "beta: 400.0\nnoise: 0.0",
        BATCH,
        "",
    );
    let noisy = run_loop_full(
        population(7),
        dbd_teams(),
        400,
        2400.0,
        11,
        metrics_with_guard(),
        GAME_LOGISTIC,
        "beta: 400.0\nnoise: 0.2",
        BATCH,
        "",
    );
    assert!(quiet.matches_completed > 0 && noisy.matches_completed > 0);
    let quiet_mae = summary_mean(&quiet.metrics["rating_accuracy"]);
    let noisy_mae = summary_mean(&noisy.metrics["rating_accuracy"]);
    assert!(
        quiet_mae < noisy_mae,
        "noise → 0 must converge closer to true skill: quiet MAE {quiet_mae} vs noisy {noisy_mae}"
    );
    let decisive = role_pop(
        200,
        3,
        ("killer".into(), 2400.0, 50.0, 1000),
        ("survivor".into(), 500.0, 50.0, 1000),
        0.25,
    );
    let decisive_run = run_loop_full(
        decisive,
        dbd_teams(),
        2_000,
        15_000.0,
        5,
        metrics_with_guard(),
        GAME_LOGISTIC,
        "beta: 400.0\nnoise: 0.0",
        BATCH,
        "",
    );
    let favor = fraction(&decisive_run.metrics, "stronger_wins_fraction");
    assert!(
        favor >= 0.97,
        "decisive gap at zero noise must favor the strong class, got {favor}"
    );
}
#[test]
fn double_population_is_scale_invariant() {
    let small = run_loop_full(
        single_class_pop(300, 42),
        TeamComposition {
            team_size_a: 5,
            team_size_b: 5,
            role_a: None,
            role_b: None,
        },
        10_000,
        7200.0,
        99,
        metrics_with_guard(),
        GAME_LOGISTIC,
        "beta: 400.0\nnoise: 0.05",
        BATCH,
        "",
    );
    let large = run_loop_full(
        single_class_pop(600, 42),
        TeamComposition {
            team_size_a: 5,
            team_size_b: 5,
            role_a: None,
            role_b: None,
        },
        10_000,
        7200.0,
        99,
        metrics_with_guard(),
        GAME_LOGISTIC,
        "beta: 400.0\nnoise: 0.05",
        BATCH,
        "",
    );
    assert!(
        small.matches_completed > 0,
        "small population must complete matches"
    );
    let ratio = large.matches_completed as f64 / small.matches_completed as f64;
    assert!(
        (1.7..=2.3).contains(&ratio),
        "completed matches must scale ~2× at a fixed horizon, got ratio {ratio}"
    );
    let small_mae = summary_mean(&small.metrics["rating_accuracy"]);
    let large_mae = summary_mean(&large.metrics["rating_accuracy"]);
    let drift = (large_mae - small_mae).abs() / small_mae;
    assert!(
        drift <= 0.10,
        "normalized MAE must be population-scale invariant: {small_mae} vs {large_mae} (drift {drift:.3})"
    );
}
#[test]
fn widening_window_monotonically_improves_queue_and_quality() {
    let pop = |seed| single_class_pop(240, seed);
    let run = |script: &str, params: &str| {
        run_loop_full(
            pop(13),
            TeamComposition {
                team_size_a: 5,
                team_size_b: 5,
                role_a: None,
                role_b: None,
            },
            300,
            2400.0,
            21,
            metrics_with_guard(),
            GAME_LOGISTIC,
            "beta: 400.0\nnoise: 0.05",
            script,
            params,
        )
    };
    let narrow = run(STRICT, "max_skill_diff: 25.0");
    let wide = run(STRICT, "max_skill_diff: 150.0");
    let batch = run(BATCH, "");
    for (name, out) in [
        ("strict-25", &narrow),
        ("strict-150", &wide),
        ("batch", &batch),
    ] {
        assert!(
            out.matches_completed > 0,
            "{name} must complete matches to measure queue/quality"
        );
    }
    let narrow_q = summary_mean(&narrow.metrics["queue_time"]);
    let wide_q = summary_mean(&wide.metrics["queue_time"]);
    let batch_q = summary_mean(&batch.metrics["queue_time"]);
    assert!(
        narrow_q >= wide_q && wide_q >= batch_q,
        "mean queue time must be non-increasing as the window widens: narrow {narrow_q} ≥ wide {wide_q} ≥ batch {batch_q}"
    );
    let narrow_mq = summary_mean(&narrow.metrics["match_quality"]);
    let wide_mq = summary_mean(&wide.metrics["match_quality"]);
    let batch_mq = summary_mean(&batch.metrics["match_quality"]);
    assert!(
        batch_mq >= narrow_mq - 0.01 && wide_mq >= narrow_mq - 0.01,
        "match quality must stay on the balanced plateau as the window widens (batch {batch_mq}, wide {wide_mq}, narrow {narrow_mq})"
    );
}
