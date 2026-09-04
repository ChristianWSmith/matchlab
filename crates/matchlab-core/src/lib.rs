//! matchlab-core: simulation engine, time, events, world, RNG, core types.
//!
//! This crate holds the building blocks shared by every higher layer: the
//! discrete-event engine, the simulation clock (SimTime), the World state
//! container, deterministic RNG, and the player/match data structures.
//!
//! Core types are implemented incrementally per the v0.1 build order in
//! `docs/spec.md` section 17.

pub mod match_;
pub mod player;
pub mod rng;
pub mod time;
