//! Lua-native adversarial agents.
//!
//! `LuaAdversarialAgent` implements the `AdversarialAgent` trait by delegating
//! to a script's `tick` / `objective` functions. The agent receives a
//! `behavior` table (the mutable reality/observation slice it may change) plus
//! the player's observation; the adapter writes the returned behavior back.
//! Randomness flows through `matchlab.rng_*` from `world.rng`.

use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use mlua::{Table, Value};

use crate::agent::{AdversarialAgent, AdversarialObjective};

/// An adversarial agent whose behavior lives entirely in a Lua script.
pub struct LuaAdversarialAgent {
    vm: LuaVm,
    objective: AdversarialObjective,
}

impl LuaAdversarialAgent {
    pub fn load(path: &str, params: &serde_yaml::Value, player: PlayerId) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["tick", "objective"])?;
        let objective = read_objective(&vm, player)?;
        Ok(Self { vm, objective })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

fn read_objective(vm: &LuaVm, player: PlayerId) -> Result<AdversarialObjective, String> {
    let obj_tbl: Table = vm.call_with_context("objective", &[])?;
    let kind = obj_tbl.get::<String>("kind").unwrap_or_default();
    objective_from_str(&kind, player).ok_or_else(|| format!("unknown objective kind: {kind}"))
}

fn objective_from_str(kind: &str, player: PlayerId) -> Option<AdversarialObjective> {
    match kind {
        "MaximizeRating" => Some(AdversarialObjective::MaximizeRating),
        "MinimizeGamesPlayed" => Some(AdversarialObjective::MinimizeGamesPlayed),
        "MaximizeWinRate" => Some(AdversarialObjective::MaximizeWinRate { target_games: 10 }),
        "MaintainLowRating" => Some(AdversarialObjective::MaintainLowRating),
        "Derate" => Some(AdversarialObjective::Derate),
        "WinTrade" => Some(AdversarialObjective::WinTrade { partner: player }),
        _ => None,
    }
}

fn behavior_to_table(
    world: &World,
    player_id: PlayerId,
    lua: &mlua::Lua,
) -> Result<(Table, Option<Table>), String> {
    let behavior = lua.create_table().map_err(|e| e.to_string())?;
    let mut observation = None;
    if let Some(reality) = world.players.get(&player_id) {
        behavior
            .set("quit_probability", reality.quit_probability)
            .map_err(|e| e.to_string())?;
        match reality.party_id {
            Some(pid) => behavior.set("party_id", pid).map_err(|e| e.to_string())?,
            None => behavior
                .set("party_id", Value::Nil)
                .map_err(|e| e.to_string())?,
        }
    } else {
        behavior
            .set("quit_probability", 0.0)
            .map_err(|e| e.to_string())?;
    }
    if let Some(obs) = world.observations.get(&player_id) {
        behavior
            .set("tilt_level", obs.tilt_level)
            .map_err(|e| e.to_string())?;
        behavior
            .set("win_rate", obs.win_rate)
            .map_err(|e| e.to_string())?;
        behavior
            .set("is_online", obs.is_online)
            .map_err(|e| e.to_string())?;
        match obs.party_id {
            Some(pid) => behavior.set("party_id", pid).map_err(|e| e.to_string())?,
            None => behavior
                .set("party_id", Value::Nil)
                .map_err(|e| e.to_string())?,
        }
        observation = Some(convert::observation_to_table(lua, obs, false)?);
    }
    Ok((behavior, observation))
}

fn write_behavior(world: &mut World, player_id: PlayerId, behavior: &Table) -> Result<(), String> {
    if let Some(reality) = world.players.get_mut(&player_id) {
        if let Ok(qp) = behavior.get::<f64>("quit_probability") {
            reality.quit_probability = qp;
        }
        if let Ok(party) = behavior.get::<Option<u64>>("party_id") {
            reality.party_id = party;
        }
    }
    if let Some(obs) = world.observations.get_mut(&player_id) {
        if let Ok(tilt) = behavior.get::<f64>("tilt_level") {
            obs.tilt_level = tilt;
        }
        if let Ok(wr) = behavior.get::<f64>("win_rate") {
            obs.win_rate = wr;
        }
        if let Ok(online) = behavior.get::<bool>("is_online") {
            obs.is_online = online;
        }
        if let Ok(party) = behavior.get::<Option<u64>>("party_id") {
            obs.party_id = party;
        }
    }
    Ok(())
}

impl AdversarialAgent for LuaAdversarialAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        let (behavior, observation) = self
            .vm
            .with_lua(|lua| behavior_to_table(world, player_id, lua))
            .expect("build behavior table");
        let behavior_val = Value::Table(behavior.clone());
        let obs_val = observation.map(Value::Table).unwrap_or(Value::Nil);

        let new_behavior: Table = self.vm.with_rng(&mut world.rng, |vm| {
            vm.call_with_context(
                "tick",
                &[Value::Integer(player_id.0 as i64), behavior_val, obs_val],
            )
            .expect("agent tick failed")
        });

        write_behavior(world, player_id, &new_behavior).expect("write behavior back");
    }

    fn objective(&self) -> AdversarialObjective {
        self.objective.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{PlayerObservation, PlayerReality, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use std::collections::VecDeque;

    fn reality(id: u64, quit_probability: f64) -> PlayerReality {
        PlayerReality {
            id: PlayerId(id),
            skill: SkillVector::one_dimensional(1000.0),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability,
            party_id: None,
            region: matchlab_core::player::Region::NA,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".into(),
        }
    }

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

    fn world_with(id: u64, rating: f64) -> World {
        let mut w = World::new(SimRng::from_seed(7));
        w.add_player(reality(id, 0.01), obs(id, rating));
        w
    }

    fn agent(path: &str, yaml: &str, player: u64) -> LuaAdversarialAgent {
        LuaAdversarialAgent::load(path, &serde_yaml::from_str(yaml).unwrap(), PlayerId(player))
            .unwrap()
    }

    #[test]
    fn afk_sets_quit_probability() {
        let mut w = world_with(1, 1000.0);
        let mut a = agent("plugins/adversarial/afk.lua", "go_afk_probability: 1.0", 1);
        assert_eq!(a.objective(), AdversarialObjective::MinimizeGamesPlayed);
        a.tick(PlayerId(1), &mut w);
        assert_eq!(w.players[&PlayerId(1)].quit_probability, 1.0);
    }

    #[test]
    fn afk_zero_never_quits() {
        let mut w = world_with(1, 1000.0);
        let mut a = agent("plugins/adversarial/afk.lua", "go_afk_probability: 0.0", 1);
        a.tick(PlayerId(1), &mut w);
        assert_eq!(w.players[&PlayerId(1)].quit_probability, 0.01);
    }

    #[test]
    fn deranker_throws_above_target() {
        let mut w = world_with(1, 1200.0);
        let mut a = agent(
            "plugins/adversarial/deranker.lua",
            "target_rating: 500.0",
            1,
        );
        assert_eq!(a.objective(), AdversarialObjective::MaintainLowRating);
        a.tick(PlayerId(1), &mut w);
        assert_eq!(w.players[&PlayerId(1)].quit_probability, 0.9);
        assert_eq!(w.observations[&PlayerId(1)].tilt_level, 1.0);
    }

    #[test]
    fn deranker_stops_below_target() {
        let mut w = world_with(1, 400.0);
        let mut a = agent(
            "plugins/adversarial/deranker.lua",
            "target_rating: 500.0",
            1,
        );
        a.tick(PlayerId(1), &mut w);
        assert_eq!(w.players[&PlayerId(1)].quit_probability, 0.01);
    }

    #[test]
    fn win_trader_links_party() {
        let mut w = world_with(1, 1000.0);
        w.add_player(reality(2, 0.01), obs(2, 1000.0));
        let mut a = agent(
            "plugins/adversarial/win_trader.lua",
            "partner: 2\nalternating: false",
            1,
        );
        a.tick(PlayerId(1), &mut w);
        assert_eq!(w.observations[&PlayerId(1)].party_id, Some(3));
        assert_eq!(w.players[&PlayerId(1)].party_id, Some(3));
    }

    #[test]
    fn rating_farmer_goes_offline() {
        let mut w = world_with(1, 1000.0);
        let mut a = agent(
            "plugins/adversarial/rating_farmer.lua",
            "quit_probability: 1.0\nquit_after_minutes: 5.0",
            1,
        );
        a.tick(PlayerId(1), &mut w);
        assert_eq!(w.players[&PlayerId(1)].quit_probability, 1.0);
        assert!(!w.observations[&PlayerId(1)].is_online);
    }
}
