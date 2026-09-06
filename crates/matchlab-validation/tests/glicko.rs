//! Glicko-2 script-vs-reference baselines (ticket T-02).
//!
//! Every assertion compares `plugins/rating/glicko2.lua` update output against
//! the independent Rust reference in `matchlab_validation::reference::glicko2`
//! with a 1e-6 *relative* tolerance (for σ ≈ 0.06 that is ~6e-8 absolute; for
//! ratings ≈1464 it is ~1.5e-3). A disagreement means the Lua script drifted —
//! the reference is never patched to match.

use std::collections::{HashMap, VecDeque};

use matchlab_core::match_::{MatchId, MatchResult, Team};
use matchlab_core::player::{PlayerId, PlayerObservation, SkillVector, VisibleRank};
use matchlab_core::time::SimTime;
use matchlab_rating::RatingSystem;
use matchlab_rating::lua::LuaRatingSystem;
use matchlab_validation::reference::glicko2;

const TAU: f64 = 0.5;
const EPS: f64 = 1e-6;

fn obs(id: u64, rating: f64, rd: f64, volatility: f64, games: u64) -> PlayerObservation {
    PlayerObservation {
        id: PlayerId(id),
        rating,
        hidden_mmr: rating,
        visible_rank: VisibleRank {
            tier: "unranked".into(),
            division: 1,
        },
        rating_deviation: rd,
        volatility,
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

fn glicko() -> LuaRatingSystem {
    LuaRatingSystem::load(
        "plugins/rating/glicko2.lua",
        &serde_yaml::from_str(
            "initial_rating: 1500.0\ninitial_rd: 200.0\ninitial_volatility: 0.06\ntau: 0.5\nepsilon: 0.000001",
        )
        .unwrap(),
    )
    .unwrap()
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

/// One rating period through the reference, mirroring the script's shape (the
/// solo player's opponents are the other team, all with the match outcome).
fn reference_period(
    rating: f64,
    rd: f64,
    sigma: f64,
    opponents: &[(f64, f64, f64)],
    tau: f64,
    eps: f64,
) -> (f64, f64, f64) {
    let (mu, phi) = glicko2::scale(rating, rd);
    let opp: Vec<glicko2::Opponent> = opponents
        .iter()
        .map(|&(r, o_rd, outcome)| {
            let (mu_j, phi_j) = glicko2::scale(r, o_rd);
            glicko2::Opponent {
                mu: mu_j,
                phi: phi_j,
                outcome,
            }
        })
        .collect();
    let out = glicko2::single_period(mu, phi, sigma, &opp, tau, eps);
    let (r, rd_out) = glicko2::unscale(out.mu, out.phi);
    (r, rd_out, out.sigma)
}

/// Drive the script through one match and return the solo player's new state.
fn script_period(
    sys: &LuaRatingSystem,
    player: &PlayerObservation,
    opponents: &[(PlayerId, f64, f64)],
    winner: Team,
) -> (f64, f64, f64) {
    let mut map = HashMap::new();
    map.insert(player.id, player.clone());
    let mut ids = Vec::new();
    for &(oid, or, ord) in opponents {
        ids.push(oid);
        map.insert(oid, obs(oid.0, or, ord, 0.06, 0));
    }
    let mr = match_result(vec![player.id], ids, winner);
    let updates = sys.update(&mr, &map);
    let s = &updates[&player.id];
    (s.rating, s.rating_deviation, s.volatility)
}

#[test]
fn glicko_paper_example_matches_reference() {
    let sys = glicko();
    let (mut rating, mut rd, mut vol) = (1500.0, 200.0, 0.06);
    // The paper's mixed-outcome rating period, driven as three sequential 1v1
    // games (the script only supports per-team uniform outcomes), stepped
    // through an identical sequence of single-opponent reference periods.
    for (i, &(orating, ord, winner)) in [
        (1400.0, 30.0, Team::A),
        (1550.0, 100.0, Team::B),
        (1700.0, 300.0, Team::B),
    ]
    .iter()
    .enumerate()
    {
        let oid = PlayerId(2 + i as u64);
        let outcome = if winner == Team::A { 1.0 } else { 0.0 };
        let player = obs(1, rating, rd, vol, i as u64);
        let (sr, srd, sv) = script_period(&sys, &player, &[(oid, orating, ord)], winner);
        let (rr, rrd, rv) = reference_period(rating, rd, vol, &[(orating, ord, outcome)], TAU, EPS);
        assert_close(sr, rr, "rating");
        assert_close(srd, rrd, "rd");
        assert_close(sv, rv, "volatility");
        rating = sr;
        rd = srd;
        vol = sv;
    }
    // The final values reproduce Glickman's worked example. The single-period
    // paper math (1464.06) and three serialized single-game periods differ
    // slightly (~0.27) — each period's volatility iteration slightly deflates
    // RD differently — so the paper's numbers must come within the legacy
    // loose bound while the per-step script-vs-reference checks above are the
    // tight (1e-6) assertions.
    assert!((rating - 1464.06).abs() < 1.5, "r {rating}");
    assert!((rd - 151.52).abs() < 0.5, "rd {rd}");
    assert!((vol - 0.05999).abs() < 1e-4, "vol {vol}");
}

#[test]
fn two_period_chain_with_idle_volatility_growth() {
    let sys = glicko();
    let (mut rating, mut rd, mut vol) = (1500.0, 200.0, 0.06);
    let mut games = 0u64;

    // Period 1: win vs 1400/30.
    let p1 = obs(1, rating, rd, vol, games);
    let (sr, srd, sv) = script_period(&sys, &p1, &[(PlayerId(2), 1400.0, 30.0)], Team::A);
    let (rr, rrd, rv) = reference_period(rating, rd, vol, &[(1400.0, 30.0, 1.0)], TAU, EPS);
    assert_close(sr, rr, "p1 rating");
    assert_close(srd, rrd, "p1 rd");
    assert_close(sv, rv, "p1 vol");
    rating = sr;
    rd = srd;
    vol = sv;
    games += 1;

    // Idle interval before period 2: volatility leaks into RD
    // (φ' = sqrt(φ² + σ²)), i.e. the grown RD is the pre-period-2 input.
    let (_, phi) = glicko2::scale(0.0, rd);
    let grown_phi = glicko2::idle_step(phi, vol);
    let rd_grown = grown_phi * glicko2::SCALE;
    assert!(rd_grown > rd, "idle growth expected: {rd_grown} > {rd}");

    // Period 2: loss vs 1550/100, starting from the idle-grown RD.
    let p2 = obs(1, rating, rd_grown, vol, games);
    let (sr2, srd2, sv2) = script_period(&sys, &p2, &[(PlayerId(3), 1550.0, 100.0)], Team::B);
    let (rr2, rrd2, rv2) =
        reference_period(rating, rd_grown, vol, &[(1550.0, 100.0, 0.0)], TAU, EPS);
    assert_close(sr2, rr2, "p2 rating");
    assert_close(srd2, rrd2, "p2 rd");
    assert_close(sv2, rv2, "p2 vol");
}

#[test]
fn eight_opponent_period_is_stable() {
    let sys = glicko();
    let opponents: Vec<(PlayerId, f64, f64)> = (0..8)
        .map(|i| {
            (
                PlayerId(10 + i),
                1500.0 + 20.0 * i as f64,
                150.0 + 10.0 * i as f64,
            )
        })
        .collect();

    // Solo player loses the 1v8 match: all 8 opponents count as losses.
    let player = obs(1, 1500.0, 200.0, 0.06, 0);
    let (sr, srd, sv) = script_period(&sys, &player, &opponents, Team::B);

    let ref_opp: Vec<(f64, f64, f64)> = opponents.iter().map(|(_, r, rd)| (*r, *rd, 0.0)).collect();
    let (rr, rrd, rv) = reference_period(1500.0, 200.0, 0.06, &ref_opp, TAU, EPS);
    assert_close(sr, rr, "rating");
    assert_close(srd, rrd, "rd");
    assert_close(sv, rv, "volatility");

    // A single opponent's own update (a 1-opponent period against the solo
    // player) must also match its reference.
    let (orr, orrd, orv) = reference_period(1500.0, 150.0, 0.06, &[(1500.0, 200.0, 1.0)], TAU, EPS);
    let mut map = HashMap::new();
    map.insert(PlayerId(10), obs(10, 1500.0, 150.0, 0.06, 0));
    map.insert(PlayerId(1), player.clone());
    let updates = sys.update(
        &match_result(vec![PlayerId(10)], vec![PlayerId(1)], Team::A),
        &map,
    );
    let u = &updates[&PlayerId(10)];
    assert_close(u.rating, orr, "opp rating");
    assert_close(u.rating_deviation, orrd, "opp rd");
    assert_close(u.volatility, orv, "opp vol");
}

#[test]
fn negative_control_crude_epsilon_is_detected() {
    // Teeth check: the *correct* reference matches the script, but a
    // perturbed reference (volatility iteration with epsilon = 1.0, i.e. it
    // stops after ~zero Newton steps and returns a stale σ') must diverge.
    let sys = glicko();
    let player = obs(1, 1500.0, 200.0, 0.06, 0);
    let (sr, srd, _) = script_period(&sys, &player, &[(PlayerId(2), 1400.0, 30.0)], Team::A);

    let (rr, rrd, _) = reference_period(1500.0, 200.0, 0.06, &[(1400.0, 30.0, 1.0)], TAU, EPS);
    assert_close(sr, rr, "rating");
    assert_close(srd, rrd, "rd");

    let (wrong_r, _, _) = reference_period(1500.0, 200.0, 0.06, &[(1400.0, 30.0, 1.0)], TAU, 1.0);
    assert!(
        rel(wrong_r, sr).max(rel(wrong_r, rr)) > 1e-5,
        "perturbed reference unexpectedly agrees; test has no teeth"
    );
}
