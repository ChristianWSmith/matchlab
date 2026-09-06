//! Independent Glicko-2 reference implementation.
//!
//! Derived from Mark Glickman, "Example of the Glicko-2 system" (2012):
//! <https://www.glicko.net/glicko/glicko2.pdf>. Equations are cited inline
//! (eq. 1-10). This exists only to catch drift in `plugins/rating/glicko2.lua`
//! from *outside* the Lua stack — when they disagree, the script is the bug.

/// The scale constant (eq. 1-2): ratings live on a scale where 173.7178 units
/// ≈ 1 unit of "deviation-scale" skill.
pub const SCALE: f64 = 173.7178;
/// The rating center the paper shifts to μ = 0.
pub const RATING_CENTER: f64 = 1500.0;

/// g(φ) (eq. 3): shrinks an opponent's deviation contribution.
pub fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / std::f64::consts::PI.powi(2)).sqrt()
}

/// E[μ, μ_j, φ_j] (eq. 4): expected score against opponent j.
pub fn e(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

/// A single rated game in a rating period.
#[derive(Debug, Clone, Copy)]
pub struct Opponent {
    pub mu: f64,
    pub phi: f64,
    /// 1.0 for a win, 0.0 for a loss.
    pub outcome: f64,
}

/// Result of one rating period on the Glicko scale.
#[derive(Debug, Clone, Copy)]
pub struct PeriodResult {
    pub mu: f64,
    pub phi: f64,
    pub sigma: f64,
}

/// The f(x) helper of the volatility iteration (eq. 7).
fn big_f(x: f64, delta: f64, phi: f64, v: f64, a: f64, tau: f64) -> f64 {
    let ex = x.exp();
    (ex * (delta * delta - phi * phi - v - ex))
        / (2.0 * (phi * phi + v + ex) * (phi * phi + v + ex))
        - (x - a) / (tau * tau)
}

/// One full Glicko-2 rating period for a single player against a list of
/// opponents with the same outcome semantics as Glickman's paper (steps 2-4
/// plus 5.1: the players' ratings entering the period use the *pre-period*
/// σ in φ*; the volatility iteration is Newton's method with the
/// bisection-style safeguard of eq. 7-9).
pub fn single_period(
    mu: f64,
    phi: f64,
    sigma: f64,
    opponents: &[Opponent],
    tau: f64,
    epsilon: f64,
) -> PeriodResult {
    let v_inv: f64 = opponents
        .iter()
        .map(|o| {
            let gj = g(o.phi);
            let e_j = e(mu, o.mu, o.phi);
            gj * gj * e_j * (1.0 - e_j)
        })
        .sum::<f64>();
    if v_inv == 0.0 {
        return PeriodResult { mu, phi, sigma };
    }
    let v = 1.0 / v_inv;
    let delta: f64 = v * opponents
        .iter()
        .map(|o| g(o.phi) * (o.outcome - e(mu, o.mu, o.phi)))
        .sum::<f64>();

    // Step 5.2-5.6: solve f(x) = 0 for x = ln σ².
    let a = (sigma * sigma).ln();
    let b_start = if delta * delta > phi * phi + v {
        (delta * delta - phi * phi - v).ln()
    } else {
        let mut k = 1.0;
        while big_f(a - k * tau, delta, phi, v, a, tau) < 0.0 {
            k += 1.0;
        }
        a - k * tau
    };

    let mut fa = big_f(a, delta, phi, v, a, tau);
    let mut fb = big_f(b_start, delta, phi, v, a, tau);
    let (mut a_val, mut b_val) = (a, b_start);
    while (b_val - a_val).abs() > epsilon {
        let c = a_val + (a_val - b_val) * fa / (fb - fa);
        let fc = big_f(c, delta, phi, v, a, tau);
        if fc * fb <= 0.0 {
            a_val = b_val;
            fa = fb;
        } else {
            fa /= 2.0;
        }
        b_val = c;
        fb = fc;
    }
    let sigma_prime = ((a_val + b_val) / 4.0).exp();

    // Step 6-7: φ* from φ² + σ'²; φ' from combining φ* and v; μ' from Δ.
    let phi_star = (phi * phi + sigma_prime * sigma_prime).sqrt();
    let phi_prime = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
    let delta_numer: f64 = opponents
        .iter()
        .map(|o| g(o.phi) * (o.outcome - e(mu, o.mu, o.phi)))
        .sum::<f64>();
    let mu_prime = mu + phi_prime * phi_prime * delta_numer;

    PeriodResult {
        mu: mu_prime,
        phi: phi_prime,
        sigma: sigma_prime,
    }
}

/// Scale a rating-period input to the Glicko scale (eq. 1-2).
pub fn scale(rating: f64, rd: f64) -> (f64, f64) {
    ((rating - RATING_CENTER) / SCALE, rd / SCALE)
}

/// Scale a Glicko-scale result back to rating units (eq. 10).
pub fn unscale(mu: f64, phi: f64) -> (f64, f64) {
    (RATING_CENTER + SCALE * mu, SCALE * phi)
}

/// Volatility growth over an idle period (the φ* step with no games):
/// φ' = sqrt(φ² + σ²), rating unchanged. Glicko-2 applies this between rating
/// periods for players who did not play; the Lua script has no idle entry
/// point, so this reference is used to compose the T-02 two-period chain
/// (the grown RD is fed back into the script as ordinary pre-period input).
pub fn idle_step(phi: f64, sigma: f64) -> f64 {
    (phi * phi + sigma * sigma).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_worked_example_single_period() {
        // Player 1500/200/0.06 (scaled μ=0, φ=200/173.7178, σ=0.06) plays one
        // rating period: win vs 1400/30, loss vs 1550/100, loss vs 1700/300.
        let (mu, phi) = scale(1500.0, 200.0);
        let (m1400, p1400) = scale(1400.0, 30.0);
        let (m1550, p1550) = scale(1550.0, 100.0);
        let (m1700, p1700) = scale(1700.0, 300.0);
        let opponents = [
            Opponent {
                mu: m1400,
                phi: p1400,
                outcome: 1.0,
            },
            Opponent {
                mu: m1550,
                phi: p1550,
                outcome: 0.0,
            },
            Opponent {
                mu: m1700,
                phi: p1700,
                outcome: 0.0,
            },
        ];
        let out = single_period(mu, phi, 0.06, &opponents, 0.5, 1e-6);
        let (r, rd) = unscale(out.mu, out.phi);
        assert!((r - 1464.06).abs() < 0.01, "r {r}");
        assert!((rd - 151.52).abs() < 0.01, "rd {rd}");
        assert!((out.sigma - 0.05999).abs() < 1e-5, "sigma {}", out.sigma);
    }
}
#[cfg(test)]
mod probe_tests {
    use super::*;
    #[test]
    #[ignore]
    fn probe_single_win() {
        let (mu, phi) = scale(1500.0, 200.0);
        let (mu_j, phi_j) = scale(1400.0, 30.0);
        let out = single_period(
            mu,
            phi,
            0.06,
            &[Opponent {
                mu: mu_j,
                phi: phi_j,
                outcome: 1.0,
            }],
            0.5,
            1e-6,
        );
        let (r, rd) = unscale(out.mu, out.phi);
        println!("SINGLE-WIN REF => r={r} rd={rd} sigma={}", out.sigma);
    }
}
