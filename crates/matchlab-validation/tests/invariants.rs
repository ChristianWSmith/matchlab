//! Invariants as deterministic property tests.
//!
//! A small grid of seeds × config shapes is hammered through the loop and the
//! simulation is checked against its own contract on every formed match:
//! team sizes valid, role constraints never leak across pools, no NaN/±inf in
//! metrics or RatingStates, queue times never negative, probability-valued
//! fields stay in [0, 1], no player sits in two simultaneous matches, the
//! population count is conserved, and same-seed runs stay byte-identical.
//!
//! Each guard is also proven live via a negative control: a deliberately
//! violated input must make the assertion fire (never vacuous).
use matchlab_core::match_::{MatchId, MatchResult, Team, TeamComposition};
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
use matchlab_core::rng::{SimRng, StreamSeeds};
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_metrics::collector::MetricCollector;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_metrics::{MetricResult, lua::LuaMetricCollector};
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;
use matchlab_rating::system::{ObservationType, RatingState, RatingSystem};
use matchlab_validation::{build_loop, interleaved_two_class_population, observation};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
const SEED_BASE: u64 = 20240714;
const MAX_MATCHES: u64 = 60;
const HORIZON: SimTime = SimTime(7_200_000_000_000);
const QUEUE_TIME_METRIC: &str = "queue_time";
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
fn killer_archetype() -> ArchetypeConfig {
    ArchetypeConfig {
        name: "killer".to_string(),
        proportion: 0.25,
        skill_distribution: DistributionConfig::Normal {
            mean: 1250.0,
            stddev: 250.0,
        },
        skill_volatility: 0.0,
        improvement_rate: 0.0,
        play_frequency: 0.9,
        session_length: 2400.0,
        quit_probability: 0.0,
        initial_rating: Some(1200.0),
        role: Some("killer".to_string()),
        skill_dimensions: None,
        correlation: None,
        dynamics: None,
    }
}
fn survivor_archetype() -> ArchetypeConfig {
    ArchetypeConfig {
        name: "survivor".to_string(),
        proportion: 0.75,
        skill_distribution: DistributionConfig::Normal {
            mean: 1000.0,
            stddev: 100.0,
        },
        skill_volatility: 0.0,
        improvement_rate: 0.0,
        play_frequency: 0.7,
        session_length: 1800.0,
        quit_probability: 0.0,
        initial_rating: Some(1000.0),
        role: Some("survivor".to_string()),
        skill_dimensions: None,
        correlation: None,
        dynamics: None,
    }
}
fn dbd_pop(size: u64, seed: u64) -> Vec<(PlayerReality, PlayerObservation)> {
    let config = PopulationConfig {
        size,
        archetypes: vec![killer_archetype(), survivor_archetype()],
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    realities.into_iter().zip(observations).collect()
}
fn assert_finite(v: f64, what: &str) {
    assert!(v.is_finite(), "{what} must be finite, got non-finite {v}");
}
fn check_metrics_finite(results: &HashMap<String, MetricResult>) {
    for (name, result) in results {
        match result {
            MetricResult::Scalar(v) => assert_finite(*v, &format!("{name} scalar")),
            MetricResult::Distribution(vs) => {
                for (i, v) in vs.iter().enumerate() {
                    assert_finite(*v, &format!("{name} dist[{i}]"));
                }
            }
            MetricResult::Summary {
                mean,
                median,
                p75,
                p90,
                p95,
                p99,
                stddev,
            } => {
                for (label, v) in [
                    ("mean", mean),
                    ("median", median),
                    ("p75", p75),
                    ("p90", p90),
                    ("p95", p95),
                    ("p99", p99),
                    ("stddev", stddev),
                ] {
                    assert_finite(*v, &format!("{name} {label}"));
                }
            }
            MetricResult::Histogram { buckets } => {
                for (edge, _count) in buckets {
                    assert_finite(*edge, &format!("{name} bucket edge"));
                }
            }
            MetricResult::TimeSeries { bucket_means } => {
                for (i, v) in bucket_means.iter().enumerate() {
                    assert_finite(*v, &format!("{name} bucket[{i}]"));
                }
            }
        }
    }
}
fn check_finite_rating_states(states: &HashMap<PlayerId, RatingState>) {
    for (pid, state) in states {
        assert_finite(state.rating, &format!("player {} rating", pid.0));
        assert_finite(state.rating_deviation, &format!("player {} RD", pid.0));
        assert_finite(state.volatility, &format!("player {} volatility", pid.0));
    }
}
/// A `RatingSystem` wrapper that refuses to let a non-finite state through
/// `update` (the "no NaN in RatingState" invariant, enforced on the live path).
struct SanityRatingSystem {
    inner: Box<dyn RatingSystem>,
}
impl RatingSystem for SanityRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        self.inner.information_budget()
    }
    fn initialize(&self, player_id: PlayerId) -> RatingState {
        let state = self.inner.initialize(player_id);
        check_finite_rating_states(&HashMap::from([(player_id, state.clone())]));
        state
    }
    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let p = self.inner.predict(team_a, team_b);
        assert!((0.0..=1.0).contains(&p), "predict outside [0,1]: {p}");
        p
    }
    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let states = self.inner.update(match_result, observations);
        check_finite_rating_states(&states);
        states
    }
}
fn violations_of(results: &HashMap<String, MetricResult>, name: &str) -> u64 {
    match results.get(name) {
        Some(MetricResult::Scalar(v)) => *v as u64,
        other => panic!("guard metric {name} missing or non-scalar: {other:?}"),
    }
}
/// Every formed match must have exactly the configured team sizes, which must
/// themselves be valid (> 0).
struct TeamSizeGuard {
    team_size_a: usize,
    team_size_b: usize,
    checked: u64,
    violations: u64,
}
impl TeamSizeGuard {
    fn new(team_size_a: usize, team_size_b: usize) -> Self {
        assert!(
            team_size_a > 0 && team_size_b > 0,
            "invalid composition: arena sizes must be positive, got {team_size_a}v{team_size_b}"
        );
        Self {
            team_size_a,
            team_size_b,
            checked: 0,
            violations: 0,
        }
    }
}
impl MetricCollector for TeamSizeGuard {
    fn name(&self) -> &str {
        "invariants_team_size"
    }
    fn record_match(&mut self, mr: &MatchResult, _world: &World) {
        self.checked += 1;
        if mr.team_a.len() != self.team_size_a || mr.team_b.len() != self.team_size_b {
            self.violations += 1;
        }
    }
    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.violations as f64)
    }
}
/// When the composition declares roles, no player from the wrong pool may
/// appear on a side (no cross-pool leak). Roles unset ⇒ any composition ok.
struct RoleConstraintGuard {
    role_a: Option<String>,
    role_b: Option<String>,
    checked: u64,
    violations: u64,
}
impl RoleConstraintGuard {
    fn new(role_a: Option<String>, role_b: Option<String>) -> Self {
        Self {
            role_a,
            role_b,
            checked: 0,
            violations: 0,
        }
    }
}
impl MetricCollector for RoleConstraintGuard {
    fn name(&self) -> &str {
        "invariants_role_constraint"
    }
    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        self.checked += 1;
        let role_of = |pid: &PlayerId| world.observe(*pid).and_then(|o| o.role.clone());
        let leaks = mr.team_a.iter().any(|pid| match &self.role_a {
            Some(expected) => role_of(pid).as_deref() != Some(expected.as_str()),
            None => false,
        }) || mr.team_b.iter().any(|pid| match &self.role_b {
            Some(expected) => role_of(pid).as_deref() != Some(expected.as_str()),
            None => false,
        });
        if leaks {
            self.violations += 1;
        }
    }
    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.violations as f64)
    }
}
/// Probability-valued observable fields must stay in [0, 1].
#[derive(Default)]
struct WinRateGuard {
    checked: u64,
    violations: u64,
}
impl MetricCollector for WinRateGuard {
    fn name(&self) -> &str {
        "invariants_win_rate"
    }
    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        self.checked += 1;
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(o) = world.observe(*pid) {
                if !o.win_rate.is_finite() || !(0.0..=1.0).contains(&o.win_rate) {
                    self.violations += 1;
                }
            }
        }
    }
    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.violations as f64)
    }
}
/// Queue times are always non-negative: no player is formed into a match
/// before they joined the queue.
#[derive(Default)]
struct QueueTimeGuard {
    checked: u64,
    violations: u64,
}
impl MetricCollector for QueueTimeGuard {
    fn name(&self) -> &str {
        "invariants_queue_time"
    }
    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        self.checked += 1;
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(o) = world.observe(*pid) {
                if let Some(joined) = o.queue_joined_at {
                    if joined.0 > world.time.0 {
                        self.violations += 1;
                    }
                    let wait = world.time.duration_since(joined).as_secs_f64();
                    if !wait.is_finite() {
                        self.violations += 1;
                    }
                }
            }
        }
    }
    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.violations as f64)
    }
}
/// Single-match membership: across every formed match, no player_id appears in
/// two simultaneous matches. Formation intervals are derived from the match
/// result's duration and asserted pairwise-vs-previous, per player.
#[derive(Default)]
struct SingleMatchGuard {
    intervals: HashMap<PlayerId, Vec<(f64, f64)>>,
    checked: u64,
    violations: u64,
}
impl MetricCollector for SingleMatchGuard {
    fn name(&self) -> &str {
        "invariants_single_match"
    }
    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        self.checked += 1;
        let start = world.time.as_secs_f64();
        let end = start + mr.duration.as_secs_f64();
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(existing) = self.intervals.get(pid) {
                if existing.iter().any(|&(s, e)| start < e && end > s) {
                    self.violations += 1;
                }
            }
            self.intervals.entry(*pid).or_default().push((start, end));
        }
    }
    fn compute(&self) -> MetricResult {
        if self.checked == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.violations as f64)
    }
}
struct Shape {
    name: &'static str,
    population: fn(u64, u64) -> Vec<(PlayerReality, PlayerObservation)>,
    teams: TeamComposition,
    script: &'static str,
    params: &'static str,
}
fn equal_teams(size: usize) -> TeamComposition {
    TeamComposition {
        team_size_a: size,
        team_size_b: size,
        role_a: None,
        role_b: None,
    }
}
fn dbd_teams() -> TeamComposition {
    TeamComposition {
        team_size_a: 1,
        team_size_b: 4,
        role_a: Some("killer".to_string()),
        role_b: Some("survivor".to_string()),
    }
}
fn shapes() -> Vec<Shape> {
    vec![
        Shape {
            name: "single-class-batch",
            population: single_class_pop,
            teams: equal_teams(5),
            script: "plugins/matchmaking/batch.lua",
            params: "",
        },
        Shape {
            name: "two-class-batch",
            population: |size, seed| {
                interleaved_two_class_population(size, 1500.0, 1000.0, 1000.0, seed)
            },
            teams: equal_teams(5),
            script: "plugins/matchmaking/batch.lua",
            params: "",
        },
        Shape {
            name: "1v4-role-batch",
            population: dbd_pop,
            teams: dbd_teams(),
            script: "plugins/matchmaking/batch.lua",
            params: "",
        },
        Shape {
            name: "single-class-expanding",
            population: single_class_pop,
            teams: equal_teams(5),
            script: "plugins/matchmaking/expanding_window.lua",
            params: "",
        },
        Shape {
            name: "single-class-strict",
            population: single_class_pop,
            teams: equal_teams(5),
            script: "plugins/matchmaking/strict.lua",
            params: "max_skill_diff: 75.0",
        },
    ]
}
/// Build a `MatchLoop` with the standard elo + logistic(noise 0) stack and an
/// explicit matchmaker script (mirrors `matchlab_validation::build_loop`).
fn build_loop_with(
    population: Vec<(PlayerReality, PlayerObservation)>,
    teams: TeamComposition,
    max_matches: u64,
    seed: u64,
    metrics: MetricsEngine,
    script: &str,
    params: &str,
) -> MatchLoop {
    let rating = registry::from_name(
        "elo",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .expect("elo loads");
    let rating: Box<dyn RatingSystem> = Box::new(SanityRatingSystem { inner: rating });
    let outcome = LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &serde_yaml::from_str("beta: 400.0\nnoise: 0.0").unwrap(),
    )
    .expect("logistic loads");
    let matchmaker = LuaMatchmaker::load(script, &serde_yaml::from_str(params).unwrap())
        .expect("matchmaker loads");
    let config = LoopConfig {
        teams,
        batch_interval_ticks: 10,
        rejoin_delay: SimTime::from_secs(30.0),
        max_matches,
        skill_update_interval: None,
        stream_seeds: StreamSeeds::from_seed(seed),
        record_history: false,
    };
    MatchLoop::new(
        population,
        rating,
        Box::new(outcome),
        Box::new(matchmaker),
        metrics,
        config,
    )
}
fn queue_time_lua() -> Box<dyn MetricCollector> {
    Box::new(
        LuaMetricCollector::load(
            &format!("plugins/metrics/{QUEUE_TIME_METRIC}.lua"),
            &serde_yaml::Value::Null,
        )
        .expect("queue_time metric loads"),
    )
}
fn rating_snapshot(loop_: &MatchLoop) -> Vec<(u64, f64, f64, u64)> {
    let mut out: Vec<(u64, f64, f64, u64)> = loop_
        .world
        .observations
        .iter()
        .map(|(pid, o)| (pid.0, o.rating, o.rating_deviation, o.games_played))
        .collect();
    out.sort_by_key(|x| x.0);
    out
}
/// Register every invariant guard plus the real queue-time collector.
fn guard_engine(shape: &Shape) -> MetricsEngine {
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(TeamSizeGuard::new(
        shape.teams.team_size_a,
        shape.teams.team_size_b,
    )));
    metrics.register(Box::new(RoleConstraintGuard::new(
        shape.teams.role_a.clone(),
        shape.teams.role_b.clone(),
    )));
    metrics.register(Box::new(WinRateGuard::default()));
    metrics.register(Box::new(QueueTimeGuard::default()));
    metrics.register(Box::new(SingleMatchGuard::default()));
    metrics.register(queue_time_lua());
    metrics
}
/// Drive one config through the loop twice (same seed) and return the checked
/// outcome of the first run.
fn run_paired(shape: &Shape, size: u64, seed: u64) -> (MatchLoop, HashMap<String, MetricResult>) {
    let population = (shape.population)(size, seed);
    let mut a = build_loop_with(
        population,
        shape.teams.clone(),
        MAX_MATCHES,
        seed,
        guard_engine(shape),
        shape.script,
        shape.params,
    );
    let population = (shape.population)(size, seed);
    let mut b = build_loop_with(
        population,
        shape.teams.clone(),
        MAX_MATCHES,
        seed,
        guard_engine(shape),
        shape.script,
        shape.params,
    );
    a.run_until(HORIZON);
    b.run_until(HORIZON);
    let a_metrics = a.finalize_metrics();
    let b_metrics = b.finalize_metrics();
    assert_eq!(
        a.state.lock().unwrap().matches_completed,
        b.state.lock().unwrap().matches_completed,
        "{} seed {seed}: matches_completed diverges across same-seed runs",
        shape.name
    );
    assert_eq!(
        a_metrics, b_metrics,
        "{} seed {seed}: metrics diverge",
        shape.name
    );
    assert_eq!(
        rating_snapshot(&a),
        rating_snapshot(&b),
        "{} seed {seed}: ratings diverge",
        shape.name
    );
    (a, a_metrics)
}
fn check_run(
    shape: &Shape,
    size: u64,
    seed: u64,
    a: &MatchLoop,
    metrics: &HashMap<String, MetricResult>,
) {
    check_metrics_finite(metrics);
    let queue_summary = match metrics.get(QUEUE_TIME_METRIC) {
        Some(MetricResult::Summary { mean, stddev, .. }) => (*mean, *stddev),
        other => panic!(
            "{}: queue_time must be a Summary, got {other:?}",
            shape.name
        ),
    };
    assert!(
        queue_summary.0 >= 0.0 && queue_summary.1 >= 0.0,
        "{}: queue_time must not be negative (mean={} stddev={})",
        shape.name,
        queue_summary.0,
        queue_summary.1
    );
    let guards = [
        ("invariants_team_size", "team sizes"),
        ("invariants_role_constraint", "role constraints"),
        ("invariants_win_rate", "win rates"),
        ("invariants_queue_time", "queue times"),
        ("invariants_single_match", "single-match membership"),
    ];
    for (metric, what) in guards {
        assert_eq!(
            violations_of(metrics, metric),
            0,
            "{} seed {seed}: {what} invariant violated",
            shape.name
        );
    }
    let state = a.state.lock().unwrap();
    assert!(
        state.matches_completed > 0,
        "{} seed {seed}: no matches completed — sweep not exercised",
        shape.name
    );
    assert!(
        state.active_matches.is_empty(),
        "{} seed {seed}: run finished with in-flight matches",
        shape.name
    );
    assert_eq!(
        state.population.len() as u64,
        size,
        "{} seed {seed}: population not conserved",
        shape.name
    );
    assert_eq!(
        a.world.players.len() as u64,
        size,
        "{} seed {seed}: world population not conserved",
        shape.name
    );
    for (pid, o) in &a.world.observations {
        assert_finite(
            o.rating,
            &format!("{}: player {} rating", shape.name, pid.0),
        );
        assert!(
            o.win_rate.is_finite() && (0.0..=1.0).contains(&o.win_rate),
            "{}: player {} win_rate {} outside [0,1]",
            shape.name,
            pid.0,
            o.win_rate
        );
    }
}
#[test]
fn invariants_hold_across_config_seed_grid() {
    let mut pick = SimRng::from_seed(SEED_BASE);
    let shapes = shapes();
    let per_shape_seeds: Vec<u64> = (0..3).map(|_| pick.gen_u64()).collect();
    let per_shape_sizes: Vec<u64> = (0..3)
        .map(|_| 180 + 2 * (pick.gen_range(0.0, 30.0) as u64))
        .collect();
    let mut checked = 0;
    for shape in &shapes {
        for (&size, &seed) in per_shape_sizes.iter().zip(per_shape_seeds.iter()) {
            let (a, metrics) = run_paired(shape, size, seed);
            check_run(shape, size, seed, &a, &metrics);
            let rating = registry::from_name(
                "elo",
                &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n")
                    .unwrap(),
            )
            .expect("elo loads");
            let players: Vec<PlayerObservation> =
                a.world.observations.values().take(64).cloned().collect();
            for window in players.chunks(2) {
                let p = rating.predict(
                    std::slice::from_ref(&window[0]),
                    std::slice::from_ref(&window[1]),
                );
                assert!(
                    p.is_finite() && (0.0..=1.0).contains(&p),
                    "{}: elo predict {} for gap {} outside [0,1]",
                    shape.name,
                    p,
                    window[0].rating - window[1].rating
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, shapes.len() * per_shape_seeds.len());
}
fn make_result(team_a: &[u64], team_b: &[u64], duration_secs: f64) -> MatchResult {
    MatchResult {
        match_id: MatchId(1),
        winner: Team::A,
        team_a: team_a.iter().map(|id| PlayerId(*id)).collect(),
        team_b: team_b.iter().map(|id| PlayerId(*id)).collect(),
        team_a_score: 1.0,
        team_b_score: 0.0,
        player_performances: Vec::new(),
        duration: SimTime::from_secs(duration_secs),
        disconnected: false,
        forfeited: false,
        variance: 0.1,
        unexpected_events: Vec::new(),
    }
}
fn world_with(
    ratings: &[(u64, f64)],
    win_rate_override: Option<(u64, f64)>,
    queue_joined_at_override: Option<(u64, f64)>,
) -> World {
    let mut world = World::new(SimRng::from_seed(1));
    world.time = SimTime::from_secs(60.0);
    for &(id, rating) in ratings {
        let mut o = observation(id, rating);
        if let Some((target, wr)) = win_rate_override {
            if target == id {
                o.win_rate = wr;
            }
        }
        if let Some((target, jt)) = queue_joined_at_override {
            if target == id {
                o.queue_joined_at = Some(SimTime::from_secs(jt));
            }
        }
        world.observations.insert(PlayerId(id), o);
    }
    world
}
#[test]
fn guards_fire_on_deliberately_violated_input() {
    let rejected = catch_unwind(AssertUnwindSafe(|| {
        let _ = TeamSizeGuard::new(0, 2);
    }));
    assert!(rejected.is_err(), "zero-size composition must be refused");
    let mut tsg = TeamSizeGuard::new(2, 2);
    let world = world_with(&[], None, None);
    tsg.record_match(&make_result(&[1], &[2, 3], 30.0), &world);
    assert_eq!(tsg.violations, 1, "1v2 against 2v2 composition must fire");
    let mut rcg =
        RoleConstraintGuard::new(Some("killer".to_string()), Some("survivor".to_string()));
    let mut world = world_with(&[(1, 1000.0), (2, 1000.0)], None, None);
    world.observations.get_mut(&PlayerId(1)).unwrap().role = Some("survivor".to_string());
    world.observations.get_mut(&PlayerId(2)).unwrap().role = Some("survivor".to_string());
    rcg.record_match(&make_result(&[1], &[2], 30.0), &world);
    assert_eq!(rcg.violations, 1, "role leak must fire");
    let mut wrg = WinRateGuard::default();
    let world = world_with(&[(1, 1000.0)], Some((1, 2.0)), None);
    wrg.record_match(&make_result(&[1], &[2], 30.0), &world);
    assert_eq!(wrg.violations, 1, "win_rate 2.0 must fire");
    let mut qtg = QueueTimeGuard::default();
    let world = world_with(&[(1, 1000.0)], None, Some((1, 1000.0)));
    qtg.record_match(&make_result(&[1], &[2], 30.0), &world);
    assert_eq!(qtg.violations, 1, "join after formation must fire");
    let mut smg = SingleMatchGuard::default();
    let a = world_with(&[], None, None);
    smg.record_match(&make_result(&[1, 2], &[3, 4], 60.0), &a);
    let mut b = world_with(&[], None, None);
    b.observations.insert(PlayerId(1), observation(1, 1000.0));
    smg.record_match(&make_result(&[1, 5], &[6, 7], 60.0), &b);
    assert_eq!(smg.violations, 1, "overlapping intervals must fire");
    let nan_metric = MetricResult::Distribution(vec![f64::NAN]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| check_metrics_finite(&HashMap::from([
            ("bad".to_string(), nan_metric,)
        ]))))
        .is_err(),
        "NaN metric must be rejected"
    );
    let nan_state = HashMap::from([(
        PlayerId(1),
        RatingState {
            rating: f64::NAN,
            rating_deviation: 100.0,
            volatility: 0.06,
            games_played: 1,
        },
    )]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| check_finite_rating_states(&nan_state))).is_err(),
        "NaN RatingState must be rejected"
    );
}
#[test]
fn build_loop_drives_the_same_stack() {
    let population = single_class_pop(20, 7);
    let teams = equal_teams(5);
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(TeamSizeGuard::new(5, 5)));
    let mut via_build_loop = build_loop(population, teams.clone(), 4, 42, metrics);
    let population = single_class_pop(20, 7);
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(TeamSizeGuard::new(5, 5)));
    let mut via_build_loop_with = build_loop_with(
        population,
        teams.clone(),
        4,
        42,
        metrics,
        "plugins/matchmaking/batch.lua",
        "",
    );
    via_build_loop.run_until(HORIZON);
    via_build_loop_with.run_until(HORIZON);
    let a = via_build_loop.finalize_metrics();
    let b = via_build_loop_with.finalize_metrics();
    assert_eq!(
        via_build_loop.state.lock().unwrap().matches_completed,
        via_build_loop_with.state.lock().unwrap().matches_completed
    );
    assert_eq!(a, b, "build_loop_with must reproduce build_loop");
    assert_eq!(
        violations_of(&a, "invariants_team_size"),
        0,
        "helper-driven loop must still satisfy team sizes"
    );
}
#[test]
fn mid_run_active_matches_never_overlap() {
    let population = single_class_pop(200, 11);
    let metrics = MetricsEngine::new();
    let mut loop_ = build_loop(population, equal_teams(5), 200, 9, metrics);
    loop_.run_until(SimTime(40_000_000_000));
    let state = loop_.state.lock().unwrap();
    let mut seen: HashMap<PlayerId, MatchId> = HashMap::new();
    for (match_id, result) in &state.active_matches {
        for pid in result.team_a.iter().chain(result.team_b.iter()) {
            assert!(
                seen.insert(*pid, *match_id).is_none(),
                "player {} in two simultaneous matches",
                pid.0
            );
        }
    }
}
