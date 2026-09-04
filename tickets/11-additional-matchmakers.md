# Ticket 11: Additional Matchmakers — ExpandingWindow, Strict, HubSpoke

## Context
Implement the three remaining matchmaker types specified in §7. Currently only `BatchMatchmaker` exists.

## Scope
- Create `crates/matchlab-matchmaking/src/expanding.rs` — `ExpandingWindowMatchmaker`
- Create `crates/matchlab-matchmaking/src/strict.rs` — `StrictMatchmaker`
- Create `crates/matchlab-matchmaking/src/hub_spoke.rs` — `HubSpokeMatchmaker`
- Update `crates/matchlab-matchmaking/src/lib.rs` — add module declarations
- Update `crates/matchlab-matchmaking/src/matchmaker.rs` — add `ProposedMatch::match_quality` if not already present
- Add Lua hook integration to each matchmaker

## ExpandingWindowMatchmaker
- Tiered skill windows: `[(max_secs, allowed_diff)]`
- Players who wait longer get wider acceptable skill ranges
- First matching tier wins; fallback to `max_window`
- Default tiers: `[(5.0, 25.0), (10.0, 50.0), (20.0, 100.0), (30.0, 200.0)]`

## StrictMatchmaker
- Hard `max_skill_diff` cap
- Players outside the window are never matched (may wait indefinitely)
- Intended behavior: quality over speed

## HubSpokeMatchmaker
- Partitions queue by region
- Each region has a sub-matchmaker (spoke)
- If a spoke exceeds `spoke_capacity`, overflow is handled by the hub (batch greedy)
- Models regional matchmaking with overflow handling

## Acceptance Criteria
- [ ] `cargo build -p matchlab-matchmaking` succeeds
- [ ] `cargo test -p matchlab-matchmaking` passes
- [ ] `ExpandingWindowMatchmaker` widens windows with wait time
- [ ] `StrictMatchmaker` rejects matches exceeding skill diff
- [ ] `HubSpokeMatchmaker` partitions by region and handles overflow
- [ ] All three implement `Matchmaker` trait
- [ ] Lua hooks integrate with each matchmaker's quality/accept logic

## Testing
- Unit test: `ExpandingWindowMatchmaker` uses wider window for longer waits
- Unit test: `ExpandingWindowMatchmaker` falls back to max_window
- Unit test: `StrictMatchmaker` rejects match with diff > max_skill_diff
- Unit test: `StrictMatchmaker` accepts match with diff <= max_skill_diff
- Unit test: `HubSpokeMatchmaker` partitions queue by region
- Unit test: `HubSpokeMatchmaker` handles overflow when spoke exceeds capacity
- Integration test: each matchmaker forms valid `ProposedMatch` structs

## Dependencies
- `matchlab-core`
- `matchlab-matchmaking` (existing `queue.rs`, `matchmaker.rs`, `constraint.rs`)
