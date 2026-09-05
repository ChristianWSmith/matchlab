//! matchlab-lua: shared Lua-native system foundation.
//!
//! This crate is the single place where a Lua script becomes a pluggable
//! algorithm. It provides:
//!
//! - [`vm::LuaVm`] — load a script, inject `config`, call functions, read
//!   globals, and thread an opaque [`Context`] through every call.
//! - [`context`] — the arbitrary, script-defined data a Rust model persists
//!   across calls (an ordered `serde_yaml::Value`).
//! - [`rng`] — deterministic randomness: `matchlab.rng_*` helpers draw from the
//!   in-flight `&mut SimRng`; `math.random` is banned in scripts.
//! - [`convert`] — core type marshalling (observations, match results, metric
//!   snapshots).
//! - [`validate`] — required-function presence and the `math.random` ban.
//! - [`resolve`] — workspace-root path resolution for `plugins/...` paths.
//!
//! Dependency flow: `matchlab-core` ← `matchlab-lua` ← algorithm crates.

pub mod context;
pub mod convert;
pub mod resolve;
pub mod rng;
pub mod validate;
pub mod vm;

pub use context::Context;
pub use vm::LuaVm;
