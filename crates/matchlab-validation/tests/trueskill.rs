//! TrueSkill script-vs-reference baselines (ticket T-02).
//!
//! Only the 1v1 case is validated here (the team-performance-sum >2 case is
//! deferred). `trueskill.lua` has no draw outcome path (winners are always A
//! or B), so the "draw" sub-test is validated in two honest halves: (a) the
//! draw-margin `u` machinery is exercised end-to-end via a win path with
//! `draw_probability > 0`, and (b) the equal-shrinkage draw posterior is a
//! reference-only unit check (documented for a future draw-capable variable).

use std::collections::{HashMap, VecDeque};

use matchlab_core::match_::{MatchId, MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation, SkillVector, VisibleRank};
use matchlab_core::time::SimTime;
use matchlab_rating::RatingSystem;
use matchlab_rating::lua::LuaRatingSystem;
use matchlab_validation::reference::trueskill;

fn obs(id: u64, rating: f64, rd: f64, games: u64) -> PlayerObservation {
    PlayerObservation {
        id: PlayerId(id),
        rating,
        hidden_mmr: rating,
        visible_rank: VisibleRank {
            tier: "unranked".into(),
            division: 1,
        },
        rating_deviation: rd,
        volatility: 0.0,
        games_played: games,
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
        detection_flags: Vec::new(),
        role: None,
    }
}

fn match_result(a: Vec<PlayerId>, b: Vec<PlayerId>, winner: Team) -> MatchResult {
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

fn ts(draw_probability: f64) -> LuaRatingSystem {
    LuaRatingSystem::load(
        "plugins/rating/trueskill.lua",
        &serde_yaml::from_str(&format!(
            "initial_mean: 1500.0\ninitial_variance: 350.0\nbeta: 400.0\ndynamics: 0.0\ndraw_probability: {draw_probability}"
        ))
        .unwrap(),
    )
    .unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

fn assert_close(a: f64, b: f64, what: &str) {
    assert!(
        rel(a, b) < 1e-6 || (a - b).abs() < 1e-9,
        "{what}: script {a} vs reference {b} (rel {})",
        rel(a, b)
    );
}

struct SimResult {
    a_mu: f64,
    a_sigma: f64,
    b_mu: f64,
    b_sigma: f64,
}

/// Drive the script through one 1v1 game with the given observations.
fn sim(
    sys: &LuaRatingSystem,
    a: &PlayerObservation,
    b: &PlayerObservation,
    winner: Team,
) -> SimResult {
    let mut map = HashMap::new();
    map.insert(a.id, a.clone());
    map.insert(b.id, b.clone());
    let mr = match_result(vec![a.id], vec![b.id], winner);
    let updates = sys.update(&mr, &map);
    SimResult {
        a_mu: updates[&a.id].rating,
        a_sigma: updates[&a.id].rating_deviation,
        b_mu: updates[&b.id].rating,
        b_sigma: updates[&b.id].rating_deviation,
    }
}

#[test]
fn trueskill_1v1_win_matches_reference() {
    let sys = ts(0.0);
    let beta = 400.0;

    // Symmetric case, no draw margin.
    let a = obs(1, 1500.0, 100.0, 0);
    let b = obs(2, 1500.0, 100.0, 0);
    for winner in [Team::A, Team::B] {
        let s = sim(&sys, &a, &b, winner);
        let (ra, rb) = trueskill::update_head_to_head(
            a.rating,
            a.rating_deviation,
            b.rating,
            b.rating_deviation,
            beta,
            0.0,
            winner == Team::A,
        );
        let (ap, asr) = ra;
        let (bp, bsr) = rb;
        assert_close(s.a_mu, ap, "a mu");
        assert_close(s.a_sigma, asr, "a sigma");
        assert_close(s.b_mu, bp, "b mu");
        assert_close(s.b_sigma, bsr, "b sigma");
    }

    // Asymmetric case (1500 vs 1000): the big skill gap must survive the
    // update (posterior ordering preserves pre-game ordering).
    let hi = obs(3, 1500.0, 100.0, 0);
    let lo = obs(4, 1000.0, 100.0, 0);
    let s = sim(&sys, &hi, &lo, Team::A);
    assert!(
        s.a_mu > s.b_mu,
        "winner must rank above loser: {} vs {}",
        s.a_mu,
        s.b_mu
    );
    assert!(
        s.a_mu > 1500.0 && s.b_mu < 1000.0,
        "gap widened: {} vs {}",
        s.a_mu,
        s.b_mu
    );
    let (ra, rb) = trueskill::update_head_to_head(
        hi.rating,
        hi.rating_deviation,
        lo.rating,
        lo.rating_deviation,
        beta,
        0.0,
        true,
    );
    assert_close(s.a_mu, ra.0, "hi mu");
    assert_close(s.a_sigma, ra.1, "hi sigma");
    assert_close(s.b_mu, rb.0, "lo mu");
    assert_close(s.b_sigma, rb.1, "lo sigma");
}

#[test]
fn trueskill_draw_probability_engages_margin_math() {
    // With draw_probability > 0 the edge case u = Φ⁻¹((1+p)/2) enters the
    // win/loss factors; the script's full update must agree with the reference
    // that applies the same u. The margin shrinks the update vs draw_probability 0.
    let sys_draw = ts(0.5);
    let sys_nodraw = ts(0.0);
    let a = obs(1, 1500.0, 100.0, 0);
    let b = obs(2, 1500.0, 100.0, 0);

    let sd = sim(&sys_draw, &a, &b, Team::A);
    let sn = sim(&sys_nodraw, &a, &b, Team::A);
    assert!(
        (sd.a_mu - sn.a_mu).abs() > 1e-6,
        "draw probability must engage the margin math: {} vs {}",
        sd.a_mu,
        sn.a_mu
    );

    let (ra, rb) = trueskill::update_head_to_head(1500.0, 100.0, 1500.0, 100.0, 400.0, 0.5, true);
    assert_close(sd.a_mu, ra.0, "a mu (draw margin)");
    assert_close(sd.a_sigma, ra.1, "a sigma (draw margin)");
    assert_close(sd.b_mu, rb.0, "b mu (draw margin)");
    assert_close(sd.b_sigma, rb.1, "b sigma (draw margin)");
}

#[test]
fn trueskill_draw_posterior_shrinks_identically() {
    // Reference-only: with equal ratings and t = 0 a real draw keeps μ and
    // shrinks both σ identically. trueskill.lua has no draw outcome path —
    // this documents (for a future draw-capable game variable) the numbers to
    // hit: beta 100 / sigma 100 / draw 0.5 give a ~11% shrinkage.
    let (mu, sigma) = trueskill::draw_update_equal_players(1500.0, 100.0, 100.0, 0.5);
    assert!((mu - 1500.0).abs() < 1e-9, "mu must be unchanged: {mu}");
    assert!(sigma < 100.0, "sigma must shrink: {sigma}");
    assert!((sigma - 100.0).abs() > 5.0, "shrinkage too small: {sigma}");
}
