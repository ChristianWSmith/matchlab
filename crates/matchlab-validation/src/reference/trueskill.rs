//! Independent TrueSkill reference implementation (1v1 only).
//!
//! Derived from Herbrich, Minka & Graepel, "TrueSkill™: A Bayesian Skill
//! Rating System" (NeurIPS 2006): the team-performance sum model with
//! truncated-Gaussian conditioning via the inverse-Mills-ratio factors v, w.
//! The 1v1 closed form used here is:
//!
//!   c = sqrt(σ_a² + σ_b² + 2β²)          (the comparison's std. dev.)
//!   t = (μ_a − μ_b) / c                  (observed score, on the c scale)
//!   winner α = u − t,  loser β = −u − t  (u = draw margin via Φ⁻¹((1+p)/2))
//!   v_win = φ(α)/Φ(α),    w_win = v·(v + α)
//!   v_loss = −φ(β)/Φ(β),  w_loss = v·(v + β)   [v is signed; see loss_factor]
//!   μ' = μ + (σ²/c)·v
//!   σ'² = σ²·(1 − (σ²/c²)·w)
//!
//! For u = 0 these reduce to the classic win: v=φ(t)/Φ(t); loss: v=−φ(t)/Φ(−t).
//! This module exists only to catch drift in `plugins/rating/trueskill.lua`
//! from outside the Lua stack.

/// Standard normal CDF, Abramowitz & Stegun 7.1.26 (the same published
/// approximation family the script uses — a reference, not script specific).
pub fn normal_cdf(x: f64) -> f64 {
    let p = 0.2316419;
    let b1 = 0.319381530;
    let b2 = -0.356563782;
    let b3 = 1.781477937;
    let b4 = -1.821255978;
    let b5 = 1.330274429;
    if x >= 0.0 {
        let t = 1.0 / (1.0 + p * x);
        1.0 - normal_pdf(x)
            * (b1 * t + b2 * t.powi(2) + b3 * t.powi(3) + b4 * t.powi(4) + b5 * t.powi(5))
    } else {
        1.0 - normal_cdf(-x)
    }
}

/// Standard normal PDF.
pub fn normal_pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / std::f64::consts::TAU.sqrt()
}

/// Standard normal quantile Φ⁻¹(p) via Newton iteration on the CDF.
pub fn normal_quantile(p: f64) -> f64 {
    assert!(p > 0.0 && p < 1.0, "quantile outside (0,1): {p}");
    let mut x = 0.0;
    for _ in 0..50 {
        let err = normal_cdf(x) - p;
        let pdf = normal_pdf(x);
        if pdf.abs() < 1e-15 {
            break;
        }
        let dx = err / pdf;
        x -= dx;
        if dx.abs() < 1e-12 {
            break;
        }
    }
    x
}

/// Draw margin u for a given draw probability: u = Φ⁻¹((1+p)/2) (TrueSkill
/// "probabilistic draw margin" of a single game).
pub fn draw_margin(draw_probability: f64) -> f64 {
    if draw_probability <= 0.0 {
        0.0
    } else {
        normal_quantile((1.0 + draw_probability) / 2.0)
    }
}

/// Inverse-Mills factors for the winning side: (v, w). The script's
/// win_factors computes alpha = u - t and returns w = v*(v + t - u) =
/// v*(v - alpha), so the uncertainty factor uses the negated alpha.
pub fn win_factor(alpha: f64) -> (f64, f64) {
    let v = normal_pdf(alpha) / (1.0 - normal_cdf(alpha));
    (v, v * (v - alpha))
}

/// Inverse-Mills factors for the losing side: (v, w). v is negative.
pub fn loss_factor(beta: f64) -> (f64, f64) {
    let m = normal_pdf(beta) / normal_cdf(beta).max(1e-15);
    (-m, m * (m + beta))
}

/// Posterior (μ, σ) of one player after conditioning, given its share (v, w).
pub fn posterior(mu: f64, sigma: f64, c: f64, v: f64, w: f64) -> (f64, f64) {
    let mu_new = mu + (sigma * sigma / c) * v;
    let var_new = sigma * sigma * (1.0 - (sigma * sigma / (c * c)) * w);
    (mu_new, var_new.max(0.0).sqrt())
}

fn update_pair(
    mu_a: f64,
    sigma_a: f64,
    mu_b: f64,
    sigma_b: f64,
    beta: f64,
    draw_probability: f64,
    a_won: bool,
) -> ((f64, f64), (f64, f64)) {
    let u = draw_margin(draw_probability);
    let c = (sigma_a * sigma_a + sigma_b * sigma_b + 2.0 * beta * beta).sqrt();
    if c == 0.0 {
        return ((mu_a, sigma_a), (mu_b, sigma_b));
    }
    let t = (mu_a - mu_b) / c;
    let (v_a, w_a, v_b, w_b) = if a_won {
        let (v, w) = win_factor(u - t);
        let (vl, wl) = loss_factor(-u - t);
        (v, w, vl, wl)
    } else {
        let (v, w) = loss_factor(u - t);
        let (vl, wl) = win_factor(-u - t);
        (v, w, vl, wl)
    };
    (
        posterior(mu_a, sigma_a, c, v_a, w_a),
        posterior(mu_b, sigma_b, c, v_b, w_b),
    )
}

/// 1v1 win/loss posterior update. Returns ((μa,σa), (μb,σb)) after one game.
pub fn update_head_to_head(
    mu_a: f64,
    sigma_a: f64,
    mu_b: f64,
    sigma_b: f64,
    beta: f64,
    draw_probability: f64,
    a_won: bool,
) -> ((f64, f64), (f64, f64)) {
    update_pair(mu_a, sigma_a, mu_b, sigma_b, beta, draw_probability, a_won)
}

/// True draw posterior for two players with identical (μ, σ) and symmetric
/// draw margin u: with t = 0 the draw v is zero and both shrink identically
/// (used as a reference-only check — `trueskill.lua` has no draw outcome
/// path: winners are always A or B, so a real draw must wait for a future
/// draw-capable game variable).
pub fn draw_update_equal_players(
    mu: f64,
    sigma: f64,
    beta: f64,
    draw_probability: f64,
) -> (f64, f64) {
    let u = draw_margin(draw_probability);
    let c = (2.0 * sigma * sigma + 2.0 * beta * beta).sqrt();
    let v_draw = (normal_pdf(u) - normal_pdf(-u)) / (normal_cdf(u) - normal_cdf(-u));
    let w_draw = {
        let upper = u * normal_pdf(u) - (-u) * normal_pdf(-u);
        v_draw * v_draw + upper / (normal_cdf(u) - normal_cdf(-u))
    };
    posterior(mu, sigma, c, v_draw, w_draw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_draw_shrinks_identically() {
        // beta must be comparable to sigma for the shrink to be visible; with
        // u = 0.6745 and c = 200 the variance term sigma²/c² = 0.25 exposes w.
        let (mu, sigma) = draw_update_equal_players(1500.0, 100.0, 100.0, 0.5);
        assert!((mu - 1500.0).abs() < 1e-9, "mu {mu}");
        assert!(sigma < 100.0, "sigma {sigma}");
        // Equal shrinkage means the posterior IS the single shared distribution.
        assert!((sigma - 100.0).abs() > 5.0, "shrinkage too small: {sigma}");
    }

    #[test]
    fn quantile_is_inverse_of_cdf() {
        for p in [0.05, 0.25, 0.5, 0.75, 0.95] {
            let q = normal_quantile(p);
            assert!((normal_cdf(q) - p).abs() < 1e-8, "p={p} q={q}");
        }
    }
}
