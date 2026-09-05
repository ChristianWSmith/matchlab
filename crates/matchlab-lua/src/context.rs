//! The `Context`: arbitrary, script-defined data persisted by a Rust model.
//!
//! Every Lua call receives the current context (converted to a Lua table) and
//! the adapter stores whatever the script returns/mutates. Storing it as an
//! ordered `serde_yaml::Value` keeps it serializable and deterministic, and it
//! round-trips exactly with the YAML params that become the script's `config`.

use mlua::{Lua, Value};
use serde_yaml::Value as YamlValue;

fn mlua_err(e: mlua::Error) -> String {
    e.to_string()
}

/// Opaque script-owned state. Defaults to an empty ordered mapping.
pub type Context = YamlValue;

/// A fresh empty context (an ordered mapping).
pub fn empty() -> Context {
    Context::Mapping(Default::default())
}

/// Convert a YAML value (config or context) into a Lua value.
pub fn yaml_to_lua(lua: &Lua, value: &YamlValue) -> Result<Value, String> {
    match value {
        YamlValue::Null => Ok(Value::Nil),
        YamlValue::Bool(b) => Ok(Value::Boolean(*b)),
        YamlValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(u) = n.as_u64() {
                // Lua integers are signed; convert via f64 when too large.
                Ok(Value::Integer(u as i64))
            } else {
                Ok(Value::Number(n.as_f64().unwrap_or(0.0)))
            }
        }
        YamlValue::String(s) => Ok(Value::String(lua.create_string(s).map_err(mlua_err)?)),
        YamlValue::Sequence(seq) => {
            let table = lua.create_table().map_err(mlua_err)?;
            for (i, item) in seq.iter().enumerate() {
                table
                    .set(i + 1, yaml_to_lua(lua, item)?)
                    .map_err(mlua_err)?;
            }
            Ok(Value::Table(table))
        }
        YamlValue::Mapping(map) => {
            let table = lua.create_table().map_err(mlua_err)?;
            for (k, v) in map {
                table
                    .set(yaml_to_lua(lua, k)?, yaml_to_lua(lua, v)?)
                    .map_err(mlua_err)?;
            }
            Ok(Value::Table(table))
        }
        YamlValue::Tagged(t) => yaml_to_lua(lua, &t.value),
    }
}

/// Convert a Lua value back into a YAML value (context round-trip).
///
/// A table whose keys are exactly `1..=n` is treated as a sequence; any other
/// table becomes a mapping (iteration order preserved).
pub fn lua_to_yaml(value: &Value) -> Result<YamlValue, String> {
    match value {
        Value::Nil => Ok(YamlValue::Null),
        Value::Boolean(b) => Ok(YamlValue::Bool(*b)),
        Value::Integer(i) => Ok(YamlValue::Number(serde_yaml::Number::from(*i))),
        Value::Number(n) => Ok(YamlValue::Number(serde_yaml::Number::from(*n))),
        Value::String(s) => Ok(YamlValue::String(s.to_str().map_err(mlua_err)?.to_string())),
        Value::Table(table) => {
            let mut entries: Vec<(Value, Value)> = Vec::new();
            for pair in table.clone().pairs::<Value, Value>() {
                entries.push(pair.map_err(|e| format!("lua table iteration: {e}"))?);
            }

            let n = entries.len();
            let is_sequence = n > 0
                && entries
                    .iter()
                    .enumerate()
                    .all(|(i, (k, _))| matches!(k, Value::Integer(idx) if *idx == i as i64 + 1));

            if is_sequence {
                let mut seq = Vec::with_capacity(n);
                for (_, v) in entries {
                    seq.push(lua_to_yaml(&v)?);
                }
                Ok(YamlValue::Sequence(seq))
            } else {
                let mut map = serde_yaml::Mapping::new();
                for (k, v) in entries {
                    map.insert(lua_to_yaml(&k)?, lua_to_yaml(&v)?);
                }
                Ok(YamlValue::Mapping(map))
            }
        }
        Value::Error(_)
        | Value::UserData(_)
        | Value::Function(_)
        | Value::Thread(_)
        | Value::LightUserData(_)
        | Value::Other(_) => Err("unsupported Lua value in context".to_string()),
    }
}

/// Convert a context into a Lua value for passing into a call.
pub fn to_lua(lua: &Lua, context: &Context) -> Result<Value, String> {
    yaml_to_lua(lua, context)
}

/// Convert a Lua value back into a stored context.
pub fn from_lua(value: &Value) -> Result<Context, String> {
    lua_to_yaml(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn empty_is_an_ordered_mapping() {
        let ctx = empty();
        assert!(matches!(ctx, YamlValue::Mapping(_)));
        let lua = Lua::new();
        let val = to_lua(&lua, &ctx).unwrap();
        assert!(matches!(val, Value::Table(_)));
    }

    #[test]
    fn yaml_to_lua_scalars() {
        let lua = Lua::new();
        assert!(matches!(
            yaml_to_lua(&lua, &YamlValue::Bool(true)).unwrap(),
            Value::Boolean(true)
        ));
        assert!(matches!(
            yaml_to_lua(&lua, &YamlValue::Number(serde_yaml::Number::from(7))).unwrap(),
            Value::Integer(7)
        ));
        assert!(matches!(
            yaml_to_lua(&lua, &YamlValue::String("x".into())).unwrap(),
            Value::String(_)
        ));
    }

    #[test]
    fn round_trip_mapping_with_sequence() {
        let yaml: YamlValue = serde_yaml::from_str("a: 1\nb: [2, 3, {c: 4}]").unwrap();
        let lua = Lua::new();
        let val = yaml_to_lua(&lua, &yaml).unwrap();
        let back = lua_to_yaml(&val).unwrap();
        assert_eq!(yaml, back, "round-trip must preserve structure");
    }

    #[test]
    fn round_trip_nested_tables() {
        let yaml: YamlValue =
            serde_yaml::from_str("samples: [0.5, 0.7, 0.9]\nmeta: { count: 3, label: hi }")
                .unwrap();
        let lua = Lua::new();
        let val = yaml_to_lua(&lua, &yaml).unwrap();
        let back = lua_to_yaml(&val).unwrap();
        assert_eq!(yaml, back);
    }

    #[test]
    fn sequence_detection_requires_exact_indices() {
        let lua = Lua::new();
        let table = lua.create_table().unwrap();
        table.set(1, 10.0).unwrap();
        table.set(2, 20.0).unwrap();
        table.set("extra", 30.0).unwrap();
        let val = Value::Table(table);
        let yaml = lua_to_yaml(&val).unwrap();
        // Not a pure sequence -> mapping.
        assert!(matches!(yaml, YamlValue::Mapping(_)));
    }
}
