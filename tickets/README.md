# matchlab v0.1 — Ticket Plan

This directory holds the work tickets for implementing **v0.1** of matchlab, the
minimal viable simulation described in `docs/spec.md` section 17.

The project is currently at **pre-implementation**: the workspace `Cargo.toml`
exists as a single-package stub and `src/main.rs` is a `Hello, world!` binary.
No crates under `crates/` exist yet.

## How the tickets map to the build order

Each ticket maps to one step in the v0.1 build order (spec §17). Tickets are
numbered in dependency order and must be completed roughly in sequence, because
each layer depends on the layers below it.

| # | Ticket | Spec step | Layer(s) |
|---|--------|-----------|----------|
| 01 | Workspace Foundation | — | root + CI + scaffolding |
| 02 | Core Types | Step 1 | matchlab-core |
| 03 | Event Engine + World | Step 2 | matchlab-core |
| 04 | Player Population | Step 3 | matchlab-players |
| 05 | Game Outcome | Step 4 | matchlab-game |
| 06 | Rating Systems (Elo + Flat) | Step 5 | matchlab-rating |
| 07 | Queue + Matchmaker | Step 6 | matchlab-matchmaking |
| 08 | Event Handlers | Step 7 | cross-crate wiring |
| 09 | Metrics | Step 8 | matchlab-metrics |
| 10 | Config + Runner + CLI | Step 9 | matchlab-experiments + binary |
| 11 | Analysis + Output | Step 10 | matchlab-analysis |
| 12 | v0.1 Acceptance | — | end-to-end verification |

## Conventions to follow while implementing

- **Rust edition 2024**, workspace layout as in AGENTS.md/spec §3.
- **Truth separation is critical**: no algorithm, matchmaker, or detection system
  may read `world.players` directly — only `world.observations`. Ground truth
  (`PlayerReality`) is used only by simulation logic and metrics.
- **No comments in code** unless the ticket explicitly asks for them.
- **Unit tests** live in `#[cfg(test)] mod tests` blocks inside each source file.
- **File naming:** `match_.rs` (not `match.rs`, which is a Rust keyword).
- **Determinism:** every experiment must be reproducible from seed; the
  `SeedManager` derives per-domain seeds from one experiment seed.
- Keep `AGENTS.md` in sync — if a ticket changes the crate set, dependency flow,
  or conventions, update AGENTS.md in the same commit.

## Ticket template fields

Every ticket uses the same structure:

- **Goal** — what the ticket delivers.
- **Scope / Deliverables** — the concrete artifacts to produce.
- **Acceptance criteria** — how to know the ticket is done.
- **Testing** — the tests required to close the ticket.
- **Dependencies** — what must be done first.
- **Notes** — spec references, rough sketches, gotchas.
