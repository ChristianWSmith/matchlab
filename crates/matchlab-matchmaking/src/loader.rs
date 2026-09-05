use crate::hooks::LuaHooks;

pub struct ScriptValidationResult {
    pub path: String,
    pub valid: bool,
    pub error: Option<String>,
    pub defined_hooks: Vec<String>,
}

pub struct ScriptLoader;

impl ScriptLoader {
    pub fn load(path: &str) -> Result<LuaHooks, String> {
        LuaHooks::load(path)
    }

    pub fn validate(path: &str) -> ScriptValidationResult {
        match LuaHooks::load(path) {
            Ok(hooks) => {
                let defined = Self::discover_hooks(&hooks);
                ScriptValidationResult {
                    path: path.to_string(),
                    valid: true,
                    error: None,
                    defined_hooks: defined,
                }
            }
            Err(e) => ScriptValidationResult {
                path: path.to_string(),
                valid: false,
                error: Some(e),
                defined_hooks: Vec::new(),
            },
        }
    }

    pub fn validate_batch(paths: &[&str]) -> Vec<ScriptValidationResult> {
        paths.iter().map(|p| Self::validate(p)).collect()
    }

    fn discover_hooks(hooks: &LuaHooks) -> Vec<String> {
        let mut defined = Vec::new();
        let lua = hooks.lua().lock().ok().unwrap();
        let globals = lua.globals();

        let hook_names = [
            "on_match_quality",
            "on_accept_match",
            "on_queue_priority",
            "on_max_skill_diff",
        ];

        for name in &hook_names {
            if globals.get::<mlua::Function>(*name).is_ok() {
                defined.push(name.to_string());
            }
        }

        defined
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
        let path = dir.join(format!("test_mm_loader_{}_{}.lua", std::process::id(), n));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn validate_valid_script() {
        let path = write_temp_lua(
            r#"
function on_match_quality() return 0.9 end
function on_accept_match() return true end
"#,
        );
        let result = ScriptLoader::validate(path.to_str().unwrap());
        assert!(result.valid);
        assert!(result.error.is_none());
        assert!(
            result
                .defined_hooks
                .contains(&"on_match_quality".to_string())
        );
        assert!(
            result
                .defined_hooks
                .contains(&"on_accept_match".to_string())
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_invalid_script() {
        let path = write_temp_lua("syntax error {{{");
        let result = ScriptLoader::validate(path.to_str().unwrap());
        assert!(!result.valid);
        assert!(result.error.is_some());
        assert!(result.defined_hooks.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_missing_file() {
        let result = ScriptLoader::validate("/nonexistent/script.lua");
        assert!(!result.valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn validate_batch_mixed() {
        let valid_path = write_temp_lua("function on_match_quality() return 0.9 end");
        let invalid_path = write_temp_lua("bad syntax");

        let results = ScriptLoader::validate_batch(&[
            valid_path.to_str().unwrap(),
            invalid_path.to_str().unwrap(),
        ]);

        assert_eq!(results.len(), 2);
        assert!(results[0].valid);
        assert!(!results[1].valid);

        let _ = std::fs::remove_file(&valid_path);
        let _ = std::fs::remove_file(&invalid_path);
    }
}
