//! Experiment runner (spec §13.4).
//!
//! Builds a full `MatchLoop` from an `ExperimentConfig`: generate the
//! population, construct the configured rating system / outcome model /
//! matchmaker, run the discrete-event simulation, and fold the registered
//! metric collectors into an `ExperimentResult`.

use std::collections::BTreeMap;

use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_game::logistic::LogisticOutcomeModel;
use matchlab_game::outcome::OutcomeModel;
use matchlab_loop::{LoopConfig, MatchLoop};
use matchlab_matchmaking::batch::BatchMatchmaker;
use matchlab_matchmaking::expanding::ExpandingWindowMatchmaker;
use matchlab_matchmaking::hub_spoke::HubSpokeMatchmaker;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_matchmaking::strict::StrictMatchmaker;
use matchlab_metrics::{
    ConvergenceCollector, DimensionalityFidelityCollector, MatchInequalityCollector,
    MatchQualityCollector, MetricResult, MetricsEngine, NDCGCollector, PopulationHealthCollector,
    QueueTimeCollector, RatingAccuracyCollector, ResponsivenessCollector, SmurfMetricsCollector,
    StabilityCollector, StreakCollector,
};
use matchlab_objective::utility::{ObjectiveFunction, ObjectiveWeights};
use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
use matchlab_players::population::{PopulationConfig, PopulationGenerator};
use matchlab_rating::registry;
use matchlab_rating::system::RatingSystem;

use crate::config::{
    ArchetypeSpec, DistributionSpec, ExperimentConfig, GameSpec, MatchmakingSpec, RankingSpec,
    RatingSystemSpec,
};
use crate::seed::{SeedManager, git_commit_hash, hash_config};

pub struct ExperimentRunner;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExperimentResult {
    pub experiment_id: String,
    pub name: String,
    pub config_hash: String,
    pub git_commit: String,
    pub timestamp: String,
    pub matches_completed: u64,
    pub matches_formed: u64,
    pub simulated_time_secs: f64,
    pub metrics: BTreeMap<String, MetricResult>,
    pub utility_score: Option<f64>,
}

impl ExperimentRunner {
    pub fn run(config: &ExperimentConfig) -> Result<ExperimentResult, String> {
        let seeds = SeedManager::from_experiment_seed(config.experiment.seed);

        let population = generate_population(&config.experiment.population, seeds.population_seed);
        let rating_system = build_rating_system(&config.experiment.rating.systems)?;
        let outcome_model = build_outcome_model(&config.experiment.game)?;
        let matchmaker = build_matchmaker(&config.experiment.matchmaking)?;
        let mut metrics = MetricsEngine::new();
        register_metrics(&mut metrics, &config.experiment.metrics)?;

        let config_hash = hash_config(config);
        let loop_config = LoopConfig {
            team_size: config.experiment.game.team_size,
            batch_interval_ticks: batch_interval_secs(&config.experiment.matchmaking),
            rejoin_delay: SimTime::from_secs(30.0),
            max_matches: config.experiment.duration.matches,
        };

        let seed = seeds.population_seed;
        let detection_system = build_detection_system(config.experiment.detection.as_ref())?;
        let ranker = build_ranker(config.experiment.ranking.as_ref());
        let adversarial_agents = build_adversarial_agents(config.experiment.adversarial.as_ref());
        let satisfaction_model = build_satisfaction_model(config.experiment.satisfaction.as_ref());

        let mut loop_ = MatchLoop::with_extras(
            population,
            rating_system,
            outcome_model,
            matchmaker,
            metrics,
            loop_config,
            seed,
            detection_system,
            ranker,
            adversarial_agents,
            satisfaction_model,
        );
        let until = SimTime::from_secs(config.experiment.duration.max_time);
        loop_.run_until(until);

        let (matches_completed, matches_formed, simulated_time_secs) = {
            let state = loop_.state.lock().unwrap();
            (
                state.matches_completed,
                state.matches_formed(),
                loop_.world.time.as_secs_f64(),
            )
        };
        let metrics = loop_.finalize_metrics();
        let metrics: BTreeMap<String, MetricResult> = metrics.into_iter().collect();

        let utility_score = config.experiment.objectives.as_ref().map(|obj| {
            let weights = ObjectiveWeights {
                match_quality: obj.match_quality.unwrap_or(1.0),
                queue_time: obj.queue_time.unwrap_or(0.5),
                rating_accuracy: obj.rating_accuracy.unwrap_or(1.0),
                convergence_speed: obj.convergence_speed.unwrap_or(0.8),
                smurf_damage: obj.smurf_damage.unwrap_or(2.0),
                false_positive_rate: obj.false_positive_rate.unwrap_or(1.5),
                streak_frustration: obj.streak_frustration.unwrap_or(0.3),
            };
            let func = ObjectiveFunction::new(weights);
            let metrics_map: std::collections::HashMap<String, MetricResult> =
                metrics.clone().into_iter().collect();
            let (score, _) = func.evaluate(&metrics_map);
            score
        });

        let result = ExperimentResult {
            experiment_id: format!("{}-{}", config.experiment.name, config_hash),
            name: config.experiment.name.clone(),
            config_hash,
            git_commit: git_commit_hash(),
            timestamp: iso8601_utc(),
            matches_completed,
            matches_formed,
            simulated_time_secs,
            metrics,
            utility_score,
        };
        Ok(result)
    }
}

fn generate_population(
    spec: &crate::config::PopulationSpec,
    seed: u64,
) -> Vec<(PlayerReality, PlayerObservation)> {
    let archetypes: Vec<ArchetypeConfig> =
        spec.archetypes.iter().map(to_players_archetype).collect();
    let config = PopulationConfig {
        size: spec.size,
        archetypes,
    };
    let mut rng = SimRng::from_seed(seed);
    let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
    realities.into_iter().zip(observations).collect()
}

fn to_players_archetype(spec: &ArchetypeSpec) -> ArchetypeConfig {
    ArchetypeConfig {
        name: spec.name.clone(),
        proportion: spec.proportion,
        skill_distribution: match &spec.skill_distribution {
            DistributionSpec::Normal { mean, stddev } => DistributionConfig::Normal {
                mean: *mean,
                stddev: *stddev,
            },
            DistributionSpec::Uniform { low, high } => DistributionConfig::Uniform {
                low: *low,
                high: *high,
            },
            DistributionSpec::LogNormal { mean, stddev } => DistributionConfig::LogNormal {
                mean: *mean,
                stddev: *stddev,
            },
        },
        skill_volatility: spec.skill_volatility,
        improvement_rate: spec.improvement_rate,
        play_frequency: spec.play_frequency,
        session_length: spec.session_length,
        quit_probability: spec.quit_probability,
        initial_rating: spec.initial_rating,
    }
}

fn build_rating_system(systems: &[RatingSystemSpec]) -> Result<Box<dyn RatingSystem>, String> {
    let spec = match systems.first() {
        Some(s) => s,
        None => return Err("rating.systems must declare at least one system".to_string()),
    };
    let params = flatten_params(&spec.params);
    if let Some(name) = &spec.name {
        registry::from_name(name, &params)
    } else if let Some(script) = &spec.script {
        registry::from_script(script, &params)
    } else {
        Err("rating system must declare a `name` or `script`".to_string())
    }
}

fn flatten_params(
    params: &std::collections::BTreeMap<String, serde_yaml::Value>,
) -> serde_yaml::Value {
    let mut mapping = serde_yaml::Mapping::new();
    for (key, value) in params {
        mapping.insert(serde_yaml::Value::String(key.clone()), value.clone());
    }
    serde_yaml::Value::Mapping(mapping)
}

fn build_outcome_model(spec: &GameSpec) -> Result<Box<dyn OutcomeModel>, String> {
    let base = match spec.outcome_model.as_str() {
        "logistic" => {
            Box::new(LogisticOutcomeModel::new(spec.beta, spec.noise)) as Box<dyn OutcomeModel>
        }
        other => return Err(format!("unknown outcome model: {other}")),
    };
    let variant = spec.variant.as_deref().unwrap_or("");
    match variant {
        "" => Ok(base),
        "variance" => {
            let multiplier = spec
                .params
                .get("variance_multiplier")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            Ok(Box::new(
                matchlab_game::variance::VarianceOutcomeModel::new(
                    spec.beta, spec.noise, multiplier,
                ),
            ))
        }
        "composition" => {
            let weights = spec
                .params
                .get("dimension_weights")
                .and_then(|v| v.as_mapping())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| {
                            (
                                k.as_str().unwrap_or("").to_string(),
                                v.as_f64().unwrap_or(1.0),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let synergy = spec
                .params
                .get("synergy_bonus")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            Ok(Box::new(
                matchlab_game::composition::CompositionOutcomeModel::new(
                    weights, synergy, spec.beta,
                ),
            ))
        }
        "performance" => {
            let weight = spec
                .params
                .get("performance_weight")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            Ok(Box::new(
                matchlab_game::performance::PerformanceOutcomeModel::new(spec.beta, weight),
            ))
        }
        "fatigue" => {
            let decay = spec
                .params
                .get("fatigue_decay_rate")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.001);
            Ok(Box::new(matchlab_game::fatigue::FatigueOutcomeModel::new(
                base, decay,
            )))
        }
        "momentum" => {
            let factor = spec
                .params
                .get("momentum_factor")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.1);
            Ok(Box::new(
                matchlab_game::momentum::MomentumOutcomeModel::new(base, factor),
            ))
        }
        other => Err(format!("unknown outcome model variant: {other}")),
    }
}

fn build_matchmaker(spec: &MatchmakingSpec) -> Result<Box<dyn Matchmaker>, String> {
    match spec.algorithm.as_str() {
        "batch" => Ok(Box::new(BatchMatchmaker::new(batch_interval_secs(spec)))),
        "expanding_window" => {
            let tiers: Vec<(f64, f64)> = spec
                .params
                .get("tiers")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|t| {
                            let arr = t.as_sequence()?;
                            let a = arr.first()?.as_f64()?;
                            let b = arr.get(1)?.as_f64()?;
                            Some((a, b))
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![(5.0, 25.0), (10.0, 50.0), (20.0, 100.0), (30.0, 200.0)]);
            let max_window = spec
                .params
                .get("max_window")
                .and_then(|v| v.as_f64())
                .unwrap_or(400.0);
            Ok(Box::new(ExpandingWindowMatchmaker::with_tiers(
                tiers, max_window,
            )))
        }
        "strict" => {
            let max_diff = spec
                .params
                .get("max_skill_diff")
                .and_then(|v| v.as_f64())
                .unwrap_or(50.0);
            Ok(Box::new(StrictMatchmaker::new(max_diff)))
        }
        "hub_spoke" => {
            let capacity = spec
                .params
                .get("spoke_capacity")
                .and_then(|v| v.as_u64())
                .unwrap_or(100) as usize;
            let spokes = std::collections::HashMap::new();
            Ok(Box::new(HubSpokeMatchmaker::new(spokes, capacity)))
        }
        other => Err(format!("unknown matchmaker algorithm: {other}")),
    }
}

fn batch_interval_secs(spec: &MatchmakingSpec) -> u64 {
    spec.params
        .get("batch_interval")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            spec.params
                .get("batch_interval")
                .and_then(|v| v.as_f64())
                .map(|f| f as u64)
        })
        .unwrap_or(60)
}

fn register_metrics(engine: &mut MetricsEngine, names: &[String]) -> Result<(), String> {
    for name in names {
        match name.as_str() {
            "rating_accuracy" => engine.register(Box::new(RatingAccuracyCollector::new())),
            "match_quality" => engine.register(Box::new(MatchQualityCollector::new())),
            "queue_time" => engine.register(Box::new(QueueTimeCollector::new())),
            "match_inequality" => engine.register(Box::new(MatchInequalityCollector::new())),
            "ndcg" => engine.register(Box::new(NDCGCollector::new())),
            "dimensionality_fidelity" => {
                engine.register(Box::new(DimensionalityFidelityCollector::new()))
            }
            "convergence" => engine.register(Box::new(ConvergenceCollector::default())),
            "responsiveness" => engine.register(Box::new(ResponsivenessCollector::new())),
            "stability" => engine.register(Box::new(StabilityCollector::new())),
            "streaks" => engine.register(Box::new(StreakCollector::new())),
            "population_health" => engine.register(Box::new(PopulationHealthCollector::new())),
            "smurf" => engine.register(Box::new(SmurfMetricsCollector::new())),
            other => return Err(format!("unknown metric collector: {other}")),
        }
    }
    Ok(())
}

fn build_detection_system(
    spec: Option<&crate::config::DetectionSpec>,
) -> Result<Option<Box<dyn matchlab_detection::detector::DetectionSystem>>, String> {
    let Some(spec) = spec else {
        return Ok(None);
    };
    if !spec.enabled {
        return Ok(None);
    }
    let Some(smurf) = spec.smurf.as_ref() else {
        return Ok(None);
    };
    let mut policy = matchlab_detection::intervention::InterventionPolicy::default_ladder();
    policy.min_games_before_action = smurf.min_games_before_action;
    let mut detector = matchlab_detection::smurf::SmurfDetector::new(policy);
    detector.sigma_threshold = 3.0;
    detector.min_anomalous_games = smurf.min_games_before_action.max(1);
    Ok(Some(Box::new(detector)))
}

fn build_ranker(
    spec: Option<&RankingSpec>,
) -> Option<Box<dyn matchlab_ranking::ranker::RankMapper>> {
    let spec = spec?;
    let brackets = spec
        .brackets
        .iter()
        .map(|b| matchlab_ranking::ranker::RankBracket {
            rank: matchlab_ranking::ranker::Rank {
                tier: b.rank.tier.clone(),
                division: b.rank.division,
            },
            min: b.min,
            max: b.max,
        })
        .collect();
    Some(Box::new(matchlab_ranking::ranker::BracketRankMapper::new(
        brackets,
    )))
}

fn build_adversarial_agents(
    spec: Option<&crate::config::AdversarialSpec>,
) -> std::collections::HashMap<PlayerId, Box<dyn matchlab_adversarial::agent::AdversarialAgent>> {
    use matchlab_adversarial::agent::AdversarialAgent;
    let mut agents: std::collections::HashMap<PlayerId, Box<dyn AdversarialAgent>> =
        std::collections::HashMap::new();
    let Some(spec) = spec else {
        return agents;
    };
    for agent_spec in &spec.agents {
        let agent: Option<Box<dyn AdversarialAgent>> = match agent_spec.agent_type.as_str() {
            "afk" => {
                let p = agent_spec
                    .params
                    .get("go_afk_probability")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.1);
                Some(Box::new(matchlab_adversarial::afk::AfkAgent::new(p)))
            }
            "deranker" => {
                let target = agent_spec
                    .params
                    .get("target_rating")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(500.0);
                Some(Box::new(
                    matchlab_adversarial::deranker::DerankerAgent::new(target),
                ))
            }
            "rating_farmer" => {
                let qp = agent_spec
                    .params
                    .get("quit_probability")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5);
                let minutes = agent_spec
                    .params
                    .get("quit_after_minutes")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(5.0);
                Some(Box::new(
                    matchlab_adversarial::rating_farmer::RatingFarmerAgent::new(qp, minutes),
                ))
            }
            "booster" => {
                let boost_target = agent_spec
                    .params
                    .get("boost_target")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(agent_spec.player.unwrap_or(0));
                let boostee = agent_spec
                    .params
                    .get("boostee")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(boost_target + 1);
                Some(Box::new(matchlab_adversarial::booster::BoosterAgent::new(
                    PlayerId(boost_target),
                    PlayerId(boostee),
                )))
            }
            "win_trader" => {
                let partner = agent_spec
                    .params
                    .get("partner")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(agent_spec.player.unwrap_or(0) + 1);
                let alternating = agent_spec
                    .params
                    .get("alternating")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                Some(Box::new(
                    matchlab_adversarial::win_trader::WinTraderAgent::new(
                        PlayerId(partner),
                        alternating,
                    ),
                ))
            }
            _ => None,
        };
        if let Some(agent) = agent {
            let pid = agent_spec.player.unwrap_or(0);
            agents.insert(PlayerId(pid), agent);
        }
    }
    agents
}

fn build_satisfaction_model(
    spec: Option<&crate::config::SatisfactionSpec>,
) -> Option<matchlab_utility::satisfaction::SatisfactionModel> {
    let spec = spec?;
    if !spec.enabled {
        return None;
    }
    let w = spec.weights.as_ref();
    let weights = matchlab_utility::satisfaction::SatisfactionWeights {
        match_quality: w.and_then(|w| w.match_quality).unwrap_or(1.0),
        queue_time_penalty: w.and_then(|w| w.queue_time_penalty).unwrap_or(-0.01),
        win_bonus: w.and_then(|w| w.win_bonus).unwrap_or(0.5),
        loss_streak_penalty: w.and_then(|w| w.loss_streak_penalty).unwrap_or(-0.3),
        rank_progression_bonus: w.and_then(|w| w.rank_progression_bonus).unwrap_or(0.2),
        fairness_sensitivity: w.and_then(|w| w.fairness_sensitivity).unwrap_or(-0.8),
        rematch_bonus: w.and_then(|w| w.rematch_bonus).unwrap_or(0.1),
    };
    Some(matchlab_utility::satisfaction::SatisfactionModel::new(
        weights,
    ))
}

/// ISO-8601 UTC timestamp without external dependencies.
fn iso8601_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Convert days since 1970-01-01 to a (year, month, day) civil date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = r#"
experiment:
  name: mini
  seed: 7
  population:
    size: 100
    seed: 7
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 150 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    team_size: 1
    outcome_model: logistic
    beta: 400.0
    noise: 0.05
  matchmaking:
    algorithm: batch
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - script: plugins/rating/elo.lua
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics:
    - match_quality
    - queue_time
    - rating_accuracy
  cohorts: []
  duration:
    matches: 40
    max_time: 200000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;

    fn mini_config() -> ExperimentConfig {
        serde_yaml::from_str(MINI).unwrap()
    }

    #[test]
    fn runner_completes_and_reports_metrics() {
        let config = mini_config();
        let result = ExperimentRunner::run(&config).expect("run completes");
        assert_eq!(result.matches_completed, 40);
        assert_eq!(result.matches_formed, 40);
        assert!(result.config_hash.len() == 16);
        assert!(result.git_commit == "unknown" || result.git_commit.len() >= 7);
        for metric in ["match_quality", "queue_time", "rating_accuracy"] {
            assert!(
                result.metrics.contains_key(metric),
                "missing metric result: {metric}"
            );
        }
    }

    #[test]
    fn same_config_same_seed_gives_identical_results() {
        let config = mini_config();
        let a = ExperimentRunner::run(&config).unwrap();
        let b = ExperimentRunner::run(&config).unwrap();
        assert_eq!(a.config_hash, b.config_hash);
        assert_eq!(a.matches_completed, b.matches_completed);
        assert_eq!(a.metrics, b.metrics);
        assert_eq!(a.experiment_id, b.experiment_id);
    }

    #[test]
    fn unknown_rating_system_is_rejected() {
        let mut config = mini_config();
        config.experiment.rating.systems[0].name = Some("bogus".to_string());
        config.experiment.rating.systems[0].script = None;
        assert!(ExperimentRunner::run(&config).is_err());
    }

    #[test]
    fn unknown_metric_is_rejected() {
        let mut config = mini_config();
        config.experiment.metrics.push("bogus".to_string());
        assert!(ExperimentRunner::run(&config).is_err());
    }

    #[test]
    fn empty_rating_systems_is_rejected() {
        let mut config = mini_config();
        config.experiment.rating.systems.clear();
        assert!(ExperimentRunner::run(&config).is_err());
    }

    #[test]
    fn run_bounds_by_max_time() {
        let mut config = mini_config();
        config.experiment.duration.max_time = 100.0;
        let result = ExperimentRunner::run(&config).unwrap();
        assert!(
            result.matches_completed < 40,
            "max_time cutoff should stop before all matches complete"
        );
        assert!(
            result.simulated_time_secs <= 100.0 + 1.0,
            "simulated time should respect max_time"
        );
    }

    #[test]
    fn objectives_produce_utility_score() {
        let mut config = mini_config();
        config.experiment.objectives = Some(crate::config::ObjectiveWeightsSpec {
            match_quality: Some(1.0),
            queue_time: Some(0.5),
            rating_accuracy: Some(1.0),
            convergence_speed: None,
            smurf_damage: None,
            false_positive_rate: None,
            streak_frustration: None,
        });
        let result = ExperimentRunner::run(&config).unwrap();
        assert!(result.utility_score.is_some());
    }

    #[test]
    fn expanding_window_matchmaker_runs() {
        let mut config = mini_config();
        config.experiment.matchmaking = MatchmakingSpec {
            algorithm: "expanding_window".to_string(),
            max_queue_time: 60.0,
            params: BTreeMap::from([
                (
                    "tiers".to_string(),
                    serde_yaml::Value::Sequence(vec![
                        serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::from(5.0),
                            serde_yaml::Value::from(25.0),
                        ]),
                        serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::from(10.0),
                            serde_yaml::Value::from(50.0),
                        ]),
                    ]),
                ),
                ("max_window".to_string(), serde_yaml::Value::from(400.0)),
                ("batch_interval".to_string(), serde_yaml::Value::from(10u64)),
            ]),
        };
        let result = ExperimentRunner::run(&config).unwrap();
        assert_eq!(result.matches_completed, 40);
    }

    #[test]
    fn fatigue_outcome_variant_runs() {
        let mut config = mini_config();
        config.experiment.game.variant = Some("fatigue".to_string());
        config.experiment.game.params = BTreeMap::from([(
            "fatigue_decay_rate".to_string(),
            serde_yaml::Value::from(0.001),
        )]);
        let result = ExperimentRunner::run(&config).unwrap();
        assert_eq!(result.matches_completed, 40);
    }

    #[test]
    fn all_metric_collectors_register() {
        let mut config = mini_config();
        config.experiment.metrics = vec![
            "rating_accuracy".to_string(),
            "match_quality".to_string(),
            "queue_time".to_string(),
            "match_inequality".to_string(),
            "ndcg".to_string(),
            "dimensionality_fidelity".to_string(),
            "convergence".to_string(),
            "responsiveness".to_string(),
            "stability".to_string(),
            "streaks".to_string(),
            "population_health".to_string(),
            "smurf".to_string(),
        ];
        let result = ExperimentRunner::run(&config).unwrap();
        for metric in &config.experiment.metrics {
            assert!(
                result.metrics.contains_key(metric),
                "missing metric: {metric}"
            );
        }
    }
}
