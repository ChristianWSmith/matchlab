# Ticket 07: Create matchlab-adversarial Crate

## Context
Create the adversarial agents crate. Implements active agents that exploit or manipulate the rating system (§15).

## Scope
- Create `crates/matchlab-adversarial/Cargo.toml` (deps: `matchlab-core`, `serde`)
- Create `crates/matchlab-adversarial/src/lib.rs` — re-exports
- Create `crates/matchlab-adversarial/src/agent.rs` — `AdversarialAgent` trait, `AdversarialObjective` enum
- Create `crates/matchlab-adversarial/src/booster.rs` — `BoosterAgent`
- Create `crates/matchlab-adversarial/src/deranker.rs` — `DerankerAgent`
- Create `crates/matchlab-adversarial/src/win_trader.rs` — `WinTraderAgent`
- Create `crates/matchlab-adversarial/src/afk.rs` — `AfkAgent`
- Create `crates/matchlab-adversarial/src/rating_farmer.rs` — `RatingFarmerAgent`

## Types

### AdversarialAgent Trait
```rust
pub trait AdversarialAgent: Send + Sync {
    fn tick(&mut self, player_id: PlayerId, world: &mut World);
    fn objective(&self) -> AdversarialObjective;
}
```

### AdversarialObjective
```rust
pub enum AdversarialObjective {
    MaximizeRating,
    MinimizeGamesPlayed,
    MaximizeWinRate { target_games: u64 },
    MaintainLowRating,
    WinTrade { partner: PlayerId },
    Derate,
}
```

### Agent Implementations
- **BoosterAgent**: Two players (booster + boostee). Booster queues with boostee, intentionally underperforms to lower own rating while boostee wins.
- **DerankerAgent**: Intentionally loses matches to drop rating. May AFK, disconnect, or throw.
- **WinTraderAgent**: Two partners alternate wins to maintain rating while farming games.
- **AfkAgent**: Randomly goes AFK or disconnects during matches based on probability.
- **RatingFarmerAgent**: Queues, then immediately quits/disconnects after starting. Keeps games_played minimal for smurf-like reset.

## Acceptance Criteria
- [ ] `cargo build -p matchlab-adversarial` succeeds
- [ ] `cargo test -p matchlab-adversarial` passes
- [ ] Each agent's `tick()` modifies world state consistent with its objective
- [ ] `objective()` returns correct enum variant for each agent type
- [ ] Agents only modify observable state (truth separation preserved)

## Testing
- Unit test: `BoosterAgent::tick` reduces booster's performance metrics
- Unit test: `DerankerAgent::tick` increases disconnect/quit probability
- Unit test: `WinTraderAgent::tick` alternates win/loss pattern
- Unit test: `AfkAgent::tick` triggers AFK based on probability
- Unit test: `RatingFarmerAgent::tick` triggers quit after match start
- Unit test: Each agent's `objective()` returns correct variant

## Dependencies
- `matchlab-core`
- `serde` (workspace)
