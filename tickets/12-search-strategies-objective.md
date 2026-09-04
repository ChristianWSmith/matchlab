# Ticket 12: Search Strategies + Match Objective

## Context
Implement the `SearchStrategy` trait and `MatchObjective` scoring system for matchmaking optimization (§7.4-7.5).

## Scope
- Create `crates/matchlab-matchmaking/src/search.rs` — `SearchStrategy` trait + implementations
- Create `crates/matchlab-matchmaking/src/objective.rs` — `MatchObjective` scoring

## SearchStrategy Trait
```rust
pub trait SearchStrategy: Send + Sync {
    fn search(
        &self,
        queue: &[QueueEntry],
        objective: &MatchObjective,
        team_size: usize,
        world: &World,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch>;
}
```

## Implementations (start with 3, stub the rest)
1. **Greedy** — For each entry, find best teammates/opponents by objective score
2. **RandomSampling** — Generate N random valid compositions, return best
3. **BeamSearch** — Maintain beam of K partial assignments, expand, keep top K

### Stubs (for later):
- NearestNeighbor
- HungarianAssignment
- GeneticAlgorithm
- IntegerProgramming
- SimulatedAnnealing

## MatchObjective
```rust
pub struct MatchObjective {
    pub weight_quality: f64,
    pub weight_queue_time: f64,
    pub weight_ping: f64,
    pub weight_rating_uncertainty: f64,
}

impl MatchObjective {
    pub fn score(&self, proposed: &ProposedMatch, queue_entries: &[QueueEntry], world: &World) -> f64;
    fn match_quality(&self, proposed: &ProposedMatch, world: &World) -> f64;
    fn queue_time_cost(&self, proposed: &ProposedMatch, queue_entries: &[QueueEntry], world: &World) -> f64;
    fn ping_cost(&self, _proposed: &ProposedMatch, _world: &World) -> f64; // placeholder
    fn rating_uncertainty_cost(&self, proposed: &ProposedMatch, world: &World) -> f64;
}
```

## Scoring
```
Score = w_quality × Q - w_queue × T - w_ping × P - w_uncertainty × R
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-matchmaking` succeeds
- [ ] `cargo test -p matchlab-matchmaking` passes
- [ ] `Greedy` strategy produces valid matches
- [ ] `RandomSampling` returns best of N samples
- [ ] `BeamSearch` maintains beam width correctly
- [ ] `MatchObjective::score` computes weighted combination correctly
- [ ] `ping_cost` returns 0.0 (placeholder, as specified)

## Testing
- Unit test: `Greedy` forms teams from available queue entries
- Unit test: `RandomSampling` with N=1 returns single sample
- Unit test: `RandomSampling` with N=10 returns best of 10
- Unit test: `BeamSearch` with width=5 never exceeds 5 partial assignments
- Unit test: `MatchObjective::score` with all weights = 1.0
- Unit test: `MatchObjective::score` with zero weights → 0.0
- Unit test: `queue_time_cost` increases with longer waits

## Dependencies
- `matchlab-core`
- `matchlab-matchmaking` (existing `queue.rs`, `matchmaker.rs`)
