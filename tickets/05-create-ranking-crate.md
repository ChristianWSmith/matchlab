# Ticket 05: Create matchlab-ranking Crate

## Context
Create the ranking crate from scratch. Implements rank mapping from ratings to visible tiers/divisions and a leaderboard system as specified in §10.

## Scope
- Create `crates/matchlab-ranking/Cargo.toml` (deps: `matchlab-core`, `serde`)
- Create `crates/matchlab-ranking/src/lib.rs` — re-exports
- Create `crates/matchlab-ranking/src/ranker.rs` — `RankMapper` trait, `Rank`, `BracketRankMapper`
- Create `crates/matchlab-ranking/src/leaderboard.rs` — `Leaderboard`, `LeaderboardEntry`

## Types

### RankMapper Trait
```rust
pub trait RankMapper: Send + Sync {
    fn rating_to_rank(&self, rating: f64) -> Rank;
    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64);
}
```

### Rank
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Rank {
    pub tier: String,
    pub division: u8,
}
```

### BracketRankMapper
```rust
pub struct BracketRankMapper {
    pub brackets: Vec<RankBracket>,
}

pub struct RankBracket {
    pub rank: Rank,
    pub min: f64,
    pub max: f64,
}
```

### Leaderboard
```rust
pub struct Leaderboard {
    entries: Vec<LeaderboardEntry>,
}

pub struct LeaderboardEntry {
    pub player_id: PlayerId,
    pub rating: f64,
    pub rank: Rank,
    pub games_played: u64,
}
```

## Acceptance Criteria
- [ ] `cargo build -p matchlab-ranking` succeeds
- [ ] `cargo test -p matchlab-ranking` passes
- [ ] `BracketRankMapper` correctly maps ratings to rank tiers
- [ ] `Leaderboard` maintains sorted order by rating
- [ ] `rank_of()` returns correct position for a player
- [ ] `top_n()` returns correct slice

## Testing
- Unit test: `BracketRankMapper::rating_to_rank` with known brackets
- Unit test: `BracketRankMapper::rank_to_rating_range` returns correct bounds
- Unit test: `Leaderboard::update` inserts and re-sorts
- Unit test: `Leaderboard::rank_of` returns correct index
- Unit test: `Leaderboard::top_n` with n > entries.len()

## Dependencies
- `matchlab-core`
- `serde` (workspace)
