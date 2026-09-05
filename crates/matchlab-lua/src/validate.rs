//! Script validation: parse, check required functions, and enforce the
//! `math.random` ban.

use mlua::Lua;

/// What a validation run found.
#[derive(Debug)]
pub struct ValidationReport {
    pub defined_functions: Vec<String>,
}

/// Validate a script for a given contract.
///
/// Rejects the script when:
/// - it cannot be read / parsed / executed,
/// - its source contains `math.random` (determinism rule — all randomness must
///   flow through `matchlab.rng_*`), or
/// - any name in `required` is not a global function.
pub fn validate_script(path: &str, required: &[&str]) -> Result<ValidationReport, String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;

    if source.contains("math.random") {
        return Err(format!(
            "script {} uses banned math.random; use matchlab.rng_* instead",
            path
        ));
    }

    let lua = Lua::new();
    lua.load(&source)
        .exec()
        .map_err(|e| format!("lua error in {}: {}", path, e))?;

    let globals = lua.globals();
    let mut defined = Vec::new();
    for name in required {
        if globals.get::<mlua::Function>(*name).is_ok() {
            defined.push(name.to_string());
        } else {
            return Err(format!(
                "script {} is missing required function {}",
                path, name
            ));
        }
    }

    Ok(ValidationReport {
        defined_functions: defined,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp(content: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "matchlab_lua_validate_{}_{}.lua",
            std::process::id(),
            n
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn valid_script_with_required_functions() {
        let p = write_temp("function foo(a) return a end\nfunction bar() return 1 end");
        let report = validate_script(p.to_str().unwrap(), &["foo", "bar"]).unwrap();
        assert_eq!(report.defined_functions, vec!["foo", "bar"]);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_required_function_is_rejected() {
        let p = write_temp("function foo() return 1 end");
        let err = validate_script(p.to_str().unwrap(), &["foo", "bar"]).unwrap_err();
        assert!(
            err.contains("bar"),
            "error should name the missing function: {err}"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn math_random_is_rejected() {
        let p = write_temp("function foo() return math.random() end");
        let err = validate_script(p.to_str().unwrap(), &["foo"]).unwrap_err();
        assert!(
            err.contains("math.random"),
            "error should name the ban: {err}"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn invalid_lua_is_rejected() {
        let p = write_temp("this is not lua {{{");
        assert!(validate_script(p.to_str().unwrap(), &["foo"]).is_err());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_rejected() {
        assert!(validate_script("/nonexistent/script.lua", &["foo"]).is_err());
    }
}
