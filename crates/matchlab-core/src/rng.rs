use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
/// Derive a per-stream seed from a base seed and a stream index (spec §13.7).
///
/// The same `(seed, index)` pair always maps to the same derived seed, so an
/// experiment reproduces exactly for a given experiment seed. Indices 1–4 are
/// reserved for population/game/arrival/behavior (kept identical to
/// `SeedManager`); 5 and 6 are matchmaker and master.
pub fn derive(seed: u64, index: u64) -> u64 {
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    index.hash(&mut h);
    h.finish()
}
/// Independent seeds for the loop's stochastic subsystems (spec §13.7 /).
///
/// Each subsystem draws from its own `SimRng`, so two experiments that share a
/// stream get identical draws on it (the common-random-numbers mechanism)
/// while varying another stream in isolation. `master` seeds the `World` RNG
/// that no subsystem consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSeeds {
    pub master: u64,
    pub game: u64,
    pub matchmaker: u64,
    pub behavior: u64,
}
impl StreamSeeds {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            master: derive(seed, 6),
            game: derive(seed, 2),
            matchmaker: derive(seed, 5),
            behavior: derive(seed, 4),
        }
    }
}
/// Deterministic RNG wrapper around `SmallRng`, seeded from a single `u64`.
///
/// A `SimRng` is fully reproducible: the same seed produces the same sequence
/// of draws, which is what makes every experiment deterministic (spec §2.4).
#[derive(Debug)]
pub struct SimRng {
    inner: SmallRng,
}
impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: SmallRng::seed_from_u64(seed),
        }
    }
    pub fn gen_range(&mut self, low: f64, high: f64) -> f64 {
        self.inner.gen_range(low..high)
    }
    pub fn gen_bool(&mut self, p: f64) -> bool {
        self.inner.gen_bool(p)
    }
    /// Draw from a normal distribution via Box-Muller (spec §4.6).
    pub fn sample_normal(&mut self, mean: f64, stddev: f64) -> f64 {
        let u: f64 = self.inner.r#gen();
        let v: f64 = self.inner.r#gen();
        let z = (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos();
        mean + stddev * z
    }
    pub fn gen_u64(&mut self) -> u64 {
        self.inner.r#gen()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_seed_same_sequence() {
        let mut a = SimRng::from_seed(42);
        let mut b = SimRng::from_seed(42);
        let seq_a: Vec<f64> = (0..100).map(|_| a.gen_range(0.0, 1.0)).collect();
        let seq_b: Vec<f64> = (0..100).map(|_| b.gen_range(0.0, 1.0)).collect();
        assert_eq!(seq_a, seq_b);
    }
    #[test]
    fn different_seeds_diverge() {
        let mut a = SimRng::from_seed(1);
        let mut b = SimRng::from_seed(2);
        let seq_a: Vec<f64> = (0..100).map(|_| a.gen_range(0.0, 1.0)).collect();
        let seq_b: Vec<f64> = (0..100).map(|_| b.gen_range(0.0, 1.0)).collect();
        assert_ne!(seq_a, seq_b);
    }
    #[test]
    fn sample_normal_mean_matches_requested() {
        let mut rng = SimRng::from_seed(7);
        let draws: Vec<f64> = (0..100_000)
            .map(|_| rng.sample_normal(1000.0, 250.0))
            .collect();
        let mean = draws.iter().sum::<f64>() / draws.len() as f64;
        assert!((mean - 1000.0).abs() < 5.0, "mean drifted: {mean}");
    }
    #[test]
    fn gen_bool_respects_probability() {
        let mut rng = SimRng::from_seed(9);
        let p = 0.5;
        let mut count = 0;
        for _ in 0..10_000 {
            if rng.gen_bool(p) {
                count += 1;
            }
        }
        let rate = count as f64 / 10_000.0;
        assert!((rate - p).abs() < 0.05, "rate drifted: {rate}");
    }
    #[test]
    fn gen_u64_is_varied() {
        let mut rng = SimRng::from_seed(13);
        let first = rng.gen_u64();
        let second = rng.gen_u64();
        assert_ne!(first, second);
    }
}
