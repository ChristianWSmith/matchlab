use mlua::prelude::*;

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
