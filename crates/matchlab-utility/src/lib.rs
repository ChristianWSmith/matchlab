//! matchlab-utility: Lua-native player satisfaction / retention model.
//!
//! Models player retention probability from observable experience proxies
//! (§16). `PlayerExperience` (loop-maintained data) stays in Rust; the
//! `SatisfactionModel` trait is implemented by a Lua script
//! (`plugins/utility/satisfaction.lua`).

pub mod lua;
pub mod satisfaction;

pub use lua::LuaSatisfactionModel;
pub use satisfaction::{PlayerExperience, SatisfactionModel};
