//! Deterministic randomness for Lua scripts.
//!
//! Lua scripts never call `math.random` (banned at load). Instead they draw
//! from the simulation's `SimRng` via the registered `matchlab.rng_*` helpers.
//! The in-flight `&mut SimRng` is routed through a thread-local slot, set and
//! cleared around every guarded Lua call by [`with_active`].

use matchlab_core::rng::SimRng;
use mlua::{Lua, Table};
use std::cell::RefCell;

thread_local! {
    static ACTIVE: RefCell<Option<*mut SimRng>> = const { RefCell::new(None) };
}

/// Run `f` with `rng` made available to `matchlab.rng_*`. The slot is always
/// cleared on return, including across panics.
///
/// # Safety
///
/// The raw pointer is only dereferenced while the slot holds it, and the slot
/// is owned by this thread; the simulation is single-threaded and the guarded
/// region never outlives the `&mut SimRng` borrow.
pub fn with_active<R>(rng: &mut SimRng, f: impl FnOnce() -> R) -> R {
    let ptr = rng as *mut SimRng;
    ACTIVE.with(|slot| *slot.borrow_mut() = Some(ptr));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    ACTIVE.with(|slot| *slot.borrow_mut() = None);
    match result {
        Ok(v) => v,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn with_rng_mut<R>(f: impl FnOnce(&mut SimRng) -> R) -> Result<R, String> {
    ACTIVE.with(|slot| {
        let borrowed = slot.borrow();
        match *borrowed {
            Some(ptr) => {
                // SAFETY: see `with_active`; the slot is only live inside the
                // guarded region and the borrow of `rng` is exclusive.
                let rng = unsafe { &mut *ptr };
                Ok(f(rng))
            }
            None => Err("matchlab.rng_* called outside a guarded region (with_rng)".to_string()),
        }
    })
}

/// Register the `matchlab.rng_*` globals on a Lua state.
pub fn register(lua: &Lua) -> Result<(), String> {
    let matchlab: Table = lua
        .create_table()
        .map_err(|e| format!("create matchlab table: {e}"))?;

    let rng_range = lua
        .create_function(|_, (low, high): (f64, f64)| {
            with_rng_mut(|r| r.gen_range(low, high)).map_err(runtime_error)
        })
        .map_err(|e| format!("register rng_range: {e}"))?;
    matchlab
        .set("rng_range", rng_range)
        .map_err(|e| format!("set rng_range: {e}"))?;

    let rng_bool = lua
        .create_function(|_, p: f64| with_rng_mut(|r| r.gen_bool(p)).map_err(runtime_error))
        .map_err(|e| format!("register rng_bool: {e}"))?;
    matchlab
        .set("rng_bool", rng_bool)
        .map_err(|e| format!("set rng_bool: {e}"))?;

    let rng_normal = lua
        .create_function(|_, (mean, stddev): (f64, f64)| {
            with_rng_mut(|r| r.sample_normal(mean, stddev)).map_err(runtime_error)
        })
        .map_err(|e| format!("register rng_normal: {e}"))?;
    matchlab
        .set("rng_normal", rng_normal)
        .map_err(|e| format!("set rng_normal: {e}"))?;

    let rng_u64 = lua
        .create_function(|_, ()| with_rng_mut(|r| r.gen_u64()).map_err(runtime_error))
        .map_err(|e| format!("register rng_u64: {e}"))?;
    matchlab
        .set("rng_u64", rng_u64)
        .map_err(|e| format!("set rng_u64: {e}"))?;

    lua.globals()
        .set("matchlab", matchlab)
        .map_err(|e| format!("set matchlab global: {e}"))
}

fn runtime_error(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::rng::SimRng;
    use mlua::Lua;

    #[test]
    fn rng_helpers_are_registered() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let matchlab: Table = lua.globals().get("matchlab").unwrap();
        assert!(matchlab.get::<mlua::Function>("rng_range").is_ok());
        assert!(matchlab.get::<mlua::Function>("rng_bool").is_ok());
        assert!(matchlab.get::<mlua::Function>("rng_normal").is_ok());
        assert!(matchlab.get::<mlua::Function>("rng_u64").is_ok());
    }

    #[test]
    fn draws_consume_simrng_identically() {
        // Lua drawing through matchlab.rng_* must consume the exact same SimRng
        // sequence as a pure-Rust reference.
        let mut rust_rng = SimRng::from_seed(42);
        let reference: Vec<f64> = (0..10).map(|_| rust_rng.gen_range(0.0, 1.0)).collect();

        let lua = Lua::new();
        register(&lua).unwrap();
        let script = r#"
            local out = {}
            for i = 1, 10 do
                out[i] = matchlab.rng_range(0.0, 1.0)
            end
            return out
        "#;
        let f: mlua::Function = lua.load(script).into_function().unwrap();

        let mut sim_rng = SimRng::from_seed(42);
        let drawn: Vec<f64> = with_active(&mut sim_rng, || f.call(()).unwrap());
        assert_eq!(drawn, reference, "Lua draws must match Rust draws");
    }

    #[test]
    fn interleaved_draws_match_sequence() {
        let mut rng_a = SimRng::from_seed(7);
        let mut rng_b = SimRng::from_seed(7);

        let lua = Lua::new();
        register(&lua).unwrap();
        let script = r#"
            return matchlab.rng_range(0.0, 100.0), matchlab.rng_bool(0.5)
        "#;
        let f: mlua::Function = lua.load(script).into_function().unwrap();

        // Interleave: one Lua draw pair + one Rust draw pair, repeated.
        let mut lua_vals = Vec::new();
        let mut rust_vals = Vec::new();
        for _ in 0..5 {
            let (a, b): (f64, bool) = with_active(&mut rng_a, || f.call(()).unwrap());
            lua_vals.push(a);
            lua_vals.push(if b { 1.0 } else { 0.0 });
            rust_vals.push(rng_b.gen_range(0.0, 100.0));
            rust_vals.push(if rng_b.gen_bool(0.5) { 1.0 } else { 0.0 });
        }
        assert_eq!(lua_vals, rust_vals);
    }

    #[test]
    fn rng_outside_guard_errors() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let script = "return matchlab.rng_range(0.0, 1.0)";
        let f: mlua::Function = lua.load(script).into_function().unwrap();
        // Calling without a guarded SimRng must fail, not panic or return junk.
        assert!(f.call::<f64>(()).is_err());
    }

    #[test]
    fn slot_is_cleared_even_on_panic() {
        let mut rng = SimRng::from_seed(1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_active(&mut rng, || {
                panic!("inside guarded region");
            })
        }));
        assert!(result.is_err());
        // Slot cleared -> rng helpers error instead of using a stale pointer.
        let lua = Lua::new();
        register(&lua).unwrap();
        let script = "return matchlab.rng_range(0.0, 1.0)";
        let f: mlua::Function = lua.load(script).into_function().unwrap();
        assert!(f.call::<f64>(()).is_err());
    }
}
