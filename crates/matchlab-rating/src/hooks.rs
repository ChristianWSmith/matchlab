use mlua::{Function, Lua, Table};
use std::sync::Mutex;

pub struct LuaHooks {
    lua: Mutex<Lua>,
    script_path: String,
}

impl LuaHooks {
    pub fn load(path: &str) -> Result<Self, String> {
        let lua = Lua::new();
        let script =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
        lua.load(&script)
            .exec()
            .map_err(|e| format!("lua error in {}: {}", path, e))?;
        Ok(Self {
            lua: Mutex::new(lua),
            script_path: path.to_string(),
        })
    }

    pub fn script_path(&self) -> &str {
        &self.script_path
    }

    pub(crate) fn lua(&self) -> &Mutex<Lua> {
        &self.lua
    }

    pub fn call_k_factor(
        &self,
        player_id: u64,
        rating: f64,
        games_played: u64,
        recent_win_rate: f64,
    ) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_k_factor").ok()?;
        func.call::<f64>((player_id, rating, games_played as f64, recent_win_rate))
            .ok()
    }

    pub fn call_rating_bounds(&self) -> Option<(f64, f64)> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_rating_bounds").ok()?;
        let table: Table = func.call::<Table>(()).ok()?;
        let floor: f64 = table.get("floor").ok()?;
        let ceiling: f64 = table.get("ceiling").ok()?;
        Some((floor, ceiling))
    }

    pub fn call_initial_rating(&self, archetype_name: &str) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_initial_rating").ok()?;
        func.call::<f64>(archetype_name).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp_lua(content: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("test_hooks_{}_{}.lua", std::process::id(), n));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_valid_script_succeeds() {
        let path = write_temp_lua("function on_k_factor() return 32.0 end");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert_eq!(hooks.call_k_factor(1, 1000.0, 0, 0.5), Some(32.0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_invalid_script_fails() {
        let path = write_temp_lua("this is not valid lua");
        let result = LuaHooks::load(path.to_str().unwrap());
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_fails() {
        let result = LuaHooks::load("/nonexistent/path/script.lua");
        assert!(result.is_err());
    }

    #[test]
    fn call_k_factor_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_k_factor defined");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_k_factor(1, 1000.0, 0, 0.5).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_rating_bounds_returns_tuple() {
        let path = write_temp_lua(
            r#"
function on_rating_bounds()
    return { floor = 100.0, ceiling = 3000.0 }
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let (floor, ceiling) = hooks.call_rating_bounds().unwrap();
        assert_eq!(floor, 100.0);
        assert_eq!(ceiling, 3000.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_rating_bounds_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_rating_bounds");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_rating_bounds().is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_initial_rating_returns_value() {
        let path = write_temp_lua(
            r#"
function on_initial_rating(name)
    if name == "smurf" then return 700.0 end
    return 1000.0
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert_eq!(hooks.call_initial_rating("smurf"), Some(700.0));
        assert_eq!(hooks.call_initial_rating("stable"), Some(1000.0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_initial_rating_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_initial_rating");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_initial_rating("smurf").is_none());
        let _ = std::fs::remove_file(&path);
    }
}
