//! `LuaVm` — the shared runtime for a Lua-native system.
//!
//! Loads a script, stores its `config` (from YAML params), registers the
//! `matchlab.rng_*` helpers, and provides context-threading call helpers.
//!
//! Call convention: every script function receives its layer-specific inputs,
//! then `config`, then `context` as the final argument. The script may return
//! either a single value (context read back by reference after the call) or
//! `(value, context)` where the second value replaces the stored context.

use crate::context::{self, Context};
use crate::rng;
use crate::validate;
use matchlab_core::rng::SimRng;
use mlua::{Function, FromLua, Lua, Table, Value};
use std::sync::Mutex;

/// Global under which the persistent context table is stored.
const CONTEXT_GLOBAL: &str = "_matchlab_context";

/// A loaded Lua script with its config and deterministic helpers.
pub struct LuaVm {
    lua: Mutex<Lua>,
    script_path: String,
    config: serde_yaml::Value,
}

impl LuaVm {
    /// Load and execute a script, storing `params` as its `config`.
    ///
    /// The path is resolved against the workspace root; `math.random` is
    /// banned. Optionally validates a required-function list up front.
    pub fn load(path: &str, params: &serde_yaml::Value, required: &[&str]) -> Result<Self, String> {
        let resolved = crate::resolve::resolve_script_path(path);
        let resolved_str = resolved.to_string_lossy().to_string();
        validate::validate_script(&resolved_str, required)?;

        let source = std::fs::read_to_string(&resolved_str)
            .map_err(|e| format!("cannot read {}: {}", resolved_str, e))?;
        let lua = Lua::new();
        lua.load(&source)
            .exec()
            .map_err(|e| format!("lua error in {}: {}", resolved_str, e))?;
        rng::register(&lua)?;

        Ok(Self {
            lua: Mutex::new(lua),
            script_path: resolved_str,
            config: if params.is_null() {
                context::empty()
            } else {
                params.clone()
            },
        })
    }

    pub fn script_path(&self) -> &str {
        &self.script_path
    }

    pub fn config(&self) -> &serde_yaml::Value {
        &self.config
    }

    /// Run `f` with the given `&mut SimRng` available to `matchlab.rng_*`.
    pub fn with_rng<T>(&self, rng: &mut SimRng, f: impl FnOnce(&Self) -> T) -> T {
        rng::with_active(rng, || f(self))
    }

    /// Build a Lua value with exclusive access to the underlying `Lua` state.
    /// Used by adapters to construct argument tables before a call.
    pub fn with_lua<R>(&self, f: impl FnOnce(&Lua) -> Result<R, String>) -> Result<R, String> {
        let lua = self
            .lua
            .lock()
            .map_err(|_| format!("lua mutex poisoned for {}", self.script_path))?;
        f(&lua)
    }

    /// Read a global from the loaded script (e.g. `information_budget`,
    /// `name`, `time_buckets`). `Ok(None)` when absent.
    pub fn get_global<T: mlua::FromLua>(&self, name: &str) -> Result<Option<T>, String> {
        let lua = self
            .lua
            .lock()
            .map_err(|_| format!("lua mutex poisoned for {}", self.script_path))?;
        let value: Value = lua.globals().get(name).map_err(|e| e.to_string())?;
        if matches!(value, Value::Nil) {
            return Ok(None);
        }
        T::from_lua(value, &lua)
            .map(Some)
            .map_err(|e| format!("global {name}: {e}"))
    }

    /// Call `name` with `args ++ [config, context]`.
    ///
    /// The context is a persistent Lua table stored as the `_matchlab_context`
    /// global (created empty on first use) and passed by reference to every
    /// call, so scripts accumulate state in place at O(1) per call. If the
    /// script returns a table as its second value, that table becomes the new
    /// context. This avoids round-tripping a growing accumulator through
    /// `serde_yaml` on every call.
    pub fn call_with_context<T: FromLua>(
        &self,
        name: &str,
        args: &[Value],
    ) -> Result<T, String> {
        let lua = self
            .lua
            .lock()
            .map_err(|_| format!("lua mutex poisoned for {}", self.script_path))?;
        let func: Function = lua
            .globals()
            .get(name)
            .map_err(|_| format!("{} not defined in {}", name, self.script_path))?;

        let ctx_table: Table = match lua.globals().get::<Value>(CONTEXT_GLOBAL).map_err(|e| e.to_string())? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table().map_err(|e| e.to_string())?;
                lua.globals()
                    .set(CONTEXT_GLOBAL, t.clone())
                    .map_err(|e| e.to_string())?;
                t
            }
        };

        let config_value = context::yaml_to_lua(&lua, &self.config)?;

        let mut call_args = args.to_vec();
        call_args.push(config_value);
        call_args.push(Value::Table(ctx_table.clone()));

        let results = func
            .call::<mlua::MultiValue>(mlua::MultiValue::from_vec(call_args))
            .map_err(|e| format!("{name} failed in {}: {}", self.script_path, e))?;

        let mut iter = results.into_vec().into_iter();
        let first = iter
            .next()
            .ok_or_else(|| format!("{name} in {} returned no value", self.script_path))?;
        let value: T = T::from_lua(first, &lua)
            .map_err(|e| format!("{name} result in {}: {}", self.script_path, e))?;

        if let Some(Value::Table(t)) = iter.next() {
            lua.globals()
                .set(CONTEXT_GLOBAL, t)
                .map_err(|e| e.to_string())?;
        }

        Ok(value)
    }

    /// Read the current context back as a serializable value (for inspection
    /// and tests). Returns an empty mapping when no call has run yet.
    pub fn read_context(&self) -> Result<Context, String> {
        let lua = self
            .lua
            .lock()
            .map_err(|_| format!("lua mutex poisoned for {}", self.script_path))?;
        let value: Value = lua.globals().get(CONTEXT_GLOBAL).map_err(|e| e.to_string())?;
        if matches!(value, Value::Nil) {
            return Ok(context::empty());
        }
        context::from_lua(&value)
    }

    /// Reset the context to an empty table (used by tests and re-entrant callers).
    pub fn reset_context(&self) -> Result<(), String> {
        let lua = self
            .lua
            .lock()
            .map_err(|_| format!("lua mutex poisoned for {}", self.script_path))?;
        let t = lua.create_table().map_err(|e| e.to_string())?;
        lua.globals()
            .set(CONTEXT_GLOBAL, t)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp(content: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("matchlab_lua_vm_{}_{}.lua", std::process::id(), n));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn params(map: &[(&str, f64)]) -> serde_yaml::Value {
        let mut m = serde_yaml::Mapping::new();
        for (k, v) in map {
            m.insert(
                serde_yaml::Value::String(k.to_string()),
                serde_yaml::Value::Number(serde_yaml::Number::from(*v)),
            );
        }
        serde_yaml::Value::Mapping(m)
    }

    #[test]
    fn load_injects_config_and_calls() {
        let p = write_temp(
            "function compute(x, config, context)\n  return x * config.factor, context\nend",
        );
        let vm = LuaVm::load(
            p.to_str().unwrap(),
            &params(&[("factor", 3.0)]),
            &["compute"],
        )
        .unwrap();
        let args = [Value::Integer(5)];
        let result: f64 = vm.call_with_context("compute", &args).unwrap();
        assert_eq!(result, 15.0);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn context_mutation_persists_across_calls() {
        let p = write_temp(
            "function bump(config, context)\n  context.count = (context.count or 0) + 1\n  return context.count\nend",
        );
        let vm = LuaVm::load(p.to_str().unwrap(), &params(&[]), &["bump"]).unwrap();
        for expected in 1..=3 {
            let count: i64 = vm.call_with_context("bump", &[]).unwrap();
            assert_eq!(count, expected);
        }
        let ctx = vm.read_context().unwrap();
        assert_eq!(ctx["count"].as_i64(), Some(3));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn returned_context_table_replaces_stored() {
        let p =
            write_temp("function fresh(config, context)\n  return 1, { value = config.n }\nend");
        let vm = LuaVm::load(p.to_str().unwrap(), &params(&[("n", 42.0)]), &["fresh"]).unwrap();
        let _: i64 = vm.call_with_context("fresh", &[]).unwrap();
        let ctx = vm.read_context().unwrap();
        let v = ctx.get("value").unwrap();
        assert_eq!(v.as_f64().unwrap(), 42.0);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_function_errors() {
        let p = write_temp("function other() return 1 end");
        let vm = LuaVm::load(p.to_str().unwrap(), &params(&[]), &[]).unwrap();
        let err = vm.call_with_context::<f64>("nope", &[]).unwrap_err();
        assert!(err.contains("nope"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn get_global_reads_script_globals() {
        let p = write_temp("information_budget = { \"WinLoss\" }\nfunction f() return 1 end");
        let vm = LuaVm::load(p.to_str().unwrap(), &params(&[]), &["f"]).unwrap();
        let budget: Option<Vec<String>> = vm.get_global("information_budget").unwrap();
        assert_eq!(budget, Some(vec!["WinLoss".to_string()]));
        assert!(
            vm.get_global::<f64>("nonexistent_global")
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_rejects_missing_required_function() {
        let p = write_temp("function a() return 1 end");
        assert!(LuaVm::load(p.to_str().unwrap(), &params(&[]), &["a", "b"]).is_err());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_rejects_math_random() {
        let p = write_temp("function a() return math.random() end");
        assert!(LuaVm::load(p.to_str().unwrap(), &params(&[]), &["a"]).is_err());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rng_is_available_inside_guarded_call() {
        let p = write_temp(
            "function draw(_, config, context)\n  return matchlab.rng_range(0.0, 100.0)\nend",
        );
        let vm = LuaVm::load(p.to_str().unwrap(), &params(&[]), &["draw"]).unwrap();
        let mut rng = SimRng::from_seed(42);
        let first: f64 = vm
            .with_rng(&mut rng, |vm| vm.call_with_context("draw", &[]).unwrap());
        let second: f64 = vm
            .with_rng(&mut rng, |vm| vm.call_with_context("draw", &[]).unwrap());
        assert!(first >= 0.0 && first < 100.0);
        assert_ne!(first, second);
        let _ = std::fs::remove_file(&p);
    }
}
