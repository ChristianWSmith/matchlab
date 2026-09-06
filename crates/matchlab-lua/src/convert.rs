//! Core type marshalling between Rust and Lua.
//!
//! Truth-separation note: the observation table carries the ground-truth skill
//! binding (`skill_overall`, `skill_vector`) only when `include_skill` is true.
//! The outcome model and metric snapshots pass `true` (the game decides winners
//! from true skill; metrics may read reality); rating, matchmaking, detection,
//! and adversarial adapters pass `false`.

use matchlab_core::match_::{MatchResult, PlayerPerformance, Team};
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality, Region};
use matchlab_core::world::World;
use mlua::{Lua, Table, Value};

/// Convert a `PlayerObservation` into a Lua table.
///
/// `include_skill` adds `skill_overall` and the `skill_vector` dimension map —
/// permitted for the outcome model and metrics only.
pub fn observation_to_table(
    lua: &Lua,
    obs: &PlayerObservation,
    include_skill: bool,
) -> Result<Table, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    t.set("player_id", obs.id.0).map_err(|e| e.to_string())?;
    t.set("rating", obs.rating).map_err(|e| e.to_string())?;
    t.set("hidden_mmr", obs.hidden_mmr)
        .map_err(|e| e.to_string())?;
    t.set("rating_deviation", obs.rating_deviation)
        .map_err(|e| e.to_string())?;
    t.set("volatility", obs.volatility)
        .map_err(|e| e.to_string())?;
    t.set("games_played", obs.games_played)
        .map_err(|e| e.to_string())?;
    t.set("win_rate", obs.win_rate).map_err(|e| e.to_string())?;
    t.set("tilt_level", obs.tilt_level)
        .map_err(|e| e.to_string())?;
    t.set("is_online", obs.is_online)
        .map_err(|e| e.to_string())?;
    let recent = lua.create_table().map_err(|e| e.to_string())?;
    for (i, v) in obs.recent_performances.iter().enumerate() {
        recent.set(i + 1, *v).map_err(|e| e.to_string())?;
    }
    t.set("recent_performances", recent)
        .map_err(|e| e.to_string())?;
    set_opt_number(
        &t,
        "queue_joined_at_secs",
        obs.queue_joined_at.map(|s| s.as_secs_f64()),
    )?;
    set_opt_int(
        &t,
        "queue_joined_at_ticks",
        obs.queue_joined_at.map(|s| s.ticks()),
    )?;
    set_opt_int(&t, "party_id", obs.party_id)?;
    match &obs.role {
        Some(r) => t.set("role", r.as_str()).map_err(|e| e.to_string())?,
        None => t.set("role", Value::Nil).map_err(|e| e.to_string())?,
    }
    if include_skill {
        t.set("skill_overall", obs.skill_vector.overall())
            .map_err(|e| e.to_string())?;
        let dims = lua.create_table().map_err(|e| e.to_string())?;
        for (dim, &val) in &obs.skill_vector.dimensions {
            dims.set(dim.as_str(), val).map_err(|e| e.to_string())?;
        }
        t.set("skill_vector", dims).map_err(|e| e.to_string())?;
    }
    Ok(t)
}

/// Convert a participant observation into a Lua table, appending reality
/// fields when a `PlayerReality` is supplied. **Metrics only.**
pub fn participant_to_table(
    lua: &Lua,
    obs: &PlayerObservation,
    reality: Option<&PlayerReality>,
) -> Result<Table, String> {
    let t = observation_to_table(lua, obs, true)?;
    if let Some(r) = reality {
        t.set("true_skill", r.skill.overall())
            .map_err(|e| e.to_string())?;
        t.set("improvement_rate", r.improvement_rate)
            .map_err(|e| e.to_string())?;
        t.set("reality_games_played", r.games_played)
            .map_err(|e| e.to_string())?;
        t.set("archetype", r.archetype.as_str())
            .map_err(|e| e.to_string())?;
    }
    Ok(t)
}

/// Convert a list of observations into a Lua array of tables.
pub fn observations_to_value(
    lua: &Lua,
    list: &[PlayerObservation],
    include_skill: bool,
) -> Result<Value, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for (i, obs) in list.iter().enumerate() {
        t.set(i + 1, observation_to_table(lua, obs, include_skill)?)
            .map_err(|e| e.to_string())?;
    }
    Ok(Value::Table(t))
}

/// Convert a list of observations into a Lua table keyed by `player_id`.
/// Convenient for rating/detection adapters that index participants by id.
pub fn observations_to_map(
    lua: &Lua,
    list: &[PlayerObservation],
    include_skill: bool,
) -> Result<Value, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for obs in list {
        t.set(obs.id.0, observation_to_table(lua, obs, include_skill)?)
            .map_err(|e| e.to_string())?;
    }
    Ok(Value::Table(t))
}

fn team_to_value(lua: &Lua, team: &[PlayerId]) -> Result<Value, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for (i, id) in team.iter().enumerate() {
        t.set(i + 1, id.0).map_err(|e| e.to_string())?;
    }
    Ok(Value::Table(t))
}

fn performance_to_table(lua: &Lua, p: &PlayerPerformance) -> Result<Table, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    t.set("player_id", p.player_id.0)
        .map_err(|e| e.to_string())?;
    t.set("kills", p.kills).map_err(|e| e.to_string())?;
    t.set("deaths", p.deaths).map_err(|e| e.to_string())?;
    t.set("assists", p.assists).map_err(|e| e.to_string())?;
    t.set("objective_score", p.objective_score)
        .map_err(|e| e.to_string())?;
    t.set("impact", p.impact).map_err(|e| e.to_string())?;
    t.set("variance", p.variance).map_err(|e| e.to_string())?;
    Ok(t)
}

/// Convert a `MatchResult` into a Lua table.
pub fn match_result_to_table(lua: &Lua, mr: &MatchResult) -> Result<Table, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    t.set("match_id", mr.match_id.0)
        .map_err(|e| e.to_string())?;
    t.set("winner", team_str(mr.winner))
        .map_err(|e| e.to_string())?;
    t.set("team_a", team_to_value(lua, &mr.team_a)?)
        .map_err(|e| e.to_string())?;
    t.set("team_b", team_to_value(lua, &mr.team_b)?)
        .map_err(|e| e.to_string())?;
    t.set("team_a_score", mr.team_a_score)
        .map_err(|e| e.to_string())?;
    t.set("team_b_score", mr.team_b_score)
        .map_err(|e| e.to_string())?;
    t.set("duration_secs", mr.duration.as_secs_f64())
        .map_err(|e| e.to_string())?;
    t.set("disconnected", mr.disconnected)
        .map_err(|e| e.to_string())?;
    t.set("forfeited", mr.forfeited)
        .map_err(|e| e.to_string())?;
    t.set("variance", mr.variance).map_err(|e| e.to_string())?;
    let perfs = lua.create_table().map_err(|e| e.to_string())?;
    for (i, p) in mr.player_performances.iter().enumerate() {
        perfs
            .set(i + 1, performance_to_table(lua, p)?)
            .map_err(|e| e.to_string())?;
    }
    t.set("performances", perfs).map_err(|e| e.to_string())?;
    Ok(t)
}

/// The metrics-only snapshot: the match result, the current tick, and
/// per-participant tables carrying observation + reality fields.
pub fn metric_snapshot(lua: &Lua, mr: &MatchResult, world: &World) -> Result<Value, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    t.set("match_result", match_result_to_table(lua, mr)?)
        .map_err(|e| e.to_string())?;
    t.set("tick", world.time.ticks())
        .map_err(|e| e.to_string())?;
    t.set("time_secs", world.time.as_secs_f64())
        .map_err(|e| e.to_string())?;
    let players = participant_players(lua, mr, world)?;
    t.set("players", players).map_err(|e| e.to_string())?;
    Ok(Value::Table(t))
}

fn participant_players(lua: &Lua, mr: &MatchResult, world: &World) -> Result<Value, String> {
    let players = lua.create_table().map_err(|e| e.to_string())?;
    let mut ids: Vec<PlayerId> = mr.team_a.iter().chain(mr.team_b.iter()).copied().collect();
    ids.sort_by_key(|id| id.0);
    for (i, pid) in ids.iter().enumerate() {
        if let Some(obs) = world.observations.get(pid) {
            let row = participant_to_table(lua, obs, world.players.get(pid))?;
            players.set(i + 1, row).map_err(|e| e.to_string())?;
        }
    }
    Ok(Value::Table(players))
}

/// The full population snapshot for population-level metrics (metrics only):
/// every observation + reality pair, sorted by player id.
pub fn population_snapshot(lua: &Lua, world: &World) -> Result<Value, String> {
    let players = lua.create_table().map_err(|e| e.to_string())?;
    let mut ids: Vec<PlayerId> = world.observations.keys().copied().collect();
    ids.sort_by_key(|id| id.0);
    for (i, pid) in ids.iter().enumerate() {
        if let Some(obs) = world.observations.get(pid) {
            let row = participant_to_table(lua, obs, world.players.get(pid))?;
            players.set(i + 1, row).map_err(|e| e.to_string())?;
        }
    }
    Ok(Value::Table(players))
}

/// Region to a stable string for matchmaking scripts.
pub fn region_str(region: Region) -> &'static str {
    match region {
        Region::NA => "na",
        Region::EU => "eu",
        Region::Asia => "asia",
        Region::Other => "other",
    }
}

/// Team to a stable string.
pub fn team_str(team: Team) -> &'static str {
    match team {
        Team::A => "A",
        Team::B => "B",
    }
}

fn set_opt_number(t: &Table, key: &str, value: Option<f64>) -> Result<(), String> {
    match value {
        Some(v) => t.set(key, v).map_err(|e| e.to_string()),
        None => t.set(key, Value::Nil).map_err(|e| e.to_string()),
    }
}

fn set_opt_int(t: &Table, key: &str, value: Option<u64>) -> Result<(), String> {
    match value {
        Some(v) => t.set(key, v).map_err(|e| e.to_string()),
        None => t.set(key, Value::Nil).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "gold".into(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: Some(SimTime::from_secs(1.0)),
            is_online: true,
            party_id: Some(3),
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(1200.0),
            detection_flags: Vec::new(),
            role: None,
        }
    }

    #[test]
    fn observation_table_fields() {
        let lua = Lua::new();
        let o = obs(1, 1000.0);
        let t = observation_to_table(&lua, &o, true).unwrap();
        assert_eq!(t.get::<u64>("player_id").unwrap(), 1);
        assert_eq!(t.get::<f64>("rating").unwrap(), 1000.0);
        assert_eq!(t.get::<f64>("skill_overall").unwrap(), 1200.0);
        assert_eq!(t.get::<f64>("queue_joined_at_secs").unwrap(), 1.0);
        assert_eq!(t.get::<u64>("party_id").unwrap(), 3);
    }

    #[test]
    fn skill_fields_omitted_when_disallowed() {
        let lua = Lua::new();
        let o = obs(1, 1000.0);
        let t = observation_to_table(&lua, &o, false).unwrap();
        assert!(t.get::<mlua::Value>("skill_overall").unwrap().is_nil());
        assert!(t.get::<mlua::Value>("skill_vector").unwrap().is_nil());
    }

    #[test]
    fn role_is_exposed_without_include_skill() {
        let lua = Lua::new();
        let mut o = obs(1, 1000.0);
        o.role = Some("killer".to_string());
        let t = observation_to_table(&lua, &o, false).unwrap();
        assert_eq!(t.get::<String>("role").unwrap(), "killer");
    }

    #[test]
    fn role_is_nil_when_absent() {
        let lua = Lua::new();
        let o = obs(1, 1000.0);
        let t = observation_to_table(&lua, &o, false).unwrap();
        assert!(t.get::<mlua::Value>("role").unwrap().is_nil());
    }

    #[test]
    fn participant_table_includes_reality() {
        let lua = Lua::new();
        let o = obs(1, 1000.0);
        let r = PlayerReality {
            id: PlayerId(1),
            skill: SkillVector::one_dimensional(1500.0),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::NA,
            account_age: 0,
            games_played: 3,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "smurf".into(),
            role: None,
        };
        let t = participant_to_table(&lua, &o, Some(&r)).unwrap();
        assert_eq!(t.get::<f64>("true_skill").unwrap(), 1500.0);
        assert_eq!(t.get::<u64>("reality_games_played").unwrap(), 3);
        assert_eq!(t.get::<String>("archetype").unwrap(), "smurf");
    }

    #[test]
    fn match_result_table_fields() {
        let lua = Lua::new();
        let mr = MatchResult {
            match_id: MatchId(9),
            winner: Team::A,
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: vec![PlayerPerformance {
                player_id: PlayerId(1),
                kills: 10,
                deaths: 2,
                assists: 4,
                objective_score: 55.0,
                impact: 0.8,
                variance: 0.2,
            }],
            duration: SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.05,
            unexpected_events: Vec::new(),
        };
        let t = match_result_to_table(&lua, &mr).unwrap();
        assert_eq!(t.get::<u64>("match_id").unwrap(), 9);
        assert_eq!(t.get::<String>("winner").unwrap(), "A");
        assert_eq!(t.get::<f64>("duration_secs").unwrap(), 1800.0);
        let perfs: Table = t.get("performances").unwrap();
        let first: Table = perfs.get(1).unwrap();
        assert_eq!(first.get::<u32>("kills").unwrap(), 10);
    }

    #[test]
    fn metric_snapshot_builds() {
        let lua = Lua::new();
        let mut world = World::new(SimRng::from_seed(1));
        world.add_player(
            PlayerReality {
                id: PlayerId(1),
                skill: SkillVector::one_dimensional(1500.0),
                skill_volatility: 5.0,
                improvement_rate: 0.0,
                consistency: 0.9,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                party_id: None,
                region: Region::NA,
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: "stable".into(),
                role: None,
            },
            obs(1, 1000.0),
        );
        let mr = MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        };
        let snap = metric_snapshot(&lua, &mr, &world).unwrap();
        let t = snap.as_table().unwrap();
        let players: Table = t.get("players").unwrap();
        assert_eq!(players.raw_len(), 1);
        let row: Table = players.get(1).unwrap();
        assert_eq!(row.get::<f64>("true_skill").unwrap(), 1500.0);
    }
}
