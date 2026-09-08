//! T-106: Cross-Implementation Validation.
//!
//! Validates the Lua script implementations against independently derived
//! mathematical formulas. A disagreement means the Lua script drifted — the
//! reference is never patched to match.
use matchlab_core::match_::{MatchId, MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation, SkillVector};
use matchlab_core::time::SimTime;
use matchlab_game::lua::LuaOutcomeModel;
use matchlab_game::outcome::OutcomeModel;
use matchlab_rating::RatingSystem;
use matchlab_rating::lua::LuaRatingSystem;
use matchlab_validation::observation;
use matchlab_validation::reference::glicko2;
use std::collections::HashMap;
const EPS: f64 = 1e-6;
const BETA: f64 = 400.0;
const DIVISOR: f64 = 400.0 * std::f64::consts::LN_10;
fn obs(id: u64, rating: f64) -> PlayerObservation {
    let mut o = observation(id, rating);
    o.skill_vector = SkillVector::one_dimensional(rating);
    o
}
fn obs_with_rd(id: u64, rating: f64, rd: f64, volatility: f64) -> PlayerObservation {
    let mut o = observation(id, rating);
    o.skill_vector = SkillVector::one_dimensional(rating);
    o.rating_deviation = rd;
    o.volatility = volatility;
    o
}
fn elo_system() -> LuaRatingSystem {
    LuaRatingSystem::load(
        "plugins/rating/elo.lua",
        &serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0\n").unwrap(),
    )
    .unwrap()
}
fn logistic_model() -> LuaOutcomeModel {
    LuaOutcomeModel::load(
        "plugins/game/logistic.lua",
        &serde_yaml::from_str("beta: 400.0\nnoise: 0.0").unwrap(),
    )
    .unwrap()
}
fn make_result(a: Vec<PlayerId>, b: Vec<PlayerId>, winner: Team) -> MatchResult {
    MatchResult {
        match_id: MatchId(1),
        winner,
        team_a: a,
        team_b: b,
        team_a_score: 13.0,
        team_b_score: 5.0,
        player_performances: Vec::new(),
        duration: SimTime::from_secs(1800.0),
        disconnected: false,
        forfeited: false,
        variance: 0.0,
        unexpected_events: Vec::new(),
    }
}
fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}
fn elo_expected(rating_a: f64, rating_b: f64) -> f64 {
    1.0 / (1.0 + 10.0_f64.powf((rating_b - rating_a) / DIVISOR))
}
#[test]
fn elo_update_matches_closed_form() {
    let sys = elo_system();
    let k = 32.0;
    let r_a = 1200.0;
    let r_b = 1000.0;
    let expected = elo_expected(r_a, r_b);
    let closed_form_delta = k * (1.0 - expected);
    let observations: HashMap<PlayerId, PlayerObservation> =
        vec![(PlayerId(1), obs(1, r_a)), (PlayerId(2), obs(2, r_b))]
            .into_iter()
            .collect();
    let result = make_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
    let updates = sys.update(&result, &observations);
    let new_rating_a = updates[&PlayerId(1)].rating;
    let actual_delta = new_rating_a - r_a;
    assert!(
        rel(actual_delta, closed_form_delta) < EPS,
        "Elo delta {actual_delta} vs closed-form {closed_form_delta}"
    );
}
#[test]
fn elo_update_loss_subtracts_correctly() {
    let sys = elo_system();
    let r_a = 1200.0;
    let r_b = 1000.0;
    let expected = elo_expected(r_a, r_b);
    let closed_form_delta = 32.0 * (0.0 - expected);
    let observations: HashMap<PlayerId, PlayerObservation> =
        vec![(PlayerId(1), obs(1, r_a)), (PlayerId(2), obs(2, r_b))]
            .into_iter()
            .collect();
    let result = make_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::B);
    let updates = sys.update(&result, &observations);
    let actual_delta = updates[&PlayerId(1)].rating - r_a;
    assert!(
        rel(actual_delta, closed_form_delta) < EPS,
        "Elo loss delta {actual_delta} vs closed-form {closed_form_delta}"
    );
}
fn logistic_expected(diff: f64) -> f64 {
    1.0 / (1.0 + (-diff / BETA).exp())
}
#[test]
fn logistic_probability_matches_closed_form() {
    let model = logistic_model();
    let diffs = [-800.0, -400.0, -200.0, 0.0, 200.0, 400.0, 800.0];
    for diff in diffs {
        let closed = logistic_expected(diff);
        let r_a = 1000.0 + diff / 2.0;
        let r_b = 1000.0 - diff / 2.0;
        let team_a = vec![obs(1, r_a)];
        let team_b = vec![obs(2, r_b)];
        let script_p = model.win_probability(&team_a, &team_b);
        assert!(
            (script_p - closed).abs() < EPS,
            "logistic P(diff={diff}) script {script_p} vs closed-form {closed}"
        );
    }
}
#[test]
fn logistic_symmetry() {
    let model = logistic_model();
    let team_a = vec![obs(1, 1400.0)];
    let team_b = vec![obs(2, 1000.0)];
    let p_ab = model.win_probability(&team_a, &team_b);
    let p_ba = model.win_probability(&team_b, &team_a);
    assert!(
        (p_ab + p_ba - 1.0).abs() < EPS,
        "logistic symmetry violated: P(A>B)={p_ab} + P(B>A)={p_ba} != 1.0"
    );
}
fn match_quality_closed_form(avg_a: f64, avg_b: f64) -> f64 {
    (1.0 - (avg_a - avg_b).abs() / 400.0).clamp(0.0, 1.0)
}
#[test]
fn match_quality_formula_balanced_teams() {
    let q = match_quality_closed_form(1200.0, 1200.0);
    assert!(
        (q - 1.0).abs() < EPS,
        "balanced quality must be 1.0, got {q}"
    );
}
#[test]
fn match_quality_formula_gap_400() {
    let q = match_quality_closed_form(1400.0, 1000.0);
    assert!(
        (q - 0.0).abs() < EPS,
        "400-gap quality must be 0.0, got {q}"
    );
}
#[test]
fn match_quality_formula_clamps_negative() {
    let q = match_quality_closed_form(2000.0, 1000.0);
    assert!(
        (q - 0.0).abs() < EPS,
        "large gap must clamp to 0.0, got {q}"
    );
}
#[test]
fn match_quality_formula_partial_gap() {
    let q = match_quality_closed_form(1300.0, 1000.0);
    let expected = 1.0 - 300.0 / 400.0;
    assert!(
        (q - expected).abs() < EPS,
        "partial gap quality {q} vs {expected}"
    );
}
#[test]
fn glicko2_single_period_matches_reference() {
    let sys = LuaRatingSystem::load(
        "plugins/rating/glicko2.lua",
        &serde_yaml::from_str(
            "initial_rating: 1500.0\ninitial_rd: 200.0\ninitial_volatility: 0.06\ntau: 0.5\nepsilon: 0.000001",
        )
        .unwrap(),
    )
    .unwrap();
    let mut player = obs(1, 1500.0);
    player.rating_deviation = 200.0;
    player.volatility = 0.06;
    let mut map = HashMap::new();
    map.insert(PlayerId(1), player);
    map.insert(PlayerId(2), obs_with_rd(2, 1400.0, 30.0, 0.06));
    let result = make_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
    let updates = sys.update(&result, &map);
    let script_r = updates[&PlayerId(1)].rating;
    let script_rd = updates[&PlayerId(1)].rating_deviation;
    let script_vol = updates[&PlayerId(1)].volatility;
    let (mu, phi) = glicko2::scale(1500.0, 200.0);
    let (mu_j, phi_j) = glicko2::scale(1400.0, 30.0);
    let out = glicko2::single_period(
        mu,
        phi,
        0.06,
        &[glicko2::Opponent {
            mu: mu_j,
            phi: phi_j,
            outcome: 1.0,
        }],
        0.5,
        1e-6,
    );
    let (ref_r, ref_rd) = glicko2::unscale(out.mu, out.phi);
    assert!(
        rel(script_r, ref_r) < 1e-3,
        "Glicko-2 rating: script {script_r} vs reference {ref_r}"
    );
    assert!(
        rel(script_rd, ref_rd) < 1e-3,
        "Glicko-2 RD: script {script_rd} vs reference {ref_rd}"
    );
    assert!(
        (script_vol - out.sigma).abs() < 1e-3,
        "Glicko-2 volatility: script {script_vol} vs reference {}",
        out.sigma
    );
}
#[test]
fn glicko2_loss_decreases_rating() {
    let sys = LuaRatingSystem::load(
        "plugins/rating/glicko2.lua",
        &serde_yaml::from_str(
            "initial_rating: 1500.0\ninitial_rd: 200.0\ninitial_volatility: 0.06\ntau: 0.5\nepsilon: 0.000001",
        )
        .unwrap(),
    )
    .unwrap();
    let mut player = obs(1, 1500.0);
    player.rating_deviation = 200.0;
    player.volatility = 0.06;
    let mut map = HashMap::new();
    map.insert(PlayerId(1), player);
    map.insert(PlayerId(2), obs(2, 1600.0));
    let result = make_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::B);
    let updates = sys.update(&result, &map);
    assert!(
        updates[&PlayerId(1)].rating < 1500.0,
        "Glicko-2 loss must decrease rating"
    );
}
#[test]
fn elo_and_logistic_agree_on_favored_team() {
    let sys = elo_system();
    let model = logistic_model();
    let team_a = vec![obs(1, 1500.0)];
    let team_b = vec![obs(2, 1000.0)];
    let elo_pred = sys.predict(&team_a, &team_b);
    let logistic_pred = model.win_probability(&team_a, &team_b);
    assert!(
        (elo_pred - logistic_pred).abs() < 1e-3,
        "Elo predict {elo_pred} and logistic prob {logistic_pred} must agree for 500-gap"
    );
    assert!(elo_pred > 0.5, "higher-rated team must be favored");
}
