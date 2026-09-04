use mlua::{Function, Lua};
use std::sync::Mutex;

pub struct LuaHooks {
    lua: Mutex<Lua>,
    script_path: String,
}

impl LuaHooks {
    pub fn load(path: &str) -> Result<Self, String> {
        let lua = Lua::new();
        let script = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path, e))?;
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

    pub fn call_match_quality(
        &self,
        team_a_avg: f64,
        team_b_avg: f64,
        queue_times: &[f64],
    ) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_match_quality").ok()?;
        func.call::<f64>((team_a_avg, team_b_avg, queue_times.to_vec()))
            .ok()
    }

    pub fn call_accept_match(
        &self,
        team_a: &[u64],
        team_b: &[u64],
        quality: f64,
        now_secs: f64,
    ) -> Option<bool> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_accept_match").ok()?;
        func.call::<bool>((team_a.to_vec(), team_b.to_vec(), quality, now_secs))
            .ok()
    }

    pub fn call_queue_priority(
        &self,
        rating: f64,
        wait_secs: f64,
        games_played: u64,
    ) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_queue_priority").ok()?;
        func.call::<f64>((rating, wait_secs, games_played as f64))
            .ok()
    }

    pub fn call_max_skill_diff(&self, longest_wait_secs: f64) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua
            .globals()
            .get::<Function>("on_max_skill_diff")
            .ok()?;
        func.call::<f64>(longest_wait_secs).ok()
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
        let path = dir.join(format!("test_mm_hooks_{}_{}.lua", std::process::id(), n));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_valid_script_succeeds() {
        let path = write_temp_lua("function on_match_quality() return 0.9 end");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert_eq!(hooks.call_match_quality(1000.0, 1000.0, &[]), Some(0.9));
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
    fn call_match_quality_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_match_quality");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_match_quality(1000.0, 1000.0, &[]).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_accept_match_returns_bool() {
        let path = write_temp_lua(
            r#"
function on_accept_match(team_a, team_b, quality, now)
    return quality > 0.85
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_accept_match(&[1], &[2], 0.9, 10.0).unwrap());
        assert!(!hooks.call_accept_match(&[1], &[2], 0.5, 10.0).unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_accept_match_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_accept_match");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_accept_match(&[1], &[2], 0.9, 10.0).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_queue_priority_returns_value() {
        let path = write_temp_lua(
            r#"
function on_queue_priority(rating, wait_secs, games_played)
    return rating + wait_secs * 10.0
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let priority = hooks.call_queue_priority(1000.0, 30.0, 50).unwrap();
        assert!((priority - 1300.0).abs() < 0.001);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_max_skill_diff_returns_value() {
        let path = write_temp_lua(
            r#"
function on_max_skill_diff(longest_wait)
    return 200.0 + longest_wait * 5.0
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let diff = hooks.call_max_skill_diff(10.0).unwrap();
        assert!((diff - 250.0).abs() < 0.001);
        let _ = std::fs::remove_file(&path);
    }
}
