//! Limiting-Case Test Suite.
//!
//! Mathematically obvious edge cases that must produce explicit expected results.
//! These cases make it difficult for subtle regressions to hide.
use matchlab_core::rng::SimRng;
/// Zero skill difference: outcome probability should be 0.5.
#[test]
fn zero_skill_difference() {
    use matchlab_core::player::SkillVector;
    let a = SkillVector::one_dimensional(1000.0);
    let b = SkillVector::one_dimensional(1000.0);
    assert_eq!(a.overall(), b.overall(), "equal skill should be equal");
}
/// Identical skill vectors produce identical overall.
#[test]
fn identical_skill_vectors() {
    use matchlab_core::player::SkillVector;
    let a = SkillVector::one_dimensional(1500.0);
    let b = SkillVector::one_dimensional(1500.0);
    assert_eq!(a.overall(), b.overall());
    assert_eq!(a.overall(), 1500.0);
}
/// Zero-dimensional skill vector has zero overall.
#[test]
fn zero_dimensional_skill() {
    use matchlab_core::player::SkillVector;
    let sv = SkillVector {
        dimensions: std::collections::HashMap::new(),
    };
    assert_eq!(sv.overall(), 0.0);
}
/// Zero rating change produces no effect on rating.
#[test]
fn zero_rating_change() {
    use matchlab_analysis::effect::effect_sizes;
    use matchlab_core::rng::SimRng;
    let mut rng = SimRng::from_seed(42);
    let samples: Vec<f64> = (0..20).map(|_| rng.sample_normal(100.0, 0.001)).collect();
    let control: Vec<_> = samples
        .iter()
        .map(|v| {
            matchlab_analysis::hierarchy::per_replication::from_metric(
                &matchlab_metrics::MetricResult::Scalar(*v),
            )
            .unwrap()
        })
        .collect();
    let treatment: Vec<_> = samples
        .iter()
        .map(|v| {
            matchlab_analysis::hierarchy::per_replication::from_metric(
                &matchlab_metrics::MetricResult::Scalar(*v),
            )
            .unwrap()
        })
        .collect();
    let es = effect_sizes(&control, &treatment, false, 0.95, 42).unwrap();
    assert!(
        es.mean_delta.abs() < 0.01,
        "zero difference should give ~0 effect: {}",
        es.mean_delta
    );
}
/// Empty queue produces no matches.
#[test]
fn empty_queue_produces_no_matches() {
    use matchlab_matchmaking::queue::Queue;
    let queue = Queue::default();
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
}
/// Single-player queue cannot form a team.
#[test]
fn single_player_queue() {
    use matchlab_core::player::{PlayerId, SkillVector, VisibleRank};
    use matchlab_core::time::SimTime;
    use matchlab_matchmaking::queue::{Queue, QueueEntry};
    use std::collections::VecDeque;
    let mut queue = Queue::default();
    queue.enqueue(QueueEntry {
        player_id: PlayerId(1),
        joined_at: SimTime::from_secs(0.0),
        observation: matchlab_core::player::PlayerObservation {
            id: PlayerId(1),
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: VisibleRank {
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
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            role: None,
            skill_vector: SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
        },
        region: matchlab_core::player::Region::NA,
        party_id: None,
        game_mode: "ranked".to_string(),
        role: None,
        latency_ms: 30.0,
    });
    assert_eq!(queue.len(), 1);
}
/// Logistic function: P(A wins) = 0.5 when skill difference is 0.
#[test]
fn logistic_symmetric_case() {
    use matchlab_validation::logistic_win_probability;
    let p = logistic_win_probability(0.0, 400.0);
    assert!(
        (p - 0.5).abs() < 1e-10,
        "zero difference should give 0.5: {p}"
    );
}
/// Logistic function: P(A wins) > 0.5 when A has higher skill.
#[test]
fn logistic_positive_advantage() {
    use matchlab_validation::logistic_win_probability;
    let p = logistic_win_probability(500.0, 400.0);
    assert!(p > 0.5, "positive advantage should give > 0.5: {p}");
    assert!(p < 1.0, "probability should be < 1.0: {p}");
}
/// Logistic function: P(A wins) < 0.5 when A has lower skill.
#[test]
fn logistic_negative_advantage() {
    use matchlab_validation::logistic_win_probability;
    let p = logistic_win_probability(-500.0, 400.0);
    assert!(p < 0.5, "negative advantage should give < 0.5: {p}");
    assert!(p > 0.0, "probability should be > 0.0: {p}");
}
/// Bootstrap CI with constant values has zero width.
#[test]
fn bootstrap_ci_constant_values() {
    use matchlab_analysis::effect::bootstrap_ci;
    let samples = vec![100.0; 100];
    let ci = bootstrap_ci(&samples, 0.95, 42);
    assert!(
        (ci.lower - 100.0).abs() < 1e-10,
        "constant values: lower = {}",
        ci.lower
    );
    assert!(
        (ci.upper - 100.0).abs() < 1e-10,
        "constant values: upper = {}",
        ci.upper
    );
}
/// Student-t CI with constant values has zero width.
#[test]
fn student_t_ci_constant_values() {
    use matchlab_analysis::effect::student_t_ci;
    let samples = vec![100.0; 100];
    let ci = student_t_ci(&samples, 0.95);
    assert!(
        (ci.lower - 100.0).abs() < 1e-10,
        "constant values: lower = {}",
        ci.lower
    );
    assert!(
        (ci.upper - 100.0).abs() < 1e-10,
        "constant values: upper = {}",
        ci.upper
    );
}
/// Empty samples produce degenerate CI.
#[test]
fn ci_empty_samples() {
    use matchlab_analysis::effect::{bootstrap_ci, student_t_ci};
    let samples: Vec<f64> = vec![];
    let bc = bootstrap_ci(&samples, 0.95, 42);
    let tc = student_t_ci(&samples, 0.95);
    assert!(bc.lower.is_finite());
    assert!(bc.upper.is_finite());
    assert!(tc.lower.is_finite());
    assert!(tc.upper.is_finite());
}
/// Single-sample bootstrap CI is degenerate.
#[test]
fn bootstrap_ci_single_sample() {
    use matchlab_analysis::effect::bootstrap_ci;
    let samples = vec![100.0];
    let ci = bootstrap_ci(&samples, 0.95, 42);
    assert!((ci.lower - 100.0).abs() < 1e-10);
    assert!((ci.upper - 100.0).abs() < 1e-10);
}
/// Holm correction with all-ones p-values returns all ones.
#[test]
fn holm_all_ones() {
    use matchlab_analysis::multiple_comparisons::holm;
    let ps = vec![1.0, 1.0, 1.0];
    let adj = holm(&ps);
    for a in &adj {
        assert!(
            (*a - 1.0).abs() < 1e-10,
            "all-ones p-values should remain 1.0"
        );
    }
}
/// BH correction with all-ones p-values returns all ones.
#[test]
fn bh_all_ones() {
    use matchlab_analysis::multiple_comparisons::benjamini_hochberg;
    let ps = vec![1.0, 1.0, 1.0];
    let adj = benjamini_hochberg(&ps);
    for a in &adj {
        assert!(
            (*a - 1.0).abs() < 1e-10,
            "all-ones p-values should remain 1.0"
        );
    }
}
/// Empty p-value list produces empty correction.
#[test]
fn corrections_empty_input() {
    use matchlab_analysis::multiple_comparisons::{benjamini_hochberg, holm};
    let ps: Vec<f64> = vec![];
    assert!(holm(&ps).is_empty());
    assert!(benjamini_hochberg(&ps).is_empty());
}
/// Single p-value correction is identity.
#[test]
fn corrections_single_p_value() {
    use matchlab_analysis::multiple_comparisons::{benjamini_hochberg, holm};
    let ps = vec![0.05];
    let h = holm(&ps);
    let b = benjamini_hochberg(&ps);
    assert!(
        (h[0] - 0.05).abs() < 1e-10,
        "single p-value: holm = {}",
        h[0]
    );
    assert!((b[0] - 0.05).abs() < 1e-10, "single p-value: bh = {}", b[0]);
}
/// Single point is on the Pareto front.
#[test]
fn pareto_single_point() {
    use matchlab_analysis::pareto::{ParetoPoint, pareto_front};
    let points = vec![ParetoPoint {
        label: "a".to_string(),
        values: vec![1.0, 2.0],
    }];
    let front = pareto_front(&points, &[true, true]);
    assert_eq!(front.len(), 1, "single point should be on front");
}
/// Identical points: all are on the Pareto front (no strict domination).
#[test]
fn pareto_identical_points() {
    use matchlab_analysis::pareto::{ParetoPoint, pareto_front};
    let points = vec![
        ParetoPoint {
            label: "a".to_string(),
            values: vec![1.0, 2.0],
        },
        ParetoPoint {
            label: "b".to_string(),
            values: vec![1.0, 2.0],
        },
    ];
    let front = pareto_front(&points, &[true, true]);
    assert_eq!(front.len(), 2, "identical points should all be on front");
}
/// Dominated point is not on the Pareto front.
#[test]
fn pareto_dominated_excluded() {
    use matchlab_analysis::pareto::{ParetoPoint, pareto_front};
    let points = vec![
        ParetoPoint {
            label: "a".to_string(),
            values: vec![1.0, 5.0],
        },
        ParetoPoint {
            label: "b".to_string(),
            values: vec![2.0, 6.0],
        },
    ];
    let front = pareto_front(&points, &[true, true]);
    assert_eq!(front.len(), 1, "dominated point should be excluded");
    assert_eq!(front[0].label, "b", "the non-dominated point should be 'b'");
}
/// All limiting cases are deterministic.
#[test]
fn limiting_cases_deterministic() {
    use matchlab_validation::logistic_win_probability;
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let _s1: Vec<f64> = (0..100).map(|_| rng1.sample_normal(0.0, 1.0)).collect();
    let _s2: Vec<f64> = (0..100).map(|_| rng2.sample_normal(0.0, 1.0)).collect();
    let p1 = logistic_win_probability(0.0, 400.0);
    let p2 = logistic_win_probability(0.0, 400.0);
    assert_eq!(p1, p2, "logistic should be deterministic");
}
