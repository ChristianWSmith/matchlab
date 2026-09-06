//! Elo analytical baselines (ticket T-01).
//!
//! 1. Win-rate convergence: with a two class population the observed fraction
//!    of mixed matches won by the higher class converges to the logistic
//!    outcome model's theoretical value.
//! 2. Elo convergence: MAE of rating vs true skill shrinks toward the true
//!    skill ladder from a cold start.
//! 3. Determinism: identical seed ⇒ identical results.

use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use matchlab_experiments::runner::ExperimentRunner;
use matchlab_metrics::MetricResult;
use matchlab_metrics::collector::MetricCollector;
use matchlab_metrics::engine::MetricsEngine;
use matchlab_validation::{
    interleaved_two_class_population, logistic_win_probability, run_loop, single_class_config,
    two_class_config,
};

const SKILL_HIGH: f64 = 1500.0;
const SKILL_LOW: f64 = 1000.0;
const BETA: f64 = 400.0;

fn p_high_wins() -> f64 {
    logistic_win_probability(SKILL_HIGH - SKILL_LOW, BETA)
}

/// Test-only collector: fraction of class-mixed wins for the high class.
#[derive(Default)]
struct ClassWinRate {
    mixed: u64,
    high_wins: u64,
}

impl MetricCollector for ClassWinRate {
    fn name(&self) -> &str {
        "test_class_win_rate"
    }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let skill = |pid: PlayerId| {
            world
                .observations
                .get(&pid)
                .map(|o| o.skill_vector.overall())
                .unwrap_or(f64::NAN)
        };
        let (Some(a), Some(b)) = (mr.team_a.first(), mr.team_b.first()) else {
            return;
        };
        let (sa, sb) = (skill(*a), skill(*b));
        let (high_on_a, high_on_b) = (
            (sa - SKILL_HIGH).abs() < 1e-6,
            (sb - SKILL_HIGH).abs() < 1e-6,
        );
        let (low_on_a, low_on_b) = ((sa - SKILL_LOW).abs() < 1e-6, (sb - SKILL_LOW).abs() < 1e-6);
        if !((high_on_a && low_on_b) || (high_on_b && low_on_a)) {
            return;
        }
        self.mixed += 1;
        let high_team_won =
            (high_on_a && mr.winner == Team::A) || (high_on_b && mr.winner == Team::B);
        if high_team_won {
            self.high_wins += 1;
        }
    }

    fn compute(&self) -> MetricResult {
        if self.mixed == 0 {
            return MetricResult::Scalar(0.0);
        }
        MetricResult::Scalar(self.high_wins as f64 / self.mixed as f64)
    }
}

#[test]
fn win_rate_matches_logistic_ground_truth() {
    let population = interleaved_two_class_population(600, SKILL_HIGH, SKILL_LOW, 1000.0, 1);
    let mut metrics = MetricsEngine::new();
    metrics.register(Box::new(ClassWinRate::default()));
    let outcome = run_loop(population, 1, 20_000, 604_800.0, 42, metrics);

    let observed = match &outcome.metrics["test_class_win_rate"] {
        MetricResult::Scalar(v) => *v,
        other => panic!("expected scalar, got {other:?}"),
    };
    let mixed = {
        // recompute exact mixed count via a second run is wasteful; the
        // collector already sums it, expose it through the fraction preamble:
        // every formed 1v1 match here is class-mixed by interleaving, so use
        // matches_completed as the sample size floor.
        outcome.matches_completed
    };
    assert!(mixed > 10_000, "expected a large sample, got {mixed}");
    let p = p_high_wins();
    let sigma = (p * (1.0 - p) / mixed as f64).sqrt();
    assert!(
        (observed - p).abs() < 6.0 * sigma,
        "observed {observed} vs theory {p} (sigma {sigma})"
    );
}

#[test]
fn elo_converges_toward_true_skill() {
    // Homogeneous population (single skill class). Every visible rating starts
    // at 1000 while true skill is sampled from N(1000, 250), so Elo has real
    // signal to learn per-player skill from match outcomes.
    let config = single_class_config(
        "elo_convergence",
        1,
        1_000,
        5,
        20_000,
        604_800.0,
        &["rating_accuracy"],
    );
    let result = ExperimentRunner::run(&config).expect("run converges");
    let mean_mae = matchlab_validation::summary_mean(&result.metrics["rating_accuracy"]);

    // Cold start: every player rated 1000, true skills drawn from N(1000,250);
    // expected initial MAE is E[|skill - 1000|] = 250 * sqrt(2/pi).
    let cold_mae = 250.0 * (2.0 / std::f64::consts::PI).sqrt();
    assert!(
        mean_mae < cold_mae,
        "final MAE {mean_mae:.1} above cold-ladder MAE {cold_mae:.1}"
    );

    // The real convergence signal is the time-bucketed series: the first
    // nonempty bucket sits at the cold MAE and the last must show a clear
    // reduction toward true skill.
    let bucket_means = match &result.metrics["rating_accuracy_by_time"] {
        MetricResult::TimeSeries { bucket_means } => bucket_means,
        other => panic!("expected time series, got {other:?}"),
    };
    let nonzero: Vec<_> = bucket_means.iter().filter(|&&m| m > 0.0).collect();
    assert!(
        nonzero.len() >= 10,
        "too few populated buckets: {}",
        nonzero.len()
    );
    let first = nonzero.first().expect("nonempty series");
    let last = nonzero.last().expect("nonempty series");
    assert!(
        **last < 0.87 * *first,
        "convergence too weak: first {first:.1}, last {last:.1}"
    );
}

#[test]
fn same_seed_same_results() {
    let config = two_class_config(
        "elo_determinism",
        7,
        500,
        5,
        1_500,
        604_800.0,
        &["rating_accuracy", "match_quality"],
    );
    let a = ExperimentRunner::run(&config).expect("first run");
    let b = ExperimentRunner::run(&config).expect("second run");
    assert_eq!(a.metrics, b.metrics, "metrics differ across same-seed runs");
    assert_eq!(a.matches_completed, b.matches_completed);
    assert_eq!(a.matches_formed, b.matches_formed);
    assert_eq!(a.simulated_time_secs, b.simulated_time_secs);
}
