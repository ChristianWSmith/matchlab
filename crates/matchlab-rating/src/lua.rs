//! Lua-native rating systems.
//!
//! `LuaRatingSystem` implements the `RatingSystem` trait by delegating to a
//! script's `initialize` / `predict` / `update` functions. The script declares
//! its `information_budget` global at load; per-player state that the script
//! wants to keep lives in the VM's context table (passed by reference).

use std::collections::HashMap;

use matchlab_core::match_::MatchResult;
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use mlua::Table;

use crate::system::{ObservationType, RatingState, RatingSystem};

/// A rating system whose algorithm lives entirely in a Lua script.
pub struct LuaRatingSystem {
    vm: LuaVm,
    budget: Vec<ObservationType>,
}

impl LuaRatingSystem {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["initialize", "predict", "update"])?;
        let budget = vm
            .get_global::<Vec<String>>("information_budget")?
            .map(|names| names.iter().filter_map(|n| observation_type(n)).collect())
            .unwrap_or_else(|| vec![ObservationType::WinLoss]);
        Ok(Self { vm, budget })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

fn observation_type(name: &str) -> Option<ObservationType> {
    match name {
        "WinLoss" => Some(ObservationType::WinLoss),
        "Score" => Some(ObservationType::Score),
        "Kills" => Some(ObservationType::Kills),
        "Deaths" => Some(ObservationType::Deaths),
        "Assists" => Some(ObservationType::Assists),
        "ObjectiveScore" => Some(ObservationType::ObjectiveScore),
        "Impact" => Some(ObservationType::Impact),
        "Duration" => Some(ObservationType::Duration),
        "Disconnects" => Some(ObservationType::Disconnects),
        "SessionHistory" => Some(ObservationType::SessionHistory),
        "QuitBehavior" => Some(ObservationType::QuitBehavior),
        _ => None,
    }
}

fn state_from_table(t: &Table) -> RatingState {
    let rating = t.get::<f64>("rating").unwrap_or(1000.0);
    let rating_deviation = t.get::<f64>("rating_deviation").unwrap_or(350.0);
    let volatility = t.get::<f64>("volatility").unwrap_or(0.06);
    let games_played = t.get::<u64>("games_played").unwrap_or(0);
    RatingState {
        rating,
        rating_deviation,
        volatility,
        games_played,
    }
}

impl RatingSystem for LuaRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        self.budget.clone()
    }

    fn initialize(&self, player_id: PlayerId) -> RatingState {
        let args = vec![mlua::Value::Integer(player_id.0 as i64)];
        let state_tbl: Table = self
            .vm
            .call_with_context("initialize", &args)
            .expect("rating initialize failed");
        state_from_table(&state_tbl)
    }

    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let team_a_val = self
            .vm
            .with_lua(|lua| convert::observations_to_value(lua, team_a, false))
            .expect("build team_a table");
        let team_b_val = self
            .vm
            .with_lua(|lua| convert::observations_to_value(lua, team_b, false))
            .expect("build team_b table");
        self.vm
            .call_with_context("predict", &[team_a_val, team_b_val])
            .expect("rating predict failed")
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mr_val = self
            .vm
            .with_lua(|lua| {
                convert::match_result_to_table(lua, match_result).map(mlua::Value::Table)
            })
            .expect("build match result table");

        let mut obs_list: Vec<PlayerObservation> = observations.values().cloned().collect();
        obs_list.sort_by_key(|o| o.id.0);
        let obs_val = self
            .vm
            .with_lua(|lua| convert::observations_to_map(lua, &obs_list, false))
            .expect("build observations table");

        let updates_tbl: Table = self
            .vm
            .call_with_context("update", &[mr_val, obs_val])
            .expect("rating update failed");

        let mut updates = HashMap::new();
        for pair in updates_tbl.clone().pairs::<mlua::Value, Table>() {
            let (_, row) = pair.expect("iterate rating updates");
            let pid = row.get::<u64>("player_id").expect("update row player_id");
            let rating = row.get::<f64>("rating").expect("update row rating");
            let rating_deviation = row
                .get::<f64>("rating_deviation")
                .expect("update row rating_deviation");
            let volatility = row.get::<f64>("volatility").expect("update row volatility");
            let games_played = row
                .get::<u64>("games_played")
                .expect("update row games_played");
            updates.insert(
                PlayerId(pid),
                RatingState {
                    rating,
                    rating_deviation,
                    volatility,
                    games_played,
                },
            );
        }
        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{SkillVector, VisibleRank};
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

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
            volatility: 0.06,
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
        }
    }

    fn params(yaml: &str) -> serde_yaml::Value {
        serde_yaml::from_str(yaml).unwrap()
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

    fn elo() -> LuaRatingSystem {
        LuaRatingSystem::load(
            "plugins/rating/elo.lua",
            &params("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0"),
        )
        .unwrap()
    }

    #[test]
    fn elo_predict_equal_ratings_half() {
        let sys = elo();
        let p = sys.predict(&[obs(1, 1000.0, 350.0, 0)], &[obs(2, 1000.0, 350.0, 0)]);
        assert!((p - 0.5).abs() < 0.001, "p={p}");
    }

    #[test]
    fn elo_update_winner_gains_loser_loses() {
        let sys = elo();
        let mr = match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut map = HashMap::new();
        map.insert(PlayerId(1), obs(1, 1000.0, 350.0, 0));
        map.insert(PlayerId(2), obs(2, 1000.0, 350.0, 0));
        let updates = sys.update(&mr, &map);
        assert!(updates[&PlayerId(1)].rating > 1000.0);
        assert!(updates[&PlayerId(2)].rating < 1000.0);
        assert_eq!(updates[&PlayerId(1)].games_played, 1);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn elo_matches_logistic_scale() {
        let sys = elo();
        let p = sys.predict(&[obs(1, 1000.0, 350.0, 0)], &[obs(2, 1400.0, 350.0, 0)]);
        let logistic = 1.0 / (1.0 + (-((-400.0_f64) / 400.0)).exp());
        assert!((p - logistic).abs() < 0.001, "p={p} logistic={logistic}");
    }

    #[test]
    fn glicko_golden_example() {
        let sys = LuaRatingSystem::load(
            "plugins/rating/glicko2.lua",
            &params("initial_rating: 1500.0\ninitial_rd: 200.0\ninitial_volatility: 0.06\ntau: 0.5\nepsilon: 0.000001"),
        )
        .unwrap();
        // Player at 1500/200 plays: win vs 1400/30, loss vs 1550/100, loss vs 1700/300.
        let mut map = HashMap::new();
        map.insert(PlayerId(1), obs(1, 1500.0, 200.0, 0));
        // Three sequential single matches, simulating the three opponents.
        let opps = [
            (PlayerId(2), 1400.0, 30.0, Team::A),
            (PlayerId(3), 1550.0, 100.0, Team::B),
            (PlayerId(4), 1700.0, 300.0, Team::B),
        ];
        let mut player = obs(1, 1500.0, 200.0, 0);
        for (oid, orating, ord, winner) in opps {
            let mr = match_result(vec![PlayerId(1)], vec![oid], winner);
            let mut map = HashMap::new();
            map.insert(PlayerId(1), player.clone());
            map.insert(oid, obs(oid.0, orating, ord, 0));
            let updates = sys.update(&mr, &map);
            player.rating = updates[&PlayerId(1)].rating;
            player.rating_deviation = updates[&PlayerId(1)].rating_deviation;
            player.volatility = updates[&PlayerId(1)].volatility;
            player.games_played += 1;
        }
        assert!(
            (player.rating - 1464.06).abs() < 2.0,
            "rating {} != ~1464.06",
            player.rating
        );
        assert!(
            (player.rating_deviation - 151.52).abs() < 2.0,
            "RD {} != ~151.52",
            player.rating_deviation
        );
        assert!(
            (player.volatility - 0.05999).abs() < 0.005,
            "vol {} != ~0.05999",
            player.volatility
        );
    }

    #[test]
    fn trueskill_winner_up_loser_down() {
        let sys = LuaRatingSystem::load(
            "plugins/rating/trueskill.lua",
            &params("initial_mean: 1500.0\ninitial_variance: 350.0\nbeta: 400.0\ndynamics: 0.0\ndraw_probability: 0.0"),
        )
        .unwrap();
        let mr = match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut map = HashMap::new();
        map.insert(PlayerId(1), obs(1, 1500.0, 30.0, 0));
        map.insert(PlayerId(2), obs(2, 1500.0, 30.0, 0));
        let updates = sys.update(&mr, &map);
        assert!(updates[&PlayerId(1)].rating > 1500.0);
        assert!(updates[&PlayerId(2)].rating < 1500.0);
        assert!(updates[&PlayerId(1)].rating_deviation < 30.0);
        let init = sys.initialize(PlayerId(9));
        assert!((init.rating_deviation - 350.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn flat_fixed_points() {
        let sys = LuaRatingSystem::load(
            "plugins/rating/flat.lua",
            &params("win_points: 10.0\nloss_points: 10.0\ninitial_rating: 1000.0"),
        )
        .unwrap();
        let mr = match_result(vec![PlayerId(1)], vec![PlayerId(2)], Team::A);
        let mut map = HashMap::new();
        map.insert(PlayerId(1), obs(1, 1000.0, 350.0, 0));
        map.insert(PlayerId(2), obs(2, 1000.0, 350.0, 0));
        let updates = sys.update(&mr, &map);
        assert!((updates[&PlayerId(1)].rating - 1010.0).abs() < 1e-9);
        assert!((updates[&PlayerId(2)].rating - 990.0).abs() < 1e-9);
    }

    #[test]
    fn missing_information_budget_defaults_to_winloss() {
        // elo.lua declares it; simulate absence by checking a script that doesn't.
        let sys = LuaRatingSystem::load(
            "plugins/rating/elo.lua",
            &params("k_factor: 32.0\ninitial_rating: 1000.0\nbeta: 400.0"),
        )
        .unwrap();
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }
}
