use matchlab_core::world::World;

use crate::matchmaker::ProposedMatch;

/// A predicate over a proposed match. v0.1 ships no concrete constraints (the
/// batch matchmaker runs with an empty list); later tickets can add them
/// behind this trait without changing the matchmaker.
pub trait Constraint: Send + Sync {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool;
}
