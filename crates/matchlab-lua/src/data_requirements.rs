use mlua::prelude::*;

const KNOWN_OBSERVATION_FIELDS: &[&str] = &[
    "player_id",
    "rating",
    "hidden_mmr",
    "rating_deviation",
    "volatility",
    "games_played",
    "win_rate",
    "tilt_level",
    "is_online",
    "recent_performances",
    "queue_joined_at_secs",
    "queue_joined_at_ticks",
    "party_id",
    "role",
    "skill_overall",
    "skill_vector",
];

const KNOWN_MATCH_RESULT_FIELDS: &[&str] = &[
    "match_id",
    "winner",
    "team_a",
    "team_b",
    "team_a_score",
    "team_b_score",
    "duration_secs",
    "disconnected",
    "forfeited",
    "variance",
    "performances",
];

const KNOWN_QUEUE_FIELDS: &[&str] = &[
    "idx",
    "player_id",
    "rating",
    "rating_deviation",
    "games_played",
    "win_rate",
    "joined_at_secs",
    "wait_secs",
    "region",
    "party_id",
    "latency_ms",
    "game_mode",
    "role",
];

const KNOWN_SNAPSHOT_FIELDS: &[&str] = &["tick", "time_secs"];

const KNOWN_POPULATION_FIELDS: &[&str] = &["rating", "skill_overall", "true_skill"];

const KNOWN_REALITY_FIELDS: &[&str] = &[
    "true_skill",
    "improvement_rate",
    "reality_games_played",
    "archetype",
];

const KNOWN_BEHAVIOR_FIELDS: &[&str] = &[
    "quit_probability",
    "tilt_level",
    "party_id",
    "win_rate",
    "is_online",
];

const KNOWN_COMPLETED_MATCH_FIELDS: &[&str] = &["id", "winner", "team_a", "team_b", "time"];

fn warn_unknown_fields(fields: &[String], known: &[&str], category: &str) {
    for field in fields {
        if !known.contains(&field.as_str()) {
            tracing::warn!(field = %field, category = %category, "unknown field in data_requirements");
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DataRequirements {
    pub queue_fields: Vec<String>,
    pub match_result_fields: Vec<String>,
    pub observation_fields: Vec<String>,
    pub snapshot_fields: Vec<String>,
    pub population_fields: Vec<String>,
    pub reality_fields: Vec<String>,
    pub behavior_fields: Vec<String>,
    pub completed_match_fields: Vec<String>,
}

impl DataRequirements {
    pub fn from_lua_value(val: LuaValue) -> Result<Self, String> {
        let table = val.as_table().ok_or("data_requirements must be a table")?;
        let mut req = Self::default();

        if let Ok(v) = table.get::<LuaValue>("queue_fields") {
            if !v.is_nil() {
                req.queue_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("match_result_fields") {
            if !v.is_nil() {
                req.match_result_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("observation_fields") {
            if !v.is_nil() {
                req.observation_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("snapshot_fields") {
            if !v.is_nil() {
                req.snapshot_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("population_fields") {
            if !v.is_nil() {
                req.population_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("reality_fields") {
            if !v.is_nil() {
                req.reality_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("behavior_fields") {
            if !v.is_nil() {
                req.behavior_fields = string_array(v)?;
            }
        }
        if let Ok(v) = table.get::<LuaValue>("completed_match_fields") {
            if !v.is_nil() {
                req.completed_match_fields = string_array(v)?;
            }
        }

        warn_unknown_fields(&req.queue_fields, KNOWN_QUEUE_FIELDS, "queue_fields");
        warn_unknown_fields(
            &req.match_result_fields,
            KNOWN_MATCH_RESULT_FIELDS,
            "match_result_fields",
        );
        warn_unknown_fields(
            &req.observation_fields,
            KNOWN_OBSERVATION_FIELDS,
            "observation_fields",
        );
        warn_unknown_fields(
            &req.snapshot_fields,
            KNOWN_SNAPSHOT_FIELDS,
            "snapshot_fields",
        );
        warn_unknown_fields(
            &req.population_fields,
            KNOWN_POPULATION_FIELDS,
            "population_fields",
        );
        warn_unknown_fields(&req.reality_fields, KNOWN_REALITY_FIELDS, "reality_fields");
        warn_unknown_fields(
            &req.behavior_fields,
            KNOWN_BEHAVIOR_FIELDS,
            "behavior_fields",
        );
        warn_unknown_fields(
            &req.completed_match_fields,
            KNOWN_COMPLETED_MATCH_FIELDS,
            "completed_match_fields",
        );

        Ok(req)
    }

    pub fn has_observation_field(&self, field: &str) -> bool {
        self.observation_fields.iter().any(|f| f == field)
    }

    pub fn has_match_result_field(&self, field: &str) -> bool {
        self.match_result_fields.iter().any(|f| f == field)
    }

    pub fn has_queue_field(&self, field: &str) -> bool {
        self.queue_fields.iter().any(|f| f == field)
    }

    pub fn has_reality_field(&self, field: &str) -> bool {
        self.reality_fields.iter().any(|f| f == field)
    }

    pub fn has_population_field(&self, field: &str) -> bool {
        self.population_fields.iter().any(|f| f == field)
    }

    pub fn has_behavior_field(&self, field: &str) -> bool {
        self.behavior_fields.iter().any(|f| f == field)
    }

    pub fn has_snapshot_field(&self, field: &str) -> bool {
        self.snapshot_fields.iter().any(|f| f == field)
    }

    pub fn has_completed_match_field(&self, field: &str) -> bool {
        self.completed_match_fields.iter().any(|f| f == field)
    }
}

fn string_array(val: LuaValue) -> Result<Vec<String>, String> {
    let table = val.as_table().ok_or("expected array of strings")?;
    let mut result = Vec::new();
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (_, v) = pair.map_err(|e| e.to_string())?;
        if let Some(s) = v.as_str() {
            result.push(s.to_string());
        }
    }
    Ok(result)
}
