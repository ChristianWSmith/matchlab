use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use std::collections::HashMap;

use crate::hooks::LuaHooks;
use crate::system::{ObservationType, RatingState, RatingSystem};

const SQRT_2PI: f64 = 2.5066282746310002;

fn normal_pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / SQRT_2PI
}

/// Abramowitz–Stegun 7.1.26 approximation of the standard normal CDF.
fn normal_cdf(x: f64) -> f64 {
    const P: f64 = 0.2316419;
    const B1: f64 = 0.319381530;
    const B2: f64 = -0.356563782;
    const B3: f64 = 1.781477937;
    const B4: f64 = -1.821255978;
    const B5: f64 = 1.330274429;
    if x >= 0.0 {
        let t = 1.0 / (1.0 + P * x);
        1.0 - normal_pdf(x)
            * (B1 * t + B2 * t.powi(2) + B3 * t.powi(3) + B4 * t.powi(4) + B5 * t.powi(5))
    } else {
        1.0 - normal_cdf(-x)
    }
}

/// Standard normal quantile (probit) via Newton iteration on the CDF.
fn normal_quantile(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
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

/// Truncated-Gaussian update factors for a comparison outcome.
///
/// Model: z = (d − μ_diff)/c ~ N(0, 1) with d = P_A − P_B team performances,
/// μ_diff = μ_A − μ_B, c = sqrt(σA² + σB² + (n_A+n_B)β²). A wins ⟺ z > u − t,
/// B wins ⟺ z < −u − t, where t = μ_diff/c and u = draw_margin/c.
///
/// Returns (v, w) such that μ' = μ + (σ²/c)·v and σ²' = σ²·(1 − (σ²/c²)·w).
struct UpdateFactors {
    v: f64,
    w: f64,
}

fn win_factors(t: f64, u: f64) -> UpdateFactors {
    let alpha = u - t;
    let v = normal_pdf(alpha) / (1.0 - normal_cdf(alpha));
    UpdateFactors {
        v,
        w: v * (v + t - u),
    }
}

fn loss_factors(t: f64, u: f64) -> UpdateFactors {
    let beta = -u - t;
    let m = normal_pdf(beta) / normal_cdf(beta).max(1e-15);
    UpdateFactors {
        v: -m,
        w: m * (m + beta),
    }
}

pub struct TrueSkillConfig {
    pub initial_mean: f64,
    pub initial_variance: f64,
    pub beta: f64,
    pub dynamics: f64,
    pub draw_probability: f64,
}

pub struct TrueSkillRatingSystem {
    pub config: TrueSkillConfig,
    hooks: Option<LuaHooks>,
}

impl TrueSkillRatingSystem {
    pub fn new(config: TrueSkillConfig) -> Self {
        Self {
            config,
            hooks: None,
        }
    }

    pub fn with_hooks(config: TrueSkillConfig, hooks: LuaHooks) -> Self {
        Self {
            config,
            hooks: Some(hooks),
        }
    }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let initial_mean = value
            .get("initial_mean")
            .and_then(serde_yaml::Value::as_f64)
            .or_else(|| {
                value
                    .get("initial_rating")
                    .and_then(serde_yaml::Value::as_f64)
            })?;
        Some(Self::new(TrueSkillConfig {
            initial_mean,
            initial_variance: value
                .get("initial_variance")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(350.0),
            beta: value
                .get("beta")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(400.0),
            dynamics: value
                .get("dynamics")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(0.1),
            draw_probability: value
                .get("draw_probability")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(0.0),
        }))
    }

    fn draw_margin(&self) -> f64 {
        if self.config.draw_probability <= 0.0 {
            0.0
        } else {
            normal_quantile((1.0 + self.config.draw_probability) / 2.0)
        }
    }
}

impl RatingSystem for TrueSkillRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_mean,
            rating_deviation: self.config.initial_variance.sqrt(),
            volatility: 0.0,
            games_played: 0,
        }
    }

    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let avg_a = team_a.iter().map(|p| p.rating).sum::<f64>() / team_a.len() as f64;
        let avg_b = team_b.iter().map(|p| p.rating).sum::<f64>() / team_b.len() as f64;
        let diff = avg_a - avg_b;
        1.0 / (1.0 + (-diff / self.config.beta).exp())
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mut updates = HashMap::new();
        let team_a_won = match_result.winner == Team::A;

        let collect = |ids: &[PlayerId]| -> (f64, f64, Vec<f64>) {
            let mut sum_mu = 0.0;
            let mut sum_var = 0.0;
            let mut sigmas = Vec::new();
            for pid in ids {
                if let Some(o) = observations.get(pid) {
                    let mu = o.rating;
                    let sigma = o.rating_deviation;
                    let var = sigma * sigma + self.config.dynamics * self.config.dynamics;
                    sum_mu += mu;
                    sum_var += var;
                    sigmas.push(sigma);
                }
            }
            (sum_mu, sum_var, sigmas)
        };

        let (sum_mu_a, sum_var_a, sigmas_a) = collect(&match_result.team_a);
        let (sum_mu_b, sum_var_b, sigmas_b) = collect(&match_result.team_b);
        let n = match_result.team_a.len() + match_result.team_b.len();
        let c = (sum_var_a + sum_var_b + n as f64 * self.config.beta * self.config.beta).sqrt();
        if c == 0.0 {
            return updates;
        }
        let u = self.draw_margin();
        let t = (sum_mu_a - sum_mu_b) / c;
        let factors_a = if team_a_won {
            win_factors(t, u)
        } else {
            loss_factors(t, u)
        };
        let factors_b = if team_a_won {
            loss_factors(t, u)
        } else {
            win_factors(t, u)
        };

        for (i, &pid) in match_result.team_a.iter().enumerate() {
            if let Some(o) = observations.get(&pid) {
                let sigma = sigmas_a.get(i).copied().unwrap_or(o.rating_deviation);
                let var = sigma * sigma;
                let mu_new = o.rating + (var / c) * factors_a.v;
                let var_new = var * (1.0 - (var / (c * c)) * factors_a.w);
                let rating = self
                    .hooks
                    .as_ref()
                    .and_then(|h| h.call_rating_bounds())
                    .map(|(floor, ceiling)| mu_new.clamp(floor, ceiling))
                    .unwrap_or(mu_new);
                updates.insert(
                    pid,
                    RatingState {
                        rating,
                        rating_deviation: var_new.sqrt().max(1e-6),
                        volatility: 0.0,
                        games_played: o.games_played + 1,
                    },
                );
            }
        }
        for (i, &pid) in match_result.team_b.iter().enumerate() {
            if let Some(o) = observations.get(&pid) {
                let sigma = sigmas_b.get(i).copied().unwrap_or(o.rating_deviation);
                let var = sigma * sigma;
                let mu_new = o.rating + (var / c) * factors_b.v;
                let var_new = var * (1.0 - (var / (c * c)) * factors_b.w);
                let rating = self
                    .hooks
                    .as_ref()
                    .and_then(|h| h.call_rating_bounds())
                    .map(|(floor, ceiling)| mu_new.clamp(floor, ceiling))
                    .unwrap_or(mu_new);
                updates.insert(
                    pid,
                    RatingState {
                        rating,
                        rating_deviation: var_new.sqrt().max(1e-6),
                        volatility: 0.0,
                        games_played: o.games_played + 1,
                    },
                );
            }
        }

        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::MatchId;
    use matchlab_core::player::{DetectionFlag, SkillVector, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64, sigma: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
                division: 1,
            },
            rating_deviation: sigma,
            volatility: 0.0,
            games_played: 10,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn trueskill() -> TrueSkillRatingSystem {
        TrueSkillRatingSystem::new(TrueSkillConfig {
            initial_mean: 1500.0,
            initial_variance: 350.0,
            beta: 400.0,
            dynamics: 0.0,
            draw_probability: 0.0,
        })
    }

    fn make_match_result(
        team_a: Vec<PlayerId>,
        team_b: Vec<PlayerId>,
        winner: Team,
    ) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner,
            team_a,
            team_b,
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: Vec::new(),
            duration: matchlab_core::time::SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn equal_skills_produce_50_percent() {
        let sys = trueskill();
        let p = sys.predict(&[obs(1, 1500.0, 30.0)], &[obs(2, 1500.0, 30.0)]);
        assert!((p - 0.5).abs() < 0.001, "p = {p}");
    }

    #[test]
    fn winner_mean_increases_loser_mean_decreases() {
        let sys = trueskill();
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1500.0, 30.0));
        obs_map.insert(PlayerId(2), obs(2, 1500.0, 30.0));

        let updates = sys.update(&mr, &obs_map);
        assert!(updates[&PlayerId(1)].rating > 1500.0);
        assert!(updates[&PlayerId(2)].rating < 1500.0);
    }

    #[test]
    fn variance_decreases_after_game() {
        let sys = trueskill();
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1500.0, 200.0));
        obs_map.insert(PlayerId(2), obs(2, 1500.0, 200.0));

        let updates = sys.update(&mr, &obs_map);
        assert!(updates[&PlayerId(1)].rating_deviation < 200.0);
        assert!(updates[&PlayerId(2)].rating_deviation < 200.0);
    }

    #[test]
    fn team_games_update_all_members() {
        let sys = trueskill();
        let mr = make_match_result(
            vec![PlayerId(1), PlayerId(2), PlayerId(3)],
            vec![PlayerId(4), PlayerId(5), PlayerId(6)],
            Team::A,
        );
        let mut obs_map = HashMap::new();
        for i in 1..=6u64 {
            obs_map.insert(PlayerId(i), obs(i, 1500.0, 100.0));
        }
        let updates = sys.update(&mr, &obs_map);
        for pid in [PlayerId(1), PlayerId(2), PlayerId(3)] {
            assert!(updates[&pid].rating > 1500.0);
        }
        for pid in [PlayerId(4), PlayerId(5), PlayerId(6)] {
            assert!(updates[&pid].rating < 1500.0);
        }
        for pid in [
            PlayerId(1),
            PlayerId(2),
            PlayerId(3),
            PlayerId(4),
            PlayerId(5),
            PlayerId(6),
        ] {
            assert_eq!(updates[&pid].games_played, 11);
        }
    }

    #[test]
    fn initialize_returns_configured_values() {
        let sys = trueskill();
        let state = sys.initialize(PlayerId(0));
        assert_eq!(state.rating, 1500.0);
        assert!((state.rating_deviation - 350.0_f64.sqrt()).abs() < 1e-9);
        assert_eq!(state.games_played, 0);
    }

    #[test]
    fn from_yaml_round_trips_config() {
        let yaml = serde_yaml::from_str(
            "initial_mean: 1200.0\ninitial_variance: 250.0\nbeta: 500.0\ndynamics: 0.2\ndraw_probability: 0.1\n",
        )
        .unwrap();
        let sys = TrueSkillRatingSystem::from_yaml(&yaml).unwrap();
        assert_eq!(sys.config.initial_mean, 1200.0);
        assert_eq!(sys.config.initial_variance, 250.0);
        assert_eq!(sys.config.beta, 500.0);
        assert_eq!(sys.config.dynamics, 0.2);
        assert_eq!(sys.config.draw_probability, 0.1);
    }

    #[test]
    fn from_yaml_accepts_initial_rating_alias() {
        let yaml = serde_yaml::from_str("initial_rating: 1300.0\n").unwrap();
        let sys = TrueSkillRatingSystem::from_yaml(&yaml).unwrap();
        assert_eq!(sys.config.initial_mean, 1300.0);
        assert_eq!(sys.config.initial_variance, 350.0);
        assert_eq!(sys.config.beta, 400.0);
    }

    #[test]
    fn from_yaml_requires_initial_mean() {
        let yaml = serde_yaml::from_str("{}").unwrap();
        assert!(TrueSkillRatingSystem::from_yaml(&yaml).is_none());
    }

    #[test]
    fn information_budget_only_winloss() {
        let sys = trueskill();
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn normal_cdf_sanity() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!(normal_cdf(1.96) > 0.974 && normal_cdf(1.96) < 0.976);
        assert!(normal_cdf(-1.0) < 0.5);
    }

    #[test]
    fn normal_quantile_inverts_cdf() {
        for p in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let q = normal_quantile(p);
            assert!((normal_cdf(q) - p).abs() < 1e-6, "p={p} q={q}");
        }
    }
}
