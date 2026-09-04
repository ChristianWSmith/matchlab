# Ticket 03 — Event Engine + World (v0.1 Build Order Step 2)

## Goal

Implement the discrete-event simulation engine in `matchlab-core`: the
priority-queue `EventEngine`, the `Event` trait / `EventKind` enum,
`TimestampedEvent`, the `World` state container, and the `Simulation` runner.

## Scope / Deliverables

- `event.rs`:
  - `Event` trait (`time()`, `kind()`) (spec §4.2)
  - `EventKind` enum — **all 11 variants** per spec §4.2:
    `PlayerJoin`, `PlayerLeave`, `PlayerQueue`, `PlayerQuit`, `PlayerReturn`,
    `PlayerDisconnect`, `MatchFormed`, `MatchStart`, `MatchEnd`,
    `RatingUpdate`, `DetectionCheck`, `SkillChange`
  - `TimestampedEvent` with min-heap ordering (reverse time cmp), `Eq`/`Ord`
  - Concrete event structs: `PlayerJoinEvent`, `PlayerLeaveEvent`,
    `PlayerQueueEvent`, `PlayerQuitEvent`, `PlayerReturnEvent`,
    `PlayerDisconnectEvent`, `MatchFormedEvent`, `MatchEndEvent`,
    `SkillChangeEvent` (spec §4.3)
  - `EventEngine`: `BinaryHeap<TimestampedEvent>`, handler map keyed by
    `EventKind`, methods `register_handler`, `schedule`, `next_event`,
    `peek_time`, `is_empty`, `tick` (spec §4.4)
- `world.rs`:
  - `World` holding `players` (reality), `observations`, `matches`, `rng`,
    `time`, and private `next_player_id`/`next_match_id` counters
  - `new(SimRng)`, `next_player_id()`, `next_match_id()`, `add_player(...)`
  - **Enforcement of truth separation**: expose `observe(pid)` returning
    `&PlayerObservation` for algorithms, and `reality(pid)` returning
    `&PlayerReality` marked/used only by simulation logic and metrics (spec §4.5).
  - Add an internal `MatchState` store so match lifecycle can be tracked.
- `simulation.rs` (or in `lib.rs`): `Simulation { world, engine }` with
  `new`, `run(until: SimTime)`, `run_to_completion()` (spec §4.7).
- `lib.rs` re-exports.

## Acceptance criteria

- [ ] Events execute in timestamp order; the engine skips idle time by jumping
      `world.time` to the next event's time (multi-scale time, spec §4.1).
- [ ] `Simulation::run(until)` stops at `until`; `run_to_completion` drains the
      queue.
- [ ] Handlers receive `&World` and return newly scheduled `Vec<Box<dyn Event>>`.
- [ ] Truth separation is structurally enforced: algorithms only reach players
      via `world.observe(pid)`; direct field access to `world.players` from
      algorithm crates is not possible through the public API.
- [ ] ID counters produce monotonic unique `PlayerId`/`MatchId`.

## Testing

- Schedule 3 events at different times and verify they run in increasing time
  order (spec §4 step 2 exit criterion).
- Verify `world.time` equals the processed event's time after each tick (idle
  skipping).
- Register a handler that emits a follow-up event and verify it is scheduled
  and eventually run.
- `run(until)` stops exactly at/before `until` and leaves later events queued.
- ID uniqueness/monotonicity test.

## Dependencies

Ticket 02 (core types).

## Notes

- Spec references: §4.1 (multi-scale time), §4.2–§4.4 (events/engine), §4.5
  (world), §4.7 (Simulation).
- `SimTime` is `u64` nanoseconds; the engine must not simulate every tick — it
  pops the next event and advances the clock.
- Keep `World.rng` accessible to simulation logic; it is advanced by game/match
  simulation, not by rating/matchmaking algorithms (those should use their own
  RNG or none).
