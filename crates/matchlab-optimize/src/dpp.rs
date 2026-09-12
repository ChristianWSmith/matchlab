use ndarray::Array2;
use rand::Rng;
use rand::SeedableRng;
use std::collections::BTreeMap;

pub fn k_dpp_sample(
    candidates: &[BTreeMap<String, f64>],
    acquisition_scores: &[f64],
    param_order: &[String],
    k: usize,
    seed: u64,
) -> Vec<usize> {
    let n = candidates.len();
    if n == 0 || k == 0 {
        return Vec::new();
    }
    if k >= n {
        return (0..n).collect();
    }

    let valid_scores: Vec<f64> = acquisition_scores
        .iter()
        .map(|&s| if s.is_finite() && s > 0.0 { s } else { 0.0 })
        .collect();
    let all_zero = valid_scores.iter().all(|&s| s <= 0.0);
    if all_zero {
        return greedy_top_k(acquisition_scores, k);
    }

    let x = build_candidate_matrix(candidates, param_order);
    let similarity = compute_rbf_similarity(&x);

    sequential_greedy_dpp(&valid_scores, &similarity, k, seed)
}

fn build_candidate_matrix(
    candidates: &[BTreeMap<String, f64>],
    param_order: &[String],
) -> Array2<f64> {
    let n = candidates.len();
    let d = param_order.len();
    let mut x = Array2::<f64>::zeros((n, d));
    for (i, point) in candidates.iter().enumerate() {
        for (j, name) in param_order.iter().enumerate() {
            x[[i, j]] = point[name];
        }
    }
    x
}

fn compute_rbf_similarity(x: &Array2<f64>) -> Array2<f64> {
    let n = x.nrows();

    let mut median_sq_dist = Vec::new();
    for i in 0..n.min(200) {
        for j in (i + 1)..n.min(200) {
            let mut sq = 0.0;
            for dim in 0..x.ncols() {
                let diff = x[[i, dim]] - x[[j, dim]];
                sq += diff * diff;
            }
            median_sq_dist.push(sq);
        }
    }
    median_sq_dist.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = median_sq_dist
        .get(median_sq_dist.len() / 2)
        .copied()
        .unwrap_or(1.0)
        .max(1e-10);
    let bandwidth = median.ln().max(1e-10);

    let mut sim = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        sim[[i, i]] = 1.0;
        for j in (i + 1)..n {
            let mut sq = 0.0;
            for dim in 0..x.ncols() {
                let diff = x[[i, dim]] - x[[j, dim]];
                sq += diff * diff;
            }
            let val = (-sq / (2.0 * bandwidth)).exp();
            sim[[i, j]] = val;
            sim[[j, i]] = val;
        }
    }
    sim
}

fn sequential_greedy_dpp(
    scores: &[f64],
    similarity: &Array2<f64>,
    k: usize,
    seed: u64,
) -> Vec<usize> {
    let n = scores.len();
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut selected: Vec<usize> = Vec::new();
    let mut remaining: Vec<bool> = vec![true; n];

    for _ in 0..k {
        let mut best_idx = None;
        let mut best_val = f64::NEG_INFINITY;

        for j in 0..n {
            if !remaining[j] {
                continue;
            }

            let mut diversity_penalty = 1.0;
            for &s in &selected {
                diversity_penalty *= 1.0 - similarity[[j, s]];
            }
            diversity_penalty = diversity_penalty.max(1e-10);

            let combined = scores[j] * diversity_penalty;
            let noise: f64 = rng.gen_range(0.0..1e-10);
            let candidate_val = combined + noise;

            if candidate_val > best_val {
                best_val = candidate_val;
                best_idx = Some(j);
            }
        }

        if let Some(idx) = best_idx {
            selected.push(idx);
            remaining[idx] = false;
        } else {
            break;
        }
    }

    selected
}

fn greedy_top_k(scores: &[f64], k: usize) -> Vec<usize> {
    let mut indexed: Vec<(usize, f64)> = scores.iter().enumerate().map(|(i, &s)| (i, s)).collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    indexed.into_iter().take(k).map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k_dpp_sample_empty() {
        let result = k_dpp_sample(&[], &[], &[], 3, 42);
        assert!(result.is_empty());
    }

    #[test]
    fn k_dpp_sample_k_zero() {
        let candidates = vec![BTreeMap::from([("x".to_string(), 1.0)])];
        let result = k_dpp_sample(&candidates, &[1.0], &["x".to_string()], 0, 42);
        assert!(result.is_empty());
    }

    #[test]
    fn k_dpp_sample_k_geq_n() {
        let candidates: Vec<BTreeMap<String, f64>> = (0..5)
            .map(|i| BTreeMap::from([("x".to_string(), i as f64)]))
            .collect();
        let scores: Vec<f64> = (0..5).map(|i| i as f64).collect();
        let result = k_dpp_sample(&candidates, &scores, &["x".to_string()], 10, 42);
        assert_eq!(result.len(), 5);
        assert_eq!(result, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn k_dpp_sample_selects_correct_count() {
        let candidates: Vec<BTreeMap<String, f64>> = (0..20)
            .map(|i| BTreeMap::from([("x".to_string(), i as f64)]))
            .collect();
        let scores: Vec<f64> = (0..20).map(|i| (i as f64).sin().abs() + 0.1).collect();
        let result = k_dpp_sample(&candidates, &scores, &["x".to_string()], 5, 42);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn k_dpp_sample_indices_are_unique() {
        let candidates: Vec<BTreeMap<String, f64>> = (0..30)
            .map(|i| {
                BTreeMap::from([
                    ("x".to_string(), i as f64),
                    ("y".to_string(), (i * 3) as f64),
                ])
            })
            .collect();
        let scores: Vec<f64> = (0..30)
            .map(|i| ((i as f64) * 0.5).sin().abs() + 0.1)
            .collect();
        let result = k_dpp_sample(
            &candidates,
            &scores,
            &["x".to_string(), "y".to_string()],
            8,
            42,
        );
        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), result.len(), "indices must be unique");
    }

    #[test]
    fn k_dpp_sample_indices_in_range() {
        let candidates: Vec<BTreeMap<String, f64>> = (0..15)
            .map(|i| BTreeMap::from([("x".to_string(), i as f64)]))
            .collect();
        let scores: Vec<f64> = (0..15).map(|i| (i as f64).sqrt()).collect();
        let result = k_dpp_sample(&candidates, &scores, &["x".to_string()], 6, 42);
        for &idx in &result {
            assert!(idx < 15, "index {idx} out of range");
        }
    }

    #[test]
    fn k_dpp_sample_zero_scores_falls_back() {
        let candidates: Vec<BTreeMap<String, f64>> = (0..10)
            .map(|i| BTreeMap::from([("x".to_string(), i as f64)]))
            .collect();
        let scores = vec![0.0; 10];
        let result = k_dpp_sample(&candidates, &scores, &["x".to_string()], 3, 42);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn k_dpp_sample_deterministic() {
        let candidates: Vec<BTreeMap<String, f64>> = (0..20)
            .map(|i| BTreeMap::from([("x".to_string(), i as f64)]))
            .collect();
        let scores: Vec<f64> = (0..20).map(|i| (i as f64).sin().abs() + 0.1).collect();
        let r1 = k_dpp_sample(&candidates, &scores, &["x".to_string()], 5, 42);
        let r2 = k_dpp_sample(&candidates, &scores, &["x".to_string()], 5, 42);
        assert_eq!(r1, r2);
    }

    #[test]
    fn greedy_top_k_basic() {
        let scores = vec![1.0, 5.0, 3.0, 2.0, 4.0];
        let result = greedy_top_k(&scores, 3);
        assert_eq!(result, vec![1, 4, 2]);
    }

    #[test]
    fn rbf_similarity_diagonal_is_one() {
        let x = Array2::from_shape_vec((5, 2), (0..10).map(|i| i as f64).collect()).unwrap();
        let sim = compute_rbf_similarity(&x);
        for i in 0..5 {
            assert!((sim[[i, i]] - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn rbf_similarity_symmetric() {
        let x = Array2::from_shape_vec((4, 3), (0..12).map(|i| i as f64).collect()).unwrap();
        let sim = compute_rbf_similarity(&x);
        for i in 0..4 {
            for j in 0..4 {
                assert!(
                    (sim[[i, j]] - sim[[j, i]]).abs() < 1e-10,
                    "asymmetry at [{i},{j}]"
                );
            }
        }
    }
}
