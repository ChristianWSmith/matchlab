# Ticket 15: Add Missing Events to matchlab-core

## Context
Add three missing event types to the core event system as specified in §4.2. The spec's `EventKind` enum includes `MatchStart`, `RatingUpdate`, and `DetectionCheck` which are not yet implemented.

## Scope
- Update `crates/matchlab-core/src/event.rs`:
  - Add `MatchStart`, `RatingUpdate`, `DetectionCheck` to `EventKind` enum
  - Add `MatchStartEvent` struct
  - Add `RatingUpdateEvent` struct
  - Add `DetectionCheckEvent` struct
  - Implement `Event` trait for each

## New Event Types

### MatchStartEvent
```rust
#[derive(Debug)]
pub struct MatchStartEvent {
    pub time: SimTime,
    pub match_id: MatchId,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
}
```
Fired when a formed match begins play. Allows outcome models to track match start separately from formation.

### RatingUpdateEvent
```rust
#[derive(Debug)]
pub struct RatingUpdateEvent {
    pub time: SimTime,
    pub match_id: MatchId,
    pub players: Vec<PlayerId>,
}
```
Fired after rating system updates are applied. Allows detection systems and metrics to observe rating changes.

### DetectionCheckEvent
```rust
#[derive(Debug)]
pub struct DetectionCheckEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}
```
Periodic event to trigger detection system evaluation for a specific player.

## Acceptance Criteria
- [ ] `cargo build -p matchlab-core` succeeds
- [ ] `cargo test -p matchlab-core` passes
- [ ] `EventKind` has 16 variants (was 13)
- [ ] Each new event implements `Event` trait correctly
- [ ] `downcast::<T>()` works for all new event types
- [ ] Event ordering is correct in `TimestampedEvent` heap

## Testing
- Unit test: `MatchStartEvent::kind()` returns `EventKind::MatchStart`
- Unit test: `RatingUpdateEvent::kind()` returns `EventKind::RatingUpdate`
- Unit test: `DetectionCheckEvent::kind()` returns `EventKind::DetectionCheck`
- Unit test: `downcast::<MatchStartEvent>()` returns correct type
- Unit test: `TimestampedEvent` ordering with new events

## Dependencies
- `matchlab-core` (existing `event.rs`)
