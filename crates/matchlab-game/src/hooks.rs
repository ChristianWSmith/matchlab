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

    pub fn call_effective_skill(&self, rating: f64, rd: f64, games_played: u64) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_effective_skill").ok()?;
        func.call::<f64>((rating, rd, games_played as f64)).ok()
    }

    pub fn call_noise(&self, match_duration_secs: f64, team_size: usize) -> Option<f64> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_noise").ok()?;
        func.call::<f64>((match_duration_secs, team_size as f64))
            .ok()
    }

    pub fn call_post_process(
        &self,
        winner: &str,
        team_a_score: f64,
        team_b_score: f64,
    ) -> Option<(String, f64, f64)> {
        let lua = self.lua.lock().ok()?;
        let func = lua.globals().get::<Function>("on_post_process").ok()?;
        func.call::<(String, f64, f64)>((winner, team_a_score, team_b_score))
            .ok()
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
        let path = dir.join(format!("test_game_hooks_{}_{}.lua", std::process::id(), n));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_valid_script_succeeds() {
        let path = write_temp_lua("function on_effective_skill() return 1200.0 end");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert_eq!(hooks.call_effective_skill(1000.0, 350.0, 0), Some(1200.0));
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
    fn call_effective_skill_returns_none_when_undefined() {
        let path = write_temp_lua("-- no on_effective_skill");
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        assert!(hooks.call_effective_skill(1000.0, 350.0, 0).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_noise_returns_value() {
        let path = write_temp_lua(
            r#"
function on_noise(duration, team_size)
    return 0.05 + duration / 10000.0
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let noise = hooks.call_noise(1800.0, 5).unwrap();
        assert!((noise - 0.23).abs() < 0.001);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_post_process_returns_tuple() {
        let path = write_temp_lua(
            r#"
function on_post_process(winner, a_score, b_score)
    return winner, a_score + 1.0, b_score + 1.0
end
"#,
        );
        let hooks = LuaHooks::load(path.to_str().unwrap()).unwrap();
        let (w, a, b) = hooks.call_post_process("A", 13.0, 5.0).unwrap();
        assert_eq!(w, "A");
        assert!((a - 14.0).abs() < 0.001);
        assert!((b - 6.0).abs() < 0.001);
        let _ = std::fs::remove_file(&path);
    }
}
