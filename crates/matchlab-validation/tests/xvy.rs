//! XvY 1v4 analytical baselines (ticket T-09).
//!
//! Dead-by-Daylight-style asymmetric matchmaking: one killer (team A, size 1)
//! vs four survivors (team B, size 4), roles enforced by the T-08 role-aware
//! batch matchmaker. These baselines prove match quality on unequal team sizes
//! matches the analytic formula, that role-gated formation genuinely gates
//! (killer-only ⇒ stall), and that formed matches never slip back to the
//! counts-only fallback.

use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_matchmaking::queue::Queue;
use matchlab_metrics::collector::MetricCollector;
use matchlab_metrics::{MetricResult, engine::MetricsEngine};
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_validation::{build_loop, observation, queue_entry};

fn dbd_teams() -> TeamComposition {
    TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    }
}

fn role_entry(
    id: u64,
    joined_secs: f64,
    rating: f64,
    role: &str,
) -> matchlab_matchmaking::queue::QueueEntry {
    let mut e = queue_entry(id, SimTime::from_secs(joined_secs), rating);
    e.role = Some(role.to_string());
    e
}

fn batch() -> LuaMatchmaker {
    LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap()
}

fn world_with(ratings: &[(u64, f64)]) -> matchlab_core::world::World {
    let mut world = matchlab_core::world::World::new(SimRng::from_seed(1));
    for &(id, rating) in ratings {
        world
            .observations
            .insert(PlayerId(id), observation(id, rating));
    }
    world
}

fn killer_archetype() -> ArchetypeConfig {
    ArchetypeConfig {
        name: "killer".to_string(),
        proportion: 0.18,
        skill_distribution: DistributionConfig::Normal {
            mean: 1250.0,
            stddev: 250.0,
        },
        skill_volatility: 5.0,
        improvement_rate: 0.0,
        play_frequency: 0.9,
        session_length: 2400.0,
        quit_probability: 0.005,
        initial_rating: Some(1200.0),
        role: Some("killer".to_string()),
    }
}

fn survivor_archetype() -> ArchetypeConfig {
    ArchetypeConfig {
        name: "survivor".to_string(),
        proportion: 0.82,
        skill_distribution: DistributionConfig::Normal {
            mean: 1000.0,
            stddev: 100.0,
        },
        skill_volatility: 5.0,
        improvement_rate: 0.0,
        play_frequency: 0.7,
        session_length: 1800.0,
        quit_probability: 0.01,
        initial_rating: Some(1000.0),
        role: Some("survivor".to_string()),
    }
}

fn dbd_population(size: u64, seed: u64) -> Vec<(PlayerReality, PlayerObservation)> {
    let config = PopulationConfig {
        size,
        archetypes: vec![killer_archetype(), survivor_archetype()],
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    realities.into_iter().zip(observations).collect::<Vec<_>>()
}

#[test]
fn uniform_1v4_quality_is_analytic_one() {
    // Uniform killers (1000) + uniform survivors (1000): every batch match is
    // 1 killer vs 4 survivors of identical rating → quality exactly 1.0.
    let mut queue = Queue::default();
    queue.enqueue(role_entry(1, 1.0, 1000.0, "killer"));
    queue.enqueue(role_entry(2, 2.0, 1000.0, "killer"));
    for id in 3..=10 {
        queue.enqueue(role_entry(id, id as f64, 1000.0, "survivor"));
    }
    let world = world_with(&(1..=10u64).map(|id| (id, 1000.0)).collect::<Vec<_>>());
    let mm = batch();
    let mut rng = SimRng::from_seed(7);
    let matches = mm.find_matches(&queue, &world, &dbd_teams(), SimTime::ZERO, &mut rng);
    assert_eq!(matches.len(), 2, "2 killers × 4 survivors = 2 matches");
    for m in &matches {
        assert_eq!(m.team_a.len(), 1);
        assert_eq!(m.team_b.len(), 4);
        assert!(
            (m.quality_score - 1.0).abs() < 1e-9,
            "uniform 1v4 quality = {}",
            m.quality_score
        );
    }
}

#[test]
fn known_gap_1v4_quality_matches_analytic() {
    // Killers rated 1500, survivors 1400: |avg_a − avg_b| = 100 → quality
    // 1 − 100/400 = 0.75 for every formed match.
    let mut queue = Queue::default();
    queue.enqueue(role_entry(1, 1.0, 1500.0, "killer"));
    queue.enqueue(role_entry(2, 2.0, 1500.0, "killer"));
    for (i, id) in (3..=10u64).enumerate() {
        queue.enqueue(role_entry(id, i as f64 + 3.0, 1400.0, "survivor"));
    }
    let mut world_iter_ratings: Vec<(u64, f64)> = Vec::new();
    world_iter_ratings.extend([(1, 1500.0), (2, 1500.0)]);
    world_iter_ratings.extend((3..=10u64).map(|id| (id, 1400.0)));
    let world = world_with(&world_iter_ratings);
    let mm = batch();
    let mut rng = SimRng::from_seed(7);
    let matches = mm.find_matches(&queue, &world, &dbd_teams(), SimTime::ZERO, &mut rng);
    assert_eq!(matches.len(), 2);
    let analytic = 1.0 - 100.0 / 400.0; // 0.75
    for m in &matches {
        assert!(
            (m.quality_score - analytic).abs() < 1e-9,
            "known-gap 1v4 quality {} vs analytic {}",
            m.quality_score,
            analytic
        );
    }
}

/// Test-only guard: counts every formed match whose role composition violates
/// the 1v4 contract (must be exactly 1 killer / 4 survivors).
#[derive(Default)]
struct RoleCompositionGuard {
    checked: u64,
    violations: u64,
}

impl MetricCollector for RoleCompositionGuard {
    fn name(&self) -> &str {
        "role_composition_guard"
    }

    fn record_match(
        &mut self,
        mr: &matchlab_core::match_::MatchResult,
        world: &matchlab_core::world::World,
    ) {
        self.checked += 1;
        let role = |pid: &PlayerId| world.observe(*pid).and_then(|o| o.role.clone());
        let killers_a = mr
            .team_a
            .iter()
            .filter(|p| role(p).as_deref() == Some("killer"))
            .count();
        let survivors_b = mr
            .team_b
            .iter()
            .filter(|p| role(p).as_deref() == Some("survivor"))
            .count();
        if killers_a != 1 || survivors_b != 4 || mr.team_a.len() != 1 || mr.team_b.len() != 4 {
            self.violations += 1;
        }
    }

    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(self.violations as f64);
        }
        MetricResult::Scalar(self.violations as f64 / self.checked as f64)
    }
}

#[test]
fn dbd_loop_forms_only_1v4_role_matches() {
    // Real loop run on a DbD population: every formed match must contain
    // exactly one killer and four survivors (the T-08 role path, not the
    // counts-only fallback). The guard sees every formed match, not a sample.
    let population = dbd_population(220, 7);
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(RoleCompositionGuard::default()));
    let mut loop_ = build_loop(population, dbd_teams(), 300, 42, metrics);
    loop_.run_until(SimTime::from_secs(3600.0));

    let (completed, violation_rate) = {
        let mut state = loop_.state.lock().unwrap();
        state.metrics.finalize();
        let rate = match state.metrics.results().get("role_composition_guard") {
            Some(MetricResult::Scalar(v)) => *v,
            other => panic!("expected scalar, got {other:?}"),
        };
        (state.matches_completed, rate)
    };
    assert!(completed > 0, "1v4 loop must complete matches");
    assert_eq!(
        violation_rate, 0.0,
        "every formed match must be 1 killer + 4 survivors"
    );
}

#[test]
fn killer_only_population_stalls() {
    // Role-stall invariant: a population containing no survivors can never
    // fill team B, so the loop forms zero matches and the queue grows instead.
    let only_killers = killer_archetype();
    let config = PopulationConfig {
        size: 60,
        archetypes: vec![only_killers],
    };
    let mut rng = SimRng::from_seed(3);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    let population: Vec<(PlayerReality, PlayerObservation)> =
        realities.into_iter().zip(observations).collect();

    let metrics = MetricsEngine::new();
    let mut loop_ = build_loop(population, dbd_teams(), 1_000, 42, metrics);
    loop_.run_until(SimTime::from_secs(600.0));

    let (formed, completed, queued) = {
        let state = loop_.state.lock().unwrap();
        (
            state.matches_formed(),
            state.matches_completed,
            state.queue.len(),
        )
    };
    assert_eq!(formed, 0, "no survivors ⇒ no matches can form");
    assert_eq!(completed, 0);
    assert!(queued > 0, "killer-only queue must keep players waiting");
}

#[test]
fn dbd_experiment_is_deterministic() {
    use matchlab_experiments::inherit;
    use matchlab_experiments::runner::ExperimentRunner;

    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../experiments/dbd_1v4.yaml");
    let config = inherit::load(&manifest).unwrap();
    let a = ExperimentRunner::run(&config).unwrap();
    let b = ExperimentRunner::run(&config).unwrap();

    assert_eq!(a.matches_completed, b.matches_completed);
    assert!(
        a.matches_completed > 0,
        "dbd_1v4 must complete matches (acceptance log)"
    );
    for (name, av) in &a.metrics {
        assert_eq!(
            av, &b.metrics[name],
            "metric {name} diverges across same-seed runs"
        );
    }
}
