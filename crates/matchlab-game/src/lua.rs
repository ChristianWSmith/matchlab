//! Lua-native outcome models.
//!
//! `LuaOutcomeModel` implements the `OutcomeModel` trait by delegating to a
//! script's `win_probability` / `simulate` functions. Randomness in `simulate`
//! flows through `matchlab.rng_*` (from the caller's `&mut SimRng`). The
//! observation tables carry the ground-truth skill binding (`include_skill`),
//! so match winners are decided by true skill.

use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use mlua::Table;

use crate::outcome::OutcomeModel;

/// An outcome model whose algorithm lives entirely in a Lua script.
pub struct LuaOutcomeModel {
    vm: LuaVm,
}

impl LuaOutcomeModel {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["win_probability", "simulate"])?;
        Ok(Self { vm })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

fn parse_result(t: &Table) -> MatchResult {
    let winner = match t.get::<String>("winner").as_deref().unwrap_or("A") {
        "B" => Team::B,
        _ => Team::A,
    };
    let team_a: Vec<PlayerId> = t
        .get::<Table>("team_a")
        .map(|ids| {
            ids.pairs::<mlua::Value, u64>()
                .map(|p| p.unwrap().1)
                .map(PlayerId)
                .collect()
        })
        .unwrap_or_default();
    let team_b: Vec<PlayerId> = t
        .get::<Table>("team_b")
        .map(|ids| {
            ids.pairs::<mlua::Value, u64>()
                .map(|p| p.unwrap().1)
                .map(PlayerId)
                .collect()
        })
        .unwrap_or_default();
    let team_a_score = t.get::<f64>("team_a_score").unwrap_or(13.0);
    let team_b_score = t.get::<f64>("team_b_score").unwrap_or(5.0);
    let duration_secs = t.get::<f64>("duration_secs").unwrap_or(1800.0);
    let variance = t.get::<f64>("variance").unwrap_or(0.0);
    let disconnected = t.get::<bool>("disconnected").unwrap_or(false);
    let forfeited = t.get::<bool>("forfeited").unwrap_or(false);

    let mut performances = Vec::new();
    if let Ok(perfs) = t.get::<Table>("performances") {
        for (_, row) in perfs.pairs::<mlua::Value, Table>().map_while(Result::ok) {
            performances.push(PlayerPerformance {
                player_id: PlayerId(row.get::<u64>("player_id").unwrap_or(0)),
                kills: row.get::<u32>("kills").unwrap_or(0),
                deaths: row.get::<u32>("deaths").unwrap_or(0),
                assists: row.get::<u32>("assists").unwrap_or(0),
                objective_score: row.get::<f64>("objective_score").unwrap_or(0.0),
                impact: row.get::<f64>("impact").unwrap_or(0.0),
                variance: row.get::<f64>("variance").unwrap_or(0.0),
            });
        }
    }

    MatchResult {
        match_id: MatchId(0),
        winner,
        team_a,
        team_b,
        team_a_score,
        team_b_score,
        player_performances: performances,
        duration: SimTime::from_secs(duration_secs),
        disconnected,
        forfeited,
        variance,
        unexpected_events: Vec::new(),
    }
}

impl OutcomeModel for LuaOutcomeModel {
    fn win_probability(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let a_val = self
            .vm
            .with_lua(|lua| convert::observations_to_value(lua, team_a, true))
            .expect("build team_a table");
        let b_val = self
            .vm
            .with_lua(|lua| convert::observations_to_value(lua, team_b, true))
            .expect("build team_b table");
        self.vm
            .call_with_context("win_probability", &[a_val, b_val])
            .expect("outcome win_probability failed")
    }

    fn simulate(
        &self,
        match_id: MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult {
        let a_val = self
            .vm
            .with_lua(|lua| convert::observations_to_value(lua, team_a, true))
            .expect("build team_a table");
        let b_val = self
            .vm
            .with_lua(|lua| convert::observations_to_value(lua, team_b, true))
            .expect("build team_b table");

        let result_tbl: Table = self.vm.with_rng(rng, |vm| {
            vm.call_with_context(
                "simulate",
                &[mlua::Value::Integer(match_id.0 as i64), a_val, b_val],
            )
            .expect("outcome simulate failed")
        });

        let mut result = parse_result(&result_tbl);
        result.match_id = match_id;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{SkillVector, VisibleRank};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
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

    fn logistic() -> LuaOutcomeModel {
        LuaOutcomeModel::load(
            "plugins/game/logistic.lua",
            &serde_yaml::from_str("beta: 400.0\nnoise: 0.05").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn equal_teams_have_win_probability_half() {
        let model = logistic();
        let p = model.win_probability(&[obs(1, 1000.0), obs(2, 1000.0)], &[obs(3, 1000.0)]);
        assert!((p - 0.5).abs() < 1e-9, "p={p}");
    }

    #[test]
    fn outcome_follows_skill_vector_ground_truth() {
        let model = logistic();
        // Visible rating 900 but true skill 1700 vs rating 1600 / true 950.
        let mut high = obs(1, 900.0);
        high.skill_vector = SkillVector::one_dimensional(1700.0);
        let mut low = obs(2, 1600.0);
        low.skill_vector = SkillVector::one_dimensional(950.0);
        let p = model.win_probability(&[high], &[low]);
        assert!(p > 0.8, "higher-skill team should be favored: {p}");
    }

    #[test]
    fn simulate_is_deterministic_given_seed() {
        let model = logistic();
        let team_a = vec![obs(1, 1000.0), obs(2, 1000.0)];
        let team_b = vec![obs(3, 1000.0), obs(4, 1000.0)];
        let mut rng_a = SimRng::from_seed(42);
        let mut rng_b = SimRng::from_seed(42);
        let a = model.simulate(MatchId(1), &team_a, &team_b, &mut rng_a);
        let b = model.simulate(MatchId(1), &team_a, &team_b, &mut rng_b);
        assert_eq!(a.winner, b.winner);
        assert_eq!(a.team_a_score, b.team_a_score);
        assert_eq!(a.duration, b.duration);
        for (pa, pb) in a
            .player_performances
            .iter()
            .zip(b.player_performances.iter())
        {
            assert_eq!(pa.kills, pb.kills);
            assert_eq!(pa.impact, pb.impact);
        }
    }

    #[test]
    fn simulate_builds_full_result() {
        let model = logistic();
        let team_a = vec![obs(1, 1000.0), obs(2, 1000.0)];
        let team_b = vec![obs(3, 1000.0), obs(4, 1000.0)];
        let mut rng = SimRng::from_seed(7);
        let result = model.simulate(MatchId(5), &team_a, &team_b, &mut rng);
        assert_eq!(result.match_id, MatchId(5));
        assert_eq!(result.team_a.len(), 2);
        assert_eq!(result.team_b.len(), 2);
        assert_eq!(result.player_performances.len(), 4);
        assert!(!result.duration.as_secs_f64().is_nan());
        assert!(result.variance >= 0.0);
    }

    #[test]
    fn fatigue_tilts_probability_down_with_games() {
        let model = LuaOutcomeModel::load(
            "plugins/game/fatigue.lua",
            &serde_yaml::from_str("beta: 400.0\nnoise: 0.05\nfatigue_decay_rate: 0.001").unwrap(),
        )
        .unwrap();
        let mut fresh = obs(1, 1500.0);
        fresh.games_played = 0;
        let mut tired = obs(2, 1500.0);
        tired.games_played = 500;
        let p = model.win_probability(&[tired.clone()], &[fresh.clone()]);
        assert!(p < 0.5, "fatigued team should be less favored: {p}");
    }

    #[test]
    fn momentum_tilts_probability_with_win_rate() {
        let model = LuaOutcomeModel::load(
            "plugins/game/momentum.lua",
            &serde_yaml::from_str("beta: 400.0\nnoise: 0.05\nmomentum_factor: 0.1").unwrap(),
        )
        .unwrap();
        let mut hot = obs(1, 1500.0);
        hot.win_rate = 0.9;
        let mut cold = obs(2, 1500.0);
        cold.win_rate = 0.1;
        let p = model.win_probability(&[hot], &[cold]);
        assert!(p > 0.5, "hot team should be favored: {p}");
    }
}
