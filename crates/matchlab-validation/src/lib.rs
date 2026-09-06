//! matchlab-validation: analytical-baseline regression tests.
//!
//! The purpose of this crate is to **prove the simulator behaves correctly in
//! situations where the theoretical answer is known** (feedback.md's primary
//! ask). Reference math and test-only collectors live here so the Lua systems
//! can be checked against independently derived values.
//!
//! Nothing in this crate is wired into the simulation loop. The reference
//! implementations exist solely to catch drift in the Lua scripts; when they
//! disagree with a script, that is a bug report, never a reason to patch the
//! reference.

pub mod reference;

use matchlab_core::player::{
    PlayerId, PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_experiments::config::ExperimentConfig;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_matchmaking::queue::QueueEntry;
use matchlab_metrics::MetricResult;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;
use std::collections::{HashMap, VecDeque};

/// Build a full `PlayerObservation` with a given visible rating (hidden_mmr and
/// skill_vector aligned with rating, so callers that need a rating≠skill
/// mismatch mutate `skill_vector` afterwards).
pub fn observation(id: u64, rating: f64) -> PlayerObservation {
    PlayerObservation {
        id: PlayerId(id),
        rating,
        hidden_mmr: rating,
        visible_rank: VisibleRank {
            tier: "unranked".into(),
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
        session_history: VecDeque::new(),
        quit_history: VecDeque::new(),
        tilt_level: 0.0,
        game_mode: "ranked".into(),
        skill_vector: SkillVector::one_dimensional(rating),
        detection_flags: Vec::new(),
    }
}

/// Build a `QueueEntry` with the given join timestamp and visible rating.
pub fn queue_entry(id: u64, joined_at: SimTime, rating: f64) -> QueueEntry {
    QueueEntry {
        player_id: PlayerId(id),
        joined_at,
        observation: observation(id, rating),
        region: Region::NA,
        party_id: None,
        game_mode: "ranked".to_string(),
        role: None,
        latency_ms: 30.0,
    }
}

/// Logistic win probability for a skill difference (the outcome model's
/// natural scale, `exp`, not the log10 Elo scale).
pub fn logistic_win_probability(diff: f64, beta: f64) -> f64 {
    1.0 / (1.0 + (-diff / beta).exp())
}

fn archetype(name: &str, skill: f64, initial_rating: f64) -> ArchetypeConfig {
    ArchetypeConfig {
        name: name.to_string(),
        proportion: 0.5,
        skill_distribution: DistributionConfig::Normal {
            mean: skill,
            stddev: 0.0,
        },
        skill_volatility: 0.0,
        improvement_rate: 0.0,
        play_frequency: 0.8,
        session_length: 1800.0,
        quit_probability: 0.0,
        initial_rating: Some(initial_rating),
    }
}

fn rebuilt(
    id: PlayerId,
    mut r: PlayerReality,
    mut o: PlayerObservation,
) -> (PlayerReality, PlayerObservation) {
    r.id = id;
    o.id = id;
    (r, o)
}

/// A two class population (high/low skills) with player ids interleaved so
/// that adjacent ids alternate classes — the batch matchmaker pairs adjacent
/// ids, and interleaving guarantees ~every formed team is class-mixed.
pub fn interleaved_two_class_population(
    total: u64,
    skill_high: f64,
    skill_low: f64,
    initial_rating: f64,
    seed: u64,
) -> Vec<(PlayerReality, PlayerObservation)> {
    let config = PopulationConfig {
        size: total,
        archetypes: vec![
            archetype("high", skill_high, initial_rating),
            archetype("low", skill_low, initial_rating),
        ],
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    let n = realities.len();
    let half = n / 2;
    let mut out: Vec<(PlayerReality, PlayerObservation)> = Vec::with_capacity(n);
    let mut hi = 0usize;
    let mut lo = half;
    for i in 0..n {
        let (r, o) = if i % 2 == 0 {
            let (r, o) = (realities[hi].clone(), observations[hi].clone());
            hi += 1;
            (r, o)
        } else {
            let (r, o) = (realities[lo].clone(), observations[lo].clone());
            lo += 1;
            (r, o)
        };
        out.push(rebuilt(PlayerId(i as u64), r, o));
    }
    out
}

/// A full `ExperimentConfig` manifest for a homogeneous (single-class where
/// visible rating starts below true-skill spread, so Elo has something to
/// learn) experiment.
pub fn single_class_config(
    name: &str,
    seed: u64,
    size: u64,
    team_size: usize,
    matches: u64,
    max_time: f64,
    metrics: &[&str],
) -> ExperimentConfig {
    let yaml = format!(
        r#"
experiment:
  name: {name}
  seed: {seed}
  population:
    size: {size}
    seed: {seed}
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: {{ type: normal, mean: 1000, stddev: 250 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    team_size: {team_size}
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.05
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics: [{metrics_list}]
  cohorts: []
  duration:
    matches: {matches}
    max_time: {max_time}
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        name = name,
        seed = seed,
        size = size,
        team_size = team_size,
        matches = matches,
        max_time = max_time,
        metrics_list = metrics.join(", "),
    );
    serde_yaml::from_str(&yaml).expect("valid validation manifest")
}

/// A full `ExperimentConfig` manifest for a two class experiment.
pub fn two_class_config(
    name: &str,
    seed: u64,
    size: u64,
    team_size: usize,
    matches: u64,
    max_time: f64,
    metrics: &[&str],
) -> ExperimentConfig {
    let yaml = format!(
        r#"
experiment:
  name: {name}
  seed: {seed}
  population:
    size: {size}
    seed: {seed}
    archetypes:
      - name: high
        proportion: 0.5
        skill_distribution: {{ type: normal, mean: 1500, stddev: 0 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
      - name: low
        proportion: 0.5
        skill_distribution: {{ type: normal, mean: 1000, stddev: 0 }}
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    team_size: {team_size}
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics: [{metrics_list}]
  cohorts: []
  duration:
    matches: {matches}
    max_time: {max_time}
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#,
        name = name,
        seed = seed,
        size = size,
        team_size = team_size,
        matches = matches,
        max_time = max_time,
        metrics_list = metrics.join(", "),
    );
    serde_yaml::from_str(&yaml).expect("valid validation manifest")
}

/// Drive a `MatchLoop` directly with elo + logistic(noise 0) + batch and
/// return the finalized metrics plus completion stats. `metrics` may contain
/// test-only collectors.
pub struct LoopOutcome {
    pub metrics: HashMap<String, MetricResult>,
    pub matches_completed: u64,
    pub simulated_time_secs: f64,
}

pub fn run_loop(
    population: Vec<(PlayerReality, PlayerObservation)>,
    team_size: usize,
    max_matches: u64,
    max_time_secs: f64,
    seed: u64,
    metrics: MetricsEngine,
) -> LoopOutcome {
    let rating = registry::from_name(
        "elo",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .expect("elo loads");
    let outcome = LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &serde_yaml::from_str("beta: 400.0\nnoise: 0.0").unwrap(),
    )
    .expect("logistic loads");
    let matchmaker = LuaMatchmaker::load(
        "plugins/matchmaking/batch.lua",
        &serde_yaml::from_str("").unwrap(),
    )
    .expect("batch loads");

    let config = LoopConfig {
        team_size,
        batch_interval_ticks: 10,
        rejoin_delay: SimTime::from_secs(30.0),
        max_matches,
        skill_update_interval: None,
    };
    let mut loop_ = MatchLoop::new(
        population,
        rating,
        Box::new(outcome),
        Box::new(matchmaker),
        metrics,
        config,
        seed,
    );
    let until = SimTime::from_secs(max_time_secs);
    loop_.run_until(until);
    let (matches_completed, simulated_time_secs) = {
        let state = loop_.state.lock().unwrap();
        (state.matches_completed, loop_.world.time.as_secs_f64())
    };
    let metrics = loop_.finalize_metrics();
    LoopOutcome {
        metrics,
        matches_completed,
        simulated_time_secs,
    }
}

/// Read a `Summary` metric's mean (e.g. `rating_accuracy` MAE).
pub fn summary_mean(result: &MetricResult) -> f64 {
    match result {
        MetricResult::Summary { mean, .. } => *mean,
        other => panic!("expected summary, got {other:?}"),
    }
}
