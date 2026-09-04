//! matchlab-players: archetypes, population generation, and the skill process.
//!
//! Generates synthetic player populations with known ground truth (spec §5.7,
//! §5.8). v0.1 uses a single `stable` archetype and static skill, but the
//! config schema is general so richer archetypes can be added via YAML without
//! code changes.

pub mod archetype;
pub mod population;
pub mod skill;
