use crate::config::{ParameterSpec, SearchSpace};
use rand::Rng;
use rand::SeedableRng;
use std::collections::BTreeMap;

pub fn latin_hypercube_sample(
    space: &SearchSpace,
    n_points: u64,
    seed: u64,
) -> Vec<BTreeMap<String, f64>> {
    let n = n_points as usize;
    let params: Vec<(&String, &ParameterSpec)> = space.parameters.iter().collect();
    let d = params.len();

    if d == 0 || n == 0 {
        return vec![];
    }

    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut samples = vec![vec![0.0f64; d]; n];

    for col in samples.iter_mut().take(d) {
        let mut perm: Vec<usize> = (0..n).collect();
        fisher_yates(&mut perm, &mut rng);

        for (i_idx, &i) in perm.iter().enumerate() {
            let u: f64 = rng.gen_range(0.0..1.0);
            col[i] = (i_idx as f64 + u) / n as f64;
        }
    }

    let mut result = Vec::with_capacity(n);
    for (_i, row) in samples.iter().enumerate().take(n) {
        let mut point = BTreeMap::new();
        for (j, (name, spec)) in params.iter().enumerate() {
            let val = match spec {
                ParameterSpec::Float {
                    bounds,
                    log_scale: true,
                } => {
                    let log_low = bounds[0].ln();
                    let log_high = bounds[1].ln();
                    (log_low + row[j] * (log_high - log_low)).exp()
                }
                ParameterSpec::Float { bounds, .. } => bounds[0] + row[j] * (bounds[1] - bounds[0]),
                ParameterSpec::Categorical { values } => {
                    let idx = (row[j] * values.len() as f64).floor() as usize;
                    let idx = idx.min(values.len() - 1);
                    idx as f64
                }
            };
            point.insert(name.to_string(), val);
        }
        result.push(point);
    }
    result
}

pub fn random_sample(space: &SearchSpace, n_points: u64, seed: u64) -> Vec<BTreeMap<String, f64>> {
    let n = n_points as usize;
    let params: Vec<(&String, &ParameterSpec)> = space.parameters.iter().collect();
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut result = Vec::with_capacity(n);

    for _ in 0..n {
        let mut point = BTreeMap::new();
        for (name, spec) in &params {
            let val = match spec {
                ParameterSpec::Float { bounds, .. } => rng.gen_range(bounds[0]..bounds[1]),
                ParameterSpec::Categorical { values } => {
                    let idx = rng.gen_range(0..values.len());
                    idx as f64
                }
            };
            point.insert(name.to_string(), val);
        }
        result.push(point);
    }
    result
}

fn fisher_yates(arr: &mut [usize], rng: &mut rand::rngs::SmallRng) {
    for i in (1..arr.len()).rev() {
        let j = rng.gen_range(0..=i);
        arr.swap(i, j);
    }
}

pub fn point_to_vector(point: &BTreeMap<String, f64>, param_order: &[String]) -> Vec<f64> {
    param_order.iter().map(|p| point[p]).collect()
}

pub fn vector_to_point(vec: &[f64], param_order: &[String]) -> BTreeMap<String, f64> {
    param_order
        .iter()
        .zip(vec.iter())
        .map(|(k, &v)| (k.clone(), v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lhs_produces_correct_count() {
        let mut params = BTreeMap::new();
        params.insert(
            "x".to_string(),
            ParameterSpec::Float {
                bounds: [0.0, 1.0],
                log_scale: false,
            },
        );
        let space = SearchSpace { parameters: params };
        let points = latin_hypercube_sample(&space, 5, 42);
        assert_eq!(points.len(), 5);
        for p in &points {
            assert!(p["x"] >= 0.0 && p["x"] <= 1.0);
        }
    }

    #[test]
    fn lhs_is_deterministic() {
        let mut params = BTreeMap::new();
        params.insert(
            "x".to_string(),
            ParameterSpec::Float {
                bounds: [0.0, 1.0],
                log_scale: false,
            },
        );
        let space = SearchSpace { parameters: params };
        let a = latin_hypercube_sample(&space, 5, 42);
        let b = latin_hypercube_sample(&space, 5, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn lhs_log_scale() {
        let mut params = BTreeMap::new();
        params.insert(
            "x".to_string(),
            ParameterSpec::Float {
                bounds: [1.0, 1000.0],
                log_scale: true,
            },
        );
        let space = SearchSpace { parameters: params };
        let points = latin_hypercube_sample(&space, 10, 42);
        for p in &points {
            assert!(p["x"] >= 1.0 && p["x"] <= 1000.0);
        }
    }
}
