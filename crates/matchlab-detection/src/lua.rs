//! Lua-native detection systems.
//!
//! `LuaDetectionSystem` implements the `DetectionSystem` trait by delegating
//! to a script's `observe` / `evaluate` / `recommend_action` functions.
//! Per-player evidence lives in the threaded `Context`, so detection state
//! persists across matches. Scripts return an action string that the adapter
//! maps to `InterventionAction`.

use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use mlua::{Table, Value};

use crate::detector::{DetectionResult, DetectionSystem};
use crate::intervention::InterventionAction;

/// A detection system whose algorithm lives entirely in a Lua script.
pub struct LuaDetectionSystem {
    vm: LuaVm,
}

impl LuaDetectionSystem {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["observe", "evaluate", "recommend_action"])?;
        Ok(Self { vm })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

fn action_from_str(s: &str) -> Option<InterventionAction> {
    match s {
        "None" => Some(InterventionAction::None),
        "AccelerateRating" => Some(InterventionAction::AccelerateRating { multiplier: 1.5 }),
        "IncreaseKFactor" => Some(InterventionAction::IncreaseKFactor { new_k: 32.0 }),
        "FlagForReview" => Some(InterventionAction::FlagForReview),
        "RestrictQueue" => Some(InterventionAction::RestrictQueue {
            duration_ticks: 100,
        }),
        "TempBan" => Some(InterventionAction::TempBan {
            duration_ticks: 500,
        }),
        "Probation" => Some(InterventionAction::Probation {
            duration_ticks: 1000,
        }),
        "Ban" => Some(InterventionAction::Ban),
        _ => None,
    }
}

fn result_from_table(t: &Table, player_id: PlayerId) -> DetectionResult {
    DetectionResult {
        player_id,
        probability_of_anomaly: t.get::<f64>("probability_of_anomaly").unwrap_or(0.0),
        confidence: t.get::<f64>("confidence").unwrap_or(0.0),
        evidence: t
            .get::<Vec<String>>("evidence")
            .unwrap_or_else(|_| Vec::new()),
    }
}

fn participant_observations(
    world: &World,
    match_result: &MatchResult,
) -> Vec<matchlab_core::player::PlayerObservation> {
    let mut out = Vec::new();
    for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
        if let Some(o) = world.observations.get(pid) {
            out.push(o.clone());
        }
    }
    out.sort_by_key(|o| o.id.0);
    out
}

impl DetectionSystem for LuaDetectionSystem {
    fn observe(&mut self, match_result: &MatchResult, world: &World) {
        let mr_val = self
            .vm
            .with_lua(|lua| {
                convert::match_result_to_table(lua, match_result).map(mlua::Value::Table)
            })
            .expect("build match result table");
        let obs = participant_observations(world, match_result);
        let obs_val = self
            .vm
            .with_lua(|lua| convert::observations_to_map(lua, &obs, false))
            .expect("build observations table");
        let _: Value = self
            .vm
            .call_with_context("observe", &[mr_val, obs_val])
            .expect("detection observe failed");
    }

    fn evaluate(&self, player_id: PlayerId, world: &World) -> DetectionResult {
        let obs_val = self
            .vm
            .with_lua(|lua| {
                let list: Vec<matchlab_core::player::PlayerObservation> = world
                    .observations
                    .get(&player_id)
                    .cloned()
                    .into_iter()
                    .collect();
                convert::observations_to_map(lua, &list, false)
            })
            .expect("build observation table");
        let result_tbl: Table = self
            .vm
            .call_with_context("evaluate", &[Value::Integer(player_id.0 as i64), obs_val])
            .expect("detection evaluate failed");
        result_from_table(&result_tbl, player_id)
    }

    fn recommend_action(&self, result: &DetectionResult) -> InterventionAction {
        let result_tbl = self
            .vm
            .with_lua(|lua| {
                let t = lua.create_table().map_err(|e| e.to_string())?;
                t.set("player_id", result.player_id.0)
                    .map_err(|e| e.to_string())?;
                t.set("probability_of_anomaly", result.probability_of_anomaly)
                    .map_err(|e| e.to_string())?;
                t.set("confidence", result.confidence)
                    .map_err(|e| e.to_string())?;
                let evidence = lua.create_table().map_err(|e| e.to_string())?;
                for (i, s) in result.evidence.iter().enumerate() {
                    evidence.set(i + 1, s.as_str()).map_err(|e| e.to_string())?;
                }
                t.set("evidence", evidence).map_err(|e| e.to_string())?;
                Ok(Value::Table(t))
            })
            .expect("build detection result table");
        let action: String = self
            .vm
            .call_with_context("recommend_action", &[result_tbl])
            .expect("detection recommend_action failed");
        action_from_str(&action).unwrap_or(InterventionAction::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, PlayerPerformance, Team};
    use matchlab_core::player::{PlayerObservation, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
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

    fn mr(match_id: u64, perf: PlayerPerformance) -> MatchResult {
        MatchResult {
            match_id: MatchId(match_id),
            winner: Team::A,
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: vec![perf],
            duration: SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    fn detector() -> LuaDetectionSystem {
        LuaDetectionSystem::load(
            "plugins/detection/smurf.lua",
            &serde_yaml::from_str("min_anomalous_games: 3\nmin_games_before_action: 2").unwrap(),
        )
        .unwrap()
    }

    fn sensitive_detector() -> LuaDetectionSystem {
        LuaDetectionSystem::load(
            "plugins/detection/smurf.lua",
            &serde_yaml::from_str(
                "sigma_threshold: 1.0\nmin_anomalous_games: 3\nmin_games_before_action: 2",
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn clean_player_stays_low_probability() {
        let mut world = World::new(SimRng::from_seed(42));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        let mut d = detector();
        for i in 0..5u64 {
            let perf = PlayerPerformance {
                player_id: PlayerId(1),
                kills: 5,
                deaths: 5,
                assists: 3,
                objective_score: 50.0,
                impact: 0.0,
                variance: 0.5,
            };
            d.observe(&mr(i, perf), &world);
        }
        let result = d.evaluate(PlayerId(1), &world);
        assert!(result.probability_of_anomaly < 0.3);
    }

    #[test]
    fn anomalous_streak_increases_probability() {
        let mut world = World::new(SimRng::from_seed(42));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        let mut d = sensitive_detector();
        for i in 0..5u64 {
            let perf = PlayerPerformance {
                player_id: PlayerId(1),
                kills: 50,
                deaths: 0,
                assists: 0,
                objective_score: 100.0,
                impact: 10.0,
                variance: 0.0,
            };
            d.observe(&mr(i, perf), &world);
        }
        let result = d.evaluate(PlayerId(1), &world);
        assert!(
            result.probability_of_anomaly > 0.7,
            "p={}",
            result.probability_of_anomaly
        );
    }

    #[test]
    fn unknown_player_is_zero() {
        let d = detector();
        let world = World::new(SimRng::from_seed(42));
        let result = d.evaluate(PlayerId(999), &world);
        assert_eq!(result.probability_of_anomaly, 0.0);
        assert_eq!(result.confidence, 0.0);
        assert!(result.evidence.is_empty());
    }

    #[test]
    fn ladder_selects_ban_at_high_probability() {
        let mut world = World::new(SimRng::from_seed(42));
        world.observations.insert(PlayerId(1), obs(1, 1000.0));
        let mut d = sensitive_detector();
        for i in 0..5u64 {
            let perf = PlayerPerformance {
                player_id: PlayerId(1),
                kills: 50,
                deaths: 0,
                assists: 0,
                objective_score: 100.0,
                impact: 10.0,
                variance: 0.0,
            };
            d.observe(&mr(i, perf), &world);
        }
        let result = d.evaluate(PlayerId(1), &world);
        assert!(
            result.probability_of_anomaly >= 0.9,
            "p={}",
            result.probability_of_anomaly
        );
        assert!(matches!(
            d.recommend_action(&result),
            InterventionAction::Ban
        ));
    }
}
