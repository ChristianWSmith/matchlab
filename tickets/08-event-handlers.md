# Ticket 08 — Event Handlers: End-to-End Machine Loop (v0.1 Build Order Step 7)

## Goal

Wire the core, population, game, rating, and matchmaking pieces together with
event handlers so a full simulation loop runs: PlayerJoin → PlayerQueue →
(periodic) matching → MatchFormed → MatchEnd → RatingUpdate → metrics.

## Scope / Deliverables

This ticket lives at the composition layer. Add a crate (or module) that owns
the handlers. Concretely:

- Register handlers on the `EventEngine` for:
  - `PlayerJoin` → `PlayerJoinEvent`: add player's reality+observation to
    `World`, schedule a `PlayerQueue` event (and mark `queue_joined_at`).
  - `PlayerQueue` → `PlayerQueueEvent`: add the player to the `Queue`.
  - Match-formation trigger: at a cadence driven by `BatchMatchmaker.interval_ticks`
    (or on a `MatchFormed`/dedicated tick), call the matchmaker's
    `find_matches`; for each `ProposedMatch`, create a `MatchFormedEvent`
    (assigning `MatchId`, both team player-id lists) and schedule it.
  - `MatchFormed` → `MatchFormedEvent`: fetch the two teams' observations from
    `World`, call `LogisticOutcomeModel::simulate` to produce a `MatchResult`,
    record the match into `World.matches`, schedule a `MatchEndEvent` at
    `now + duration`.
  - `MatchEnd` → `MatchEndEvent`: run the active `RatingSystem::update` on the
    `MatchResult`, apply the returned `RatingState`s back to `World.observations`
    (preserving truth separation — rating only touches observations),
    then hand the match to the metrics engine (Ticket 09) and re-queue /
    schedule the players' next `PlayerQueue` per their play behavior.
- Manage the currently-active rating system: v0.1 selects **one** rating system
  per experiment (Elo at first). Hold it where the handlers can reach it.
- Ensure the event loop is deterministic (same seed ⇒ same schedule).
- Prove the end-to-end loop with an integration test (spec §17 Step 7 exit
  criterion: 100-player simulation end-to-end).

## Acceptance criteria

- [ ] A 100-player simulation runs end-to-end without deadlock or panic,
      producing a non-zero number of completed matches.
- [ ] Players flow `Join → Queue → Formed → Match → RatingUpdate`.
- [ ] Ratings in `World.observations` change after `MatchEnd` per the rating
      system.
- [ ] Truth separation is preserved: matchmaking, game simulation, and rating
      never read `World.players` directly.
- [ ] Deterministic: identical seed and config produce identical output.
- [ ] The loop terminates (bounded by `duration.matches` / `max_time`, enforced
      here and/or by the runner in Ticket 10).

## Testing

- Integration test: generate 100 players, run the loop for a bounded number of
  matches, assert matches complete and ratings/games_played advance.
- Unit-style handler tests: feed a `PlayerJoinEvent` and assert a
  `PlayerQueue` event is scheduled; feed a `MatchFormed` and assert a
  `MatchEnd` is scheduled after match duration.

## Dependencies

Tickets 02, 03 (engine), 04 (population), 05 (game), 06 (rating), 07
(matchmaker).

## Notes

- Spec references: §17 Step 7; spec §4.4 (handlers), §4.7 (Simulation).
- The match-formation cadence: the minimal manifest sets `batch_interval: 10`.
  Use a scheduled timer/tick event to invoke the batch matchmaker at that
  interval rather than hooking every player queue.
- Where the handlers live can be a new small crate or a module inside an
  existing composition crate (e.g. `matchlab-experiments` in Ticket 10). Keep
  the dependency direction clean — if you create a dedicated crate, it must
  depend on core + players + game + rating + matchmaking + metrics (Ticket 09).
- After a `MatchEnd`, decide re-queue behavior: for v0.1, static population
  players re-queue after a short delay (or immediately); exact cadence can be a
  constant, but keep it deterministic.
