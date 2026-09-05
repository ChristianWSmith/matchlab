use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use std::collections::HashMap;

use crate::hooks::LuaHooks;
use crate::system::{ObservationType, RatingState, RatingSystem};

/// Glicko-2 scale conversion factor: rating/RD are divided by this.
const SCALE: f64 = 173.7178;

/// Default center of the Glicko-2 rating scale.
const RATING_CENTER: f64 = 1500.0;

pub struct GlickoConfig {
    pub initial_rating: f64,
    pub initial_rd: f64,
    pub initial_volatility: f64,
    pub tau: f64,
    pub epsilon: f64,
}

pub struct Glicko2RatingSystem {
    pub config: GlickoConfig,
    hooks: Option<LuaHooks>,
}

impl Glicko2RatingSystem {
    pub fn new(config: GlickoConfig) -> Self {
        Self { config, hooks: None }
    }

    pub fn with_hooks(config: GlickoConfig, hooks: LuaHooks) -> Self {
        Self {
            config,
            hooks: Some(hooks),
        }
    }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let initial_rating = value
            .get("initial_rating")
            .and_then(serde_yaml::Value::as_f64)?;
        Some(Self::new(GlickoConfig {
            initial_rating,
            initial_rd: value
                .get("initial_rd")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(350.0),
            initial_volatility: value
                .get("initial_volatility")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(0.06),
            tau: value
                .get("tau")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(0.5),
            epsilon: value
                .get("epsilon")
                .and_then(serde_yaml::Value::as_f64)
                .unwrap_or(0.000001),
        }))
    }

    fn g(&self, phi: f64) -> f64 {
        1.0 / (1.0 + 3.0 * phi * phi / std::f64::consts::PI.powi(2)).sqrt()
    }

    fn e(&self, mu: f64, mu_j: f64, phi_j: f64) -> f64 {
        let gj = self.g(phi_j);
        1.0 / (1.0 + (-gj * (mu - mu_j)).exp())
    }

    /// Iterate to find the new volatility via Newton-Raphson on the Glicko-2
    /// `f` function (Glickman 2012, steps 5.2–5.6).
    fn new_volatility(
        &self,
        sigma: f64,
        delta: f64,
        phi: f64,
        v: f64,
        tau: f64,
        epsilon: f64,
    ) -> f64 {
        let a = sigma.powi(2).ln();
        let big_f = |x: f64| {
            let ex = x.exp();
            (ex * (delta.powi(2) - phi.powi(2) - v - ex))
                / (2.0 * (phi.powi(2) + v + ex).powi(2))
                - (x - a) / tau.powi(2)
        };

        let b = if delta.powi(2) > phi.powi(2) + v {
            (delta.powi(2) - phi.powi(2) - v).ln()
        } else {
            let mut k = 1;
            while big_f(a - k as f64 * tau) < 0.0 {
                k += 1;
            }
            a - k as f64 * tau
        };

        let mut fa = big_f(a);
        let mut fb = big_f(b);
        let mut a_val = a;
        let mut b_val = b;

        while (b_val - a_val).abs() > epsilon {
            let c = a_val + (a_val - b_val) * fa / (fb - fa);
            let fc = big_f(c);
            if fc * fb <= 0.0 {
                a_val = b_val;
                fa = fb;
            } else {
                fa = fa / 2.0;
            }
            b_val = c;
            fb = fc;
        }

        // Converged x = ln(σ²) is the midpoint of [a_val, b_val]; σ' = e^(x/2).
        ((a_val + b_val) / 4.0).exp()
    }

    /// Scale a single player's state to Glicko-2 units (μ, φ, σ).
    fn scale(rating: f64, rd: f64, volatility: f64) -> (f64, f64, f64) {
        ((rating - RATING_CENTER) / SCALE, rd / SCALE, volatility)
    }

    /// Convert a scaled (μ, φ) back to (rating, RD).
    fn unscale(mu: f64, phi: f64) -> (f64, f64) {
        (RATING_CENTER + SCALE * mu, SCALE * phi)
    }

    /// Update one player against a list of opponents (each with μ, φ, outcome).
    fn update_player(
        &self,
        player: (f64, f64, f64),
        opponents: &[(f64, f64, f64)],
        epsilon: f64,
        tau: f64,
    ) -> (f64, f64, f64) {
        let (mu, phi, sigma) = player;

        let mut v_inv = 0.0;
        let mut delta_numer = 0.0;
        for &(mu_j, phi_j, outcome) in opponents {
            let gj = self.g(phi_j);
            let e = self.e(mu, mu_j, phi_j);
            v_inv += gj * gj * e * (1.0 - e);
            delta_numer += gj * (outcome - e);
        }
        if v_inv == 0.0 {
            return player;
        }
        let v = 1.0 / v_inv;
        let delta = v * delta_numer;

        let sigma_prime = self.new_volatility(sigma, delta, phi, v, tau, epsilon);
        let phi_star = (phi * phi + sigma_prime * sigma_prime).sqrt();
        let phi_prime = 1.0 / (1.0 / phi_star.powi(2) + 1.0 / v).sqrt();
        let mu_prime = mu + phi_prime.powi(2) * delta_numer;

        (mu_prime, phi_prime, sigma_prime)
    }
}

impl RatingSystem for Glicko2RatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_rating,
            rating_deviation: self.config.initial_rd,
            volatility: self.config.initial_volatility,
            games_played: 0,
        }
    }

    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let avg_a = team_a.iter().map(|p| p.rating).sum::<f64>() / team_a.len() as f64;
        let avg_b = team_b.iter().map(|p| p.rating).sum::<f64>() / team_b.len() as f64;
        let diff = avg_a - avg_b;
        1.0 / (1.0 + (-diff / 400.0).exp())
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mut updates = HashMap::new();
        let epsilon = self.config.epsilon;
        let tau = self.config.tau;

        let collect_opponents = |ids: &[PlayerId], outcome: f64, obs: &HashMap<PlayerId, PlayerObservation>| {
            let mut opponents = Vec::new();
            for pid in ids {
                if let Some(o) = obs.get(pid) {
                    let (mu, phi, _) = Self::scale(o.rating, o.rating_deviation, o.volatility);
                    opponents.push((mu, phi, outcome));
                }
            }
            opponents
        };

        let outcome_a = if match_result.winner == Team::A { 1.0 } else { 0.0 };
        let outcome_b = 1.0 - outcome_a;
        let opp_b = collect_opponents(&match_result.team_b, outcome_a, observations);
        let opp_a = collect_opponents(&match_result.team_a, outcome_b, observations);

        for &pid in &match_result.team_a {
            if let Some(obs) = observations.get(&pid) {
                let (mu, phi, sigma) = Self::scale(obs.rating, obs.rating_deviation, obs.volatility);
                let (mu_p, phi_p, sigma_p) = self.update_player((mu, phi, sigma), &opp_b, epsilon, tau);
                let (rating, rd) = Self::unscale(mu_p, phi_p);
                let rating = self
                    .hooks
                    .as_ref()
                    .and_then(|h| h.call_rating_bounds())
                    .map(|(floor, ceiling)| rating.clamp(floor, ceiling))
                    .unwrap_or(rating);
                updates.insert(
                    pid,
                    RatingState {
                        rating,
                        rating_deviation: rd,
                        volatility: sigma_p,
                        games_played: obs.games_played + 1,
                    },
                );
            }
        }
        for &pid in &match_result.team_b {
            if let Some(obs) = observations.get(&pid) {
                let (mu, phi, sigma) = Self::scale(obs.rating, obs.rating_deviation, obs.volatility);
                let (mu_p, phi_p, sigma_p) = self.update_player((mu, phi, sigma), &opp_a, epsilon, tau);
                let (rating, rd) = Self::unscale(mu_p, phi_p);
                let rating = self
                    .hooks
                    .as_ref()
                    .and_then(|h| h.call_rating_bounds())
                    .map(|(floor, ceiling)| rating.clamp(floor, ceiling))
                    .unwrap_or(rating);
                updates.insert(
                    pid,
                    RatingState {
                        rating,
                        rating_deviation: rd,
                        volatility: sigma_p,
                        games_played: obs.games_played + 1,
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

    fn obs(id: u64, rating: f64, rd: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank { tier: "unranked".into(), division: 1 },
            rating_deviation: rd,
            volatility: 0.06,
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

    fn glicko() -> Glicko2RatingSystem {
        Glicko2RatingSystem::new(GlickoConfig {
            initial_rating: 1500.0,
            initial_rd: 350.0,
            initial_volatility: 0.06,
            tau: 0.5,
            epsilon: 0.000001,
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
    fn equal_ratings_produce_50_percent() {
        let sys = glicko();
        let p = sys.predict(&[obs(1, 1500.0, 350.0)], &[obs(2, 1500.0, 350.0)]);
        assert!((p - 0.5).abs() < 0.001, "p = {p}");
    }

    #[test]
    fn initialize_returns_configured_values() {
        let sys = glicko();
        let state = sys.initialize(PlayerId(0));
        assert_eq!(state.rating, 1500.0);
        assert_eq!(state.rating_deviation, 350.0);
        assert_eq!(state.volatility, 0.06);
        assert_eq!(state.games_played, 0);
    }

    #[test]
    fn winner_gains_loser_loses() {
        let sys = glicko();
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1500.0, 200.0));
        obs_map.insert(PlayerId(2), obs(2, 1500.0, 200.0));

        let updates = sys.update(&mr, &obs_map);
        assert!(updates[&PlayerId(1)].rating > 1500.0);
        assert!(updates[&PlayerId(2)].rating < 1500.0);
    }

    #[test]
    fn rd_decreases_after_games() {
        let sys = glicko();
        let mr = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1500.0, 350.0));
        obs_map.insert(PlayerId(2), obs(2, 1500.0, 350.0));

        let updates = sys.update(&mr, &obs_map);
        assert!(updates[&PlayerId(1)].rating_deviation < 350.0);
        assert!(updates[&PlayerId(2)].rating_deviation < 350.0);
    }

    /// Glickman (2012) worked example: player rated 1500/RD 200/vol 0.06 plays one
    /// rating period with win vs 1400/30, loss vs 1550/100, loss vs 1700/300.
    /// Expected r' = 1464.06, RD' = 151.52, σ' = 0.05999.
    #[test]
    fn matches_glickman_paper_worked_example() {
        let sys = glicko();
        let player = Glicko2RatingSystem::scale(1500.0, 200.0, 0.06);
        let opp = |rating: f64, rd: f64, outcome: f64| {
            let (mu, phi, _) = Glicko2RatingSystem::scale(rating, rd, 0.06);
            (mu, phi, outcome)
        };
        let opponents = vec![
            opp(1400.0, 30.0, 1.0),
            opp(1550.0, 100.0, 0.0),
            opp(1700.0, 300.0, 0.0),
        ];

        let (mu_p, phi_p, sigma_p) =
            sys.update_player(player, &opponents, sys.config.epsilon, sys.config.tau);
        let (rating, rd) = Glicko2RatingSystem::unscale(mu_p, phi_p);

        assert!(
            (rating - 1464.06).abs() < 1.0,
            "rating {} != ~1464.06",
            rating
        );
        assert!(
            (rd - 151.52).abs() < 1.0,
            "RD {} != ~151.52",
            rd
        );
        assert!(
            (sigma_p - 0.05999).abs() < 0.002,
            "vol {} != ~0.05999",
            sigma_p
        );
    }

    #[test]
    fn upset_changes_volatility_more_than_expected() {
        let sys = glicko();
        // Weak player (1200) upsets strong player (1800).
        let mr_upset = make_match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut obs_map = HashMap::new();
        obs_map.insert(PlayerId(1), obs(1, 1200.0, 100.0));
        obs_map.insert(PlayerId(2), obs(2, 1800.0, 100.0));
        let updates = sys.update(&mr_upset, &obs_map);
        // Volatility should rise substantially after an unexpected result.
        assert!(updates[&PlayerId(1)].volatility > 0.06);
        assert!(updates[&PlayerId(2)].volatility > 0.06);
    }

    #[test]
    fn update_increments_games_played() {
        let sys = glicko();
        let mr = make_match_result(
            vec![PlayerId(1), PlayerId(2)],
            vec![PlayerId(3), PlayerId(4)],
            Team::A,
        );
        let mut obs_map = HashMap::new();
        for i in 1..=4u64 {
            obs_map.insert(PlayerId(i), obs(i, 1500.0, 350.0));
        }
        let updates = sys.update(&mr, &obs_map);
        for pid in [PlayerId(1), PlayerId(2), PlayerId(3), PlayerId(4)] {
            assert_eq!(updates[&pid].games_played, 11);
        }
    }

    #[test]
    fn from_yaml_round_trips_config() {
        let yaml = serde_yaml::from_str(
            "initial_rating: 1400.0\ninitial_rd: 300.0\ninitial_volatility: 0.05\ntau: 0.6\nepsilon: 0.00001\n",
        )
        .unwrap();
        let sys = Glicko2RatingSystem::from_yaml(&yaml).unwrap();
        assert_eq!(sys.config.initial_rating, 1400.0);
        assert_eq!(sys.config.initial_rd, 300.0);
        assert_eq!(sys.config.initial_volatility, 0.05);
        assert_eq!(sys.config.tau, 0.6);
        assert_eq!(sys.config.epsilon, 0.00001);
    }

    #[test]
    fn from_yaml_uses_defaults_for_missing() {
        let yaml = serde_yaml::from_str("initial_rating: 1500.0\n").unwrap();
        let sys = Glicko2RatingSystem::from_yaml(&yaml).unwrap();
        assert_eq!(sys.config.initial_rd, 350.0);
        assert_eq!(sys.config.initial_volatility, 0.06);
        assert_eq!(sys.config.tau, 0.5);
    }

    #[test]
    fn information_budget_only_winloss() {
        let sys = glicko();
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }
}