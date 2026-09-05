use mlua::{Function, Lua};
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

    pub fn call_on_record(&self, winner: &str, team_a_avg: f64, team_b_avg: f64) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_record").ok()?;
        func.call::<f64>((winner, team_a_avg, team_b_avg)).ok()
    }

    pub fn call_bucket_config(&self) -> Option<Vec<f64>> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_bucket_config").ok()?;
        func.call::<Vec<f64>>(()).ok()
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
        let path = dir.join(format!(
            "test_metrics_hooks_{}_{}.lua",
            std::process::id(),
            n
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_valid_script_succeeds() {
        let path = write_temp_lua("function on_record() return 1.0 end");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert_eq!(hooks.call_on_record("A", 1000.0, 1000.0), Some(1.0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_invalid_script_fails() {
        let path = write_temp_lua("invalid lua {{{");
        let result = LuaHooks::load(path.to_str().unwrap());
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_fails() {
        let result = LuaHooks::load("/nonexistent/script.lua");
        assert!(result.is_err());
    }

    #[test]
    fn call_on_record_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_record");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_on_record("A", 1000.0, 1000.0).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_bucket_config_returns_vec() {
        let path = write_temp_lua(
            r#"
function on_bucket_config()
    return {0.0, 100.0, 200.0, 300.0}
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let buckets = hooks.call_bucket_config().unwrap();
        assert_eq!(buckets.len(), 4);
        assert!((buckets[0] - 0.0).abs() < 0.001);
        assert!((buckets[3] - 300.0).abs() < 0.001);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_bucket_config_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_bucket_config");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_bucket_config().is_none());
        let _ = std::fs::remove_file(&path);
    }
}
