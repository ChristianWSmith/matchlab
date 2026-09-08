//! Multiple-comparisons corrections: Holm (FWER) and
//! Benjamini–Hochberg (FDR) corrections for a family of p-values. Both are
//! deterministic pure functions of the p-value vector.
/// Correction method for multiple comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Correction {
    Holm,
    BenjaminiHochberg,
}
/// Holm (sequential) correction for family-wise error rate (FWER).
///
/// Given sorted p-values `p₁ ≤ p₂ ≤ … ≤ pₘ`, the adjusted p-value is
/// `p̂ᵢ = min(1, max_{j≤i} (m−j+1)·pⱼ)`. The function accepts unsorted
/// input and returns adjusted values in the same order.
///
/// Reference: Holm, S. (1979). "A simple sequentially rejective multiple
/// test procedure." *Scandinavian Journal of Statistics*, 6(2), 65–70.
pub fn holm(ps: &[f64]) -> Vec<f64> {
    let m = ps.len();
    if m == 0 {
        return Vec::new();
    }
    let mut indexed: Vec<(usize, f64)> = ps.iter().enumerate().map(|(i, &p)| (i, p)).collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut adjusted = vec![0.0f64; m];
    let mut max_val = 0.0f64;
    for (rank, &(_, p)) in indexed.iter().enumerate() {
        let scaled = (m - rank) as f64 * p;
        max_val = max_val.max(scaled);
        adjusted[indexed[rank].0] = max_val.min(1.0);
    }
    adjusted
}
/// Benjamini–Hochberg (step-up) correction for false discovery rate (FDR).
///
/// Given sorted p-values `p₁ ≤ p₂ ≤ … ≤ pₘ`, the adjusted p-value is
/// `p̂ᵢ = min(1, min_{j≥i} (m/j)·pⱼ)`. The function accepts unsorted
/// input and returns adjusted values in the same order.
///
/// Reference: Benjamini, Y. & Hochberg, Y. (1995). "Controlling the false
/// discovery rate: a practical and powerful approach to multiple testing."
/// *Journal of the Royal Statistical Society B*, 57(1), 289–300.
pub fn benjamini_hochberg(ps: &[f64]) -> Vec<f64> {
    let m = ps.len();
    if m == 0 {
        return Vec::new();
    }
    let mut indexed: Vec<(usize, f64)> = ps.iter().enumerate().map(|(i, &p)| (i, p)).collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut adjusted = vec![0.0f64; m];
    let mut min_val = f64::INFINITY;
    for rank in (0..m).rev() {
        let scaled = (m as f64 / (rank + 1) as f64) * indexed[rank].1;
        min_val = min_val.min(scaled);
        adjusted[indexed[rank].0] = min_val.min(1.0);
    }
    adjusted
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn holm_known_example() {
        let ps = vec![0.01, 0.04, 0.03, 0.20];
        let adj = holm(&ps);
        assert!((adj[0] - 0.04).abs() < 1e-9);
        assert!((adj[1] - 0.09).abs() < 1e-9);
        assert!((adj[2] - 0.09).abs() < 1e-9);
        assert!((adj[3] - 0.20).abs() < 1e-9);
    }
    #[test]
    fn bh_known_example() {
        let ps = vec![0.01, 0.04, 0.03, 0.20];
        let adj = benjamini_hochberg(&ps);
        assert!((adj[0] - 0.04).abs() < 1e-12);
        assert!((adj[1] - 0.053333333333).abs() < 1e-6);
        assert!((adj[2] - 0.053333333333).abs() < 1e-6);
        assert!((adj[3] - 0.20).abs() < 1e-12);
    }
    #[test]
    fn adjustments_preserve_ordering() {
        let ps = vec![0.001, 0.01, 0.05, 0.10, 0.50];
        let holm_adj = holm(&ps);
        let bh_adj = benjamini_hochberg(&ps);
        for adj in [&holm_adj, &bh_adj] {
            for i in 0..adj.len() {
                assert!(adj[i] >= ps[i], "adjusted must be ≥ raw");
            }
        }
    }
    #[test]
    fn raw_p_values_untouched() {
        let ps = vec![0.01, 0.04, 0.03, 0.20];
        let orig = ps.clone();
        let _ = holm(&ps);
        let _ = benjamini_hochberg(&ps);
        assert_eq!(ps, orig, "input must not be mutated");
    }
    #[test]
    fn empty_input() {
        assert!(holm(&[]).is_empty());
        assert!(benjamini_hochberg(&[]).is_empty());
    }
    #[test]
    fn single_p_value() {
        let ps = vec![0.05];
        assert_eq!(holm(&ps), vec![0.05]);
        assert_eq!(benjamini_hochberg(&ps), vec![0.05]);
    }
    #[test]
    fn all_equal_p_values() {
        let ps = vec![0.05, 0.05, 0.05];
        let h = holm(&ps);
        for v in &h {
            assert!((v - 0.15).abs() < 1e-9, "holm: {v}");
        }
        let bh = benjamini_hochberg(&ps);
        for v in &bh {
            assert!((v - 0.05).abs() < 1e-9, "bh: {v}");
        }
    }
}
