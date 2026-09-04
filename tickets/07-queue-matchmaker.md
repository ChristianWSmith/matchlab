# Ticket 07 — Queue + Batch Matchmaker (v0.1 Build Order Step 6)

## Goal

Create `crates/matchlab-matchmaking` with the `Queue` data structure and the
`BatchMatchmaker` used by v0.1, plus the `Matchmaker` trait and the
`ProposedMatch` type.

## Scope / Deliverables

- `crates/matchlab-matchmaking/` depending on `matchlab-core`.
  - `queue.rs` — `QueueEntry` (player_id, joined_at, observation, region,
    party_id, game_mode, role, latency_ms) and `Queue` with `enqueue`, `remove`,
    `remove_batch`, `waiting_time`, `entries`, `len`, `is_empty`,
    `from_entries` (spec §7.1).
  - `matchmaker.rs` — `Matchmaker` trait
    `find_matches(queue, world, team_size, now, rng) -> Vec<ProposedMatch>`,
    and `ProposedMatch { team_a, team_b, quality_score }` (spec §7.2).
    Add the `match_quality` helper (spec §7.8):
    `1.0 - (|avg_a - avg_b| / 400.0).clamp(0,1)` computed from **observations**
    only.
  - `batch.rs` — `BatchMatchmaker { interval_ticks, constraints }`
    (spec §7.8). For v0.1 the constraint list may be empty; implement the
    FIFO-candidate greedy formation described in spec §7.8 (sort by `joined_at`,
    fill team A then team B, emit matches when both teams full and constraints
    satisfied). The `interval_ticks` field is metadata used by the event handler
    (Ticket 08) to decide *when* to run the matchmaker.
  - The `Constraint` trait and at least `SkillBalanceConstraint`/others are
    **out of scope** for v0.1 unless trivially cheap; keep `constraints` as an
    empty `Vec` and only build the trait if time permits. Prefer the minimal
    batch implementation.
- `lib.rs` re-exports.

## Acceptance criteria

- [ ] Enqueue/remove/waiting-time work correctly; `waiting_time` measures time
      since `joined_at` (this is the basis of the v0.1 queue-time metric).
- [ ] Filling a queue with ≥ 2×`team_size` players yields at least one
      `ProposedMatch` with two full teams.
- [ ] Batch matchmaker is FIFO: longest-waiting players are matched first.
- [ ] `match_quality` is computed only from observations (never reality).
- [ ] Deterministic for a given queue + seed.

## Testing

- Queue: enqueue several entries, verify ordering, `waiting_time` advances with
  a later `now`, `remove`/`remove_batch` behavior.
- Matchmaker: queue of 10 players, `team_size=5` → exactly one proposed match
  with 5v5 using the first 10 entries FIFO.
- Matchmaker: fewer than 2×`team_size` → no matches.
- `match_quality`: equal teams → 1.0; lopsided → approaches 0.

## Dependencies

Tickets 02 (types), 04 (observations available from population).

## Notes

- Spec references: §7.1, §7.2, §7.8, §17 Step 6.
- v0.1 uses the **batch** algorithm (per the minimal manifest in spec §17).
  ExpandingWindow/Strict/HubSpoke are **out of scope** for v0.1.
- `BatchMatchmaker.interval_ticks` controls how often the handler triggers
  matchmaking; the handler (Ticket 08) reads it to schedule `MatchFormed`
  events.
