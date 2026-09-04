//! matchlab-matchmaking: queue, matchmaker, constraints, and search strategies.
//!
//! v0.1 implements the `Queue` (spec §7.1), the `Matchmaker` trait and
//! `ProposedMatch` (spec §7.2), the `Constraint` trait (spec §7.3, no concrete
//! constraints yet), and the `BatchMatchmaker` (spec §7.8). The other
//! matchmakers (ExpandingWindow, Strict, HubSpoke) are out of scope for v0.1.

pub mod batch;
pub mod constraint;
pub mod matchmaker;
pub mod queue;
