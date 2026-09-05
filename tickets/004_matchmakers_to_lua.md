# 004 — Port matchmakers to Lua

## Summary

Make `Matchmaker` fully Lua-implementable and **delete the Rust matchmakers**.
`LuaMatchmaker` becomes the only way to build a matchmaker; batch,
expanding-window, strict, and hub-spoke are ported to Lua scripts under
`plugins/matchmaking/`.

## Context

`matchlab-matchmaking` currently has `BatchMatchmaker`, `ExpandingWindowMatchmaker`,
`StrictMatchmaker`, `HubSpokeMatchmaker` (with hook-style `LuaHooks` on several).
The queue, `ProposedMatch`, `Constraint`, `MatchObjective`, and `SearchStrategy`
are infrastructure and stay in Rust. The trait is unchanged; the loop keeps
calling `find_matches`.

`find_matches` receives `&Queue`, `&World`, `team_size`, `now`, and `&mut SimRng`.
The adapter snapshots the queue into a Lua table and routes randomness through
`matchlab.rng_*`. The `World` is not passed to Lua — the snapshot comes entirely
from queue entries (which already carry the observation data matchmaking is
allowed to see), preserving truth separation.

## Scope

**In:**
- `LuaMatchmaker` adapter in `matchlab-matchmaking::lua`.
- Lua ports of batch, expanding_window, strict, hub_spoke.
- Queue-snapshot conversion.
- Config + runner wiring; loop test updates; manifest matchmaking sections;
  tests.

**Out:**
- No changes to `queue.rs`, `matchmaker.rs` (trait + `ProposedMatch`),
  `constraint.rs`, `objective.rs`, `search.rs`. (Scripts receive thresholds via
  config and can implement their own search logic inside the script.)

## Design

### Lua contract (matchmaker script)

```lua
function find_matches(queue, team_size, now_secs, config, context)
    -- queue: array of entries (matchlab-lua builds these):
    --   { player_id, rating, rating_deviation, games_played, win_rate,
    --     joined_at_secs, region, party_id, latency_ms, game_mode }
    -- team_size: integer; now_secs: number
    -- returns (matches, context)
    -- matches = { { team_a = {ids}, team_b = {ids}, quality_score }, ... }
end
```

The adapter fills `quality_score` into each `ProposedMatch`; scripts may set it
or leave it 0 (the adapter can compute it via `ProposedMatch::match_quality` if
absent, using observations only — note the adapter's queue snapshot is
observation-derived, so this stays truth-separation-clean).

### `matchlab-matchmaking::lua` — `LuaMatchmaker`

```rust
pub struct LuaMatchmaker { vm: LuaVm, context: Mutex<Context> }

impl LuaMatchmaker {
    pub fn load(script: &str, params: &serde_yaml::Value) -> Result<Self, String>;
    // validate_script(path, &["find_matches"])
}
```

`Matchmaker` impl:
- `find_matches(queue, world, team_size, now, rng)` → build the queue snapshot
  from `queue.entries()` (region to string; `joined_at` to secs via
  `now.duration_since(...)`); `vm.with_rng(rng, |vm| ...)` calls
  `find_matches(queue_tbl, team_size, now_secs, config, ctx)` → `(matches_tbl,
  ctx)`; map each to `ProposedMatch { team_a, team_b, quality_score }`; store
  `ctx`.

### Scripts (`plugins/matchmaking/`)

Port from the current Rust implementations, preserving formation semantics:

- `batch.lua` — **rating-balanced** formation: sort candidates by `rating`
  (ties by `joined_at`), assign alternately to team A / team B in consecutive
  `2 * team_size` blocks. Adjacent-by-rating players land on opposite teams →
  balanced teams, quality ~0.96–0.98 on the standard population.
- `expanding_window.lua` — stepped tiers `[(max_secs, allowed_diff)]` from
  `config.tiers` (default `5s→25, 10s→50, 20s→100, 30s→200`, fallback
  `config.max_window`); window widens with queue wait.
- `strict.lua` — only match within `config.max_skill_diff`; outliers wait
  indefinitely (intended strict behavior).
- `hub_spoke.lua` — partition the queue by `region`; under-capacity regions use
  a greedy regional match, overflow handled longest-waiting-first. **The nested
  `Box<dyn Matchmaker>` spokes of the Rust version are not expressible in pure
  Lua**; the script implements the regional greedy/overflow logic inline (same
  behavior as the current overflow path), reading `config.spoke_capacity`.

### Deletions

- `crates/matchlab-matchmaking/src/batch.rs`, `expanding.rs`, `strict.rs`,
  `hub_spoke.rs`, `hooks.rs`, `loader.rs`.
- Keep: `queue.rs`, `matchmaker.rs`, `constraint.rs`, `objective.rs`,
  `search.rs`. Update `lib.rs` exports.

### Config + runner

- `config.rs` — `MatchmakingSpec` becomes:
  ```rust
  pub struct MatchmakingSpec {
      pub script: String,                 // plugins/matchmaking/batch.lua, ...
      #[serde(flatten)] pub params: HashMap<String, serde_yaml::Value>,
  }
  ```
  Remove `algorithm`. Keep `batch_interval` as a param (the runner reads it to
  schedule the `MatchTimer`; the script also reads `config.batch_interval` if it
  wants it). Keep `max_queue_time` param.
- `runner.rs::build_matchmaker` → `LuaMatchmaker::load(&spec.script, &params)`.
  `batch_interval_secs` still reads `spec.params["batch_interval"]`.
- Runner `MINI` manifest → `script: plugins/matchmaking/batch.lua` +
  `batch_interval: 10`.
- `expanding_window_matchmaker_runs` test → point at `expanding_window.lua`
  with tiers params.

### Consumers to update

- `crates/matchlab-loop/src/machine.rs` tests: `BatchMatchmaker::new(10)` →
  load `plugins/matchmaking/batch.lua`.
- Manifests: matchmaking sections in `v0_1_basic.yaml`, `base/standard.yaml`,
  `matchmaker_comparison.yaml` (expanding_window script + tiers params), and any
  others.
- Delete `plugins/matchmaking/adaptive_quality.lua` and
  `plugins/matchmaking/custom_formation.lua` (hook-style; superseded).

## Steps

1. Implement `lua.rs` (`LuaMatchmaker`) + queue-snapshot conversion.
2. Write the four scripts under `plugins/matchmaking/`.
3. Delete the Rust matchmaker files and hooks/loader; update `lib.rs`.
4. Update `config.rs` + `runner.rs`; update runner tests.
5. Update `machine.rs` tests (Lua batch helper).
6. Update manifest matchmaking sections; remove hook-style scripts.
7. Write ported tests (below).
8. Update `AGENTS.md` (matchmaking crate section).

## Acceptance Criteria

- [ ] `cargo build/test/check --workspace`, `clippy`, `fmt` pass.
- [ ] No reference to `BatchMatchmaker`, `ExpandingWindowMatchmaker`,
      `StrictMatchmaker`, `HubSpokeMatchmaker`, or `LuaHooks` remains
      (grep-clean).
- [ ] `batch.lua` forms balanced teams: on a queue of players whose ratings are
      a spread around 1000, `ProposedMatch::match_quality` per formed match is
      ≥ 0.9 (vs the naive FIFO pairing which caps near 0.68).
- [ ] `expanding_window.lua` widens the acceptable skill diff as `now_secs -
      joined_at_secs` grows; a pair that fails the 5s tier forms under a later
      tier.
- [ ] `strict.lua` returns no match for a queue whose rating spread exceeds
      `max_skill_diff`.
- [ ] `hub_spoke.lua` keeps same-region players together when possible and
      forms overflow matches longest-waiting-first.
- [ ] Determinism: `find_matches` over the same queue/seed produces identical
      proposals (and, when a script draws randomness, consumes `SimRng`
      identically).
- [ ] The loop's `handle_match_timer` still caps formation by the remaining
      match budget with the Lua matchmaker; `v0_1_basic` reproduces
      match_quality mean ≈ 0.98 and queue time ≈ 5s.

## Testing

- Batch: quality distribution on a synthetic spread queue; deterministic
  ordering (ties by `joined_at`).
- Expanding: tier progression test (wait 2s → tier1 diff; wait 25s → tier4).
- Strict: within-window forms; outside-window returns none.
- Hub-spoke: two-region queue; under-capacity region uses spoke greedy, overflow
  region falls to hub path; party members stay together when configured.
- Adapter: snapshot field coverage; missing `find_matches` → load error.
- Runner: `expanding_window_matchmaker_runs` (script form) completes 40 matches.

## Risks / Notes

- The current Rust `batch.rs` is **rating-balanced** (not the FIFO pseudocode in
  `docs/spec.md` §7.8). Port the actual code; the quality gate above will catch
  a regression to FIFO.
- Hub-spoke loses true nested matchmakers; the inline regional greedy is an
  acceptable fidelity loss (documented). A future ticket could add Lua-to-Lua
  sub-matchmaker delegation via a registry callback if needed.