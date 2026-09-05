# matchlab Refactor: Lua-Native Systems

## Goal

Invert the current plugin model. Today Rust owns every algorithm and Lua tweaks
decision points (`on_k_factor`, `on_match_quality`, ...). After this refactor
**Lua owns the algorithm** and Rust is a thin binding layer: core types, event
loop, trait adapters, and a deterministic helper API. `plugins/` becomes the
only source of systems — there are no inherent Rust algorithms (no `elo`,
`glicko`, `batch`, `smurf`, ...).

**Decisions locked in:**

- **All 8 layers** become Lua-pluggable: rating, outcome model, matchmaking,
  detection, metrics, adversarial agents, satisfaction, rank mapping.
- **Pure/stateless Lua functions + a context blob.** Each Lua system is a set of
  pure functions. A `Context` (arbitrary, script-defined data) is stored on the
  Rust model, passed into every call, and replaced by what the script returns.
  This enables genuinely novel stateful systems while keeping the "Lua hides no
  state" hygiene and full determinism.
- **Rust implementations are deleted.** The Lua scripts become the single
  source of truth. Existing golden tests (e.g. the Glicko-2 worked example) are
  ported to test the Lua scripts.
- **`script: <path>` config.** Manifests reference systems by script path;
  `name:` is optional and only a label. No `lua:` prefix, no algorithm-name
  dispatch in the runner.

**Hard invariants (must survive the refactor):**

- Truth separation: algorithms see `PlayerObservation` only; metrics are the
  sole legitimate reader of `PlayerReality` besides the simulation.
- Determinism: one seed → byte-identical results. All randomness flows through
  `SimRng` via `matchlab.rng_*` helpers; `math.random` is banned in scripts.
- Trait signatures (`RatingSystem`, `OutcomeModel`, `Matchmaker`,
  `DetectionSystem`, `MetricCollector`, `AdversarialAgent`, `RankMapper`) stay
  **unchanged** so the loop and counterfactual eval are untouched by the layer
  ports. Each Lua adapter stores interior `Mutex<Context>`.
- `AGENTS.md` is updated in every ticket that changes architecture (repo rule),
  with a comprehensive rewrite in the docs ticket.

## Ticket order

Dependency chain: `001 → 002..008 → 009 → 010 → 011 → 012`.

| # | Ticket | Depends on |
|---|--------|------------|
| 001 | Create `matchlab-lua` crate (shared VM, context, rng, validation, conversions) | — |
| 002 | Port rating systems to Lua (adapter + 4 scripts) | 001 |
| 003 | Port outcome models to Lua (adapter + 6 scripts) | 001 |
| 004 | Port matchmakers to Lua (adapter + 4 scripts) | 001 |
| 005 | Port detection systems to Lua (adapter + smurf script) | 001 |
| 006 | Port metric collectors to Lua (adapter + 12 scripts) | 001 |
| 007 | Port adversarial agents to Lua (adapter + 5 scripts) | 001 |
| 008 | Port satisfaction model + rank mapper to Lua (adapters + 2 scripts) | 001 |
| 009 | Config/runner consolidation + script-content hashing | 002–008 |
| 010 | Experiment manifest overhaul + dogfood examples | 009 |
| 011 | Documentation overhaul (AGENTS.md + spec.md) | 009 |
| 012 | Final verification & acceptance run | 010, 011 |

Tickets 002–008 are mutually independent once 001 lands; they are listed in a
recommended order (rating first, as it is the most-referenced). Each touches the
experiments runner and the loop test suite, so doing them sequentially keeps the
workspace green at every commit.

## Cross-cutting rules for every ticket

- **Keep the tree green.** Every ticket ends with `cargo build --workspace`,
  `cargo test --workspace`, `cargo check --workspace`, `clippy`, `fmt` passing.
- **Manifests stay runnable.** Update the affected sections of
  `experiments/*.yaml` in the same ticket as the schema change.
- **Update `AGENTS.md`** for the crates/tickets you touch.
- **Lua scripts are tersely commented.** A short header explaining the contract
  and config is desirable (they are teaching artifacts); no inline chatter.
- **Scripts are pure.** No `math.random`; randomness only via `matchlab.rng_*`.
- **Tests live in `#[cfg(test)]`** in the same file, per repo convention.