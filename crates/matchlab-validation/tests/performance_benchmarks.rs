//! Performance Regression Suite.
//!
//! Stable performance benchmarks for key operations.
//! Track wall-clock time to prevent regressions.
use matchlab_core::rng::SimRng;
use std::time::Instant;
/// Benchmark: RNG throughput (operations per second).
#[test]
fn benchmark_rng_throughput() {
    let mut rng = SimRng::from_seed(42);
    let start = Instant::now();
    let mut sum = 0.0f64;
    for _ in 0..1_000_000 {
        sum += rng.sample_normal(0.0, 1.0);
    }
    let elapsed = start.elapsed();
    let ops_per_sec = 1_000_000.0 / elapsed.as_secs_f64();
    assert!(
        ops_per_sec > 100_000.0,
        "RNG throughput too low: {ops_per_sec:.0} ops/sec"
    );
    assert!(sum.is_finite());
}
/// Benchmark: population generation.
#[test]
fn benchmark_population_generation() {
    let start = Instant::now();
    for _ in 0..100 {
        let mut rng = SimRng::from_seed(42);
        let _samples: Vec<f64> = (0..1000)
            .map(|_| rng.sample_normal(1000.0, 250.0))
            .collect();
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "population generation too slow: {:?}",
        elapsed
    );
}
/// Benchmark: match simulation.
#[test]
fn benchmark_match_simulation() {
    let start = Instant::now();
    let mut rng = SimRng::from_seed(42);
    for _ in 0..100_000 {
        let skill_a = rng.sample_normal(1000.0, 250.0);
        let skill_b = rng.sample_normal(1000.0, 250.0);
        let diff = skill_a - skill_b;
        let _p = 1.0 / (1.0 + (-diff / 400.0).exp());
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "match simulation too slow: {:?}",
        elapsed
    );
}
/// Benchmark: Elo rating update.
#[test]
fn benchmark_elo_update() {
    let start = Instant::now();
    let mut rng = SimRng::from_seed(42);
    let k = 32.0_f64;
    let mut rating = 1000.0_f64;
    for _ in 0..100_000 {
        let expected = 1.0 / (1.0 + (-(rating - 1000.0) / 400.0_f64).exp());
        let actual = if rng.gen_range(0.0, 1.0) > 0.5 {
            1.0
        } else {
            0.0
        };
        rating += k * (actual - expected);
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "Elo update too slow: {:?}",
        elapsed
    );
}
/// Benchmark: bootstrap CI computation.
#[test]
fn benchmark_bootstrap_ci() {
    use matchlab_analysis::effect::bootstrap_ci;
    let mut rng = SimRng::from_seed(42);
    let samples: Vec<f64> = (0..100).map(|_| rng.sample_normal(5.0, 1.0)).collect();
    let start = Instant::now();
    for _ in 0..100 {
        let _ci = bootstrap_ci(&samples, 0.95, 42);
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 5000,
        "bootstrap CI too slow: {:?}",
        elapsed
    );
}
/// Benchmark: student-t CI computation.
#[test]
fn benchmark_student_t_ci() {
    use matchlab_analysis::effect::student_t_ci;
    let mut rng = SimRng::from_seed(42);
    let samples: Vec<f64> = (0..100).map(|_| rng.sample_normal(5.0, 1.0)).collect();
    let start = Instant::now();
    for _ in 0..1000 {
        let _ci = student_t_ci(&samples, 0.95);
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "student-t CI too slow: {:?}",
        elapsed
    );
}
