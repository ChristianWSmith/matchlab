# Contributing to matchlab

Release engineering, compatibility, and contribution guidelines.

---

## Development Environment Setup

### Prerequisites

- **Rust** edition 2024 (stable toolchain, `rustup default stable`)
- **Git** 2.30+
- **OS:** Linux, macOS, or Windows (WSL2 recommended on Windows)

### Quick start

```bash
git clone https://github.com/ChristianWSmith/matchlab.git
cd matchlab
cargo build --workspace
cargo test --workspace
```

### Verify your environment

```bash
# Check Rust version
rustc --version   # should be 1.85+ (edition 2024)

# Check clippy
cargo clippy --workspace -- -D warnings

# Check formatting
cargo fmt --check
```

---

## Code Style

### Rust edition

All code is Rust **edition 2024**. Key differences from 2021:

- `gen` is a keyword — use `r#gen` when calling `rand::Rng::gen`.
- `unsafe` blocks are required in `unsafe fn` bodies.
- `impl Trait` in return position captures all in-scope lifetimes by default.
- `dyn Trait` requires explicit `dyn` keyword.

### Formatting

```bash
cargo fmt --check       # verify
cargo fmt               # fix
```

All code must pass `cargo fmt` with default settings (rustfmt.toml if present).
CI will reject PRs that don't.

### Clippy

```bash
cargo clippy --workspace -- -D warnings
```

All code must pass clippy with no warnings. CI will reject PRs that don't.

### Comments

**No comments in code unless explicitly requested.** Code should be
self-documenting through naming and structure. If you think a comment is needed,
consider whether the code can be rewritten to make the comment unnecessary.

The exception is:
- `// SAFETY:` comments for `unsafe` blocks (required by clippy)
- `#[doc = "..."]` on public API items (see API Reference below)

### Naming conventions

| Item | Convention | Example |
|------|-----------|---------|
| Types | `PascalCase` | `PlayerReality`, `SimTime` |
| Functions | `snake_case` | `win_probability`, `filter_match_result` |
| Constants | `SCREAMING_SNAKE_CASE` | `ZERO`, `DEFAULT_K_FACTOR` |
| Modules | `snake_case` | `outcome.rs`, `match_.rs` |
| Files | `snake_case`, avoid keywords | `match_.rs` (not `match.rs`) |
| Lua scripts | `snake_case.lua` | `elo.lua`, `expanding_window.lua` |
| Config keys | `snake_case` YAML | `skill_update_interval_secs` |

### Rustdoc conventions

Every public type must have a `///` doc comment explaining what it is, when to use it, and key invariants. Every public function must document what it does, parameters, return value, panics/errors, and include at least one `# Examples` block. All numeric fields must document their units.

### Workspace structure

The workspace root `Cargo.toml` declares shared dependencies. Never add
dependencies to individual crates without adding them to the workspace first:

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
rand = "0.8"
rand_chacha = "0.3"
mlua = { version = "0.10", features = ["lua54", "vendored"] }
```

---

## Adding a New Lua Plugin

Every algorithm is a Lua script under `plugins/`. Adding a new system requires
no Rust code — just a script and a manifest reference.

### Step 1: Choose the layer

| Layer | Directory | Contract |
|-------|-----------|----------|
| Rating system | `plugins/rating/` | `information_budget`, `initialize`, `predict`, `update` |
| Outcome model | `plugins/game/` | `win_probability`, `simulate` |
| Matchmaker | `plugins/matchmaking/` | `find_matches` |
| Metric collector | `plugins/metrics/` | `name`, `on_record`, `compute` |
| Detection system | `plugins/detection/` | `observe`, `evaluate`, `recommend_action` |
| Rank mapper | `plugins/ranking/` | `rating_to_rank`, `rank_to_rating_range` |
| Adversarial agent | `plugins/adversarial/` | `tick`, `objective` |
| Satisfaction model | `plugins/utility/` | `satisfaction`, `retention_probability`, `rematch_probability` |

### Step 2: Write the script

Use an existing script as a template. For example, to add a new rating system:

```lua
-- plugins/rating/my_system.lua

local M = {}

M.name = "my_system"
M.information_budget = { "WinLoss" }  -- or {"WinLoss", "Score", "Duration", "PerformanceData"}

function M.initialize(player_id)
    return {
        rating = 1000,
        rating_deviation = 350,
        volatility = 0.06,
        games_played = 0,
    }
end

function M.predict(team_a, team_b)
    -- compute win probability for team_a
    return 0.5
end

function M.update(match_result, observations)
    local states = {}
    for _, participant in ipairs(match_result.participants) do
        local obs = observations[participant.player_id]
        states[participant.player_id] = {
            rating = obs.rating + 32 * (participant.outcome - 0.5),
            rating_deviation = obs.rating_deviation,
            volatility = obs.volatility,
            games_played = obs.games_played + 1,
        }
    end
    return states
end

return M
```

### Step 3: Reference in manifest

```yaml
rating:
  systems:
    - name: my_system
      script: plugins/rating/my_system.lua
      params: {}
```

### Step 4: Validate

```bash
# Check the script parses and has required functions
cargo test -p matchlab-rating -- --test-threads=1 lua

# Run an experiment with the new system
cargo run -- run experiments/my_experiment.yaml
```

---

## Adding a New Metric

### Step 1: Write the collector script

```lua
-- plugins/metrics/my_metric.lua

local M = {}

M.name = "my_metric"

-- Optional: enable time-bucketed output
-- function M.time_buckets()
--     return {60, 300, 900, 3600, 86400, 604800}
-- end

-- Optional: include full population snapshot (not just match participants)
-- M.needs_population = true

function M.on_record(match, world)
    -- Accumulate samples in the context table
    local ctx = matchlab.context or {}
    ctx.samples = ctx.samples or {}
    table.insert(ctx.samples, match.quality_score or 0)
    matchlab.context = ctx
end

function M.compute()
    local ctx = matchlab.context or {}
    local samples = ctx.samples or {}
    if #samples == 0 then
        return { type = "scalar", value = 0 }
    end
    local sum = 0
    for _, v in ipairs(samples) do sum = sum + v end
    return { type = "scalar", value = sum / #samples }
end

return M
```

### Step 2: Register the metric

Add the metric name to the registry in `crates/matchlab-metrics/src/lua.rs`:

```rust
"my_metric" => "plugins/metrics/my_metric.lua",
```

### Step 3: Reference in manifest

```yaml
metrics:
  collectors:
    - my_metric
```

### Step 4: Test

```bash
cargo test -p matchlab-metrics
cargo run -- run experiments/my_experiment.yaml
```

---

## Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p matchlab-core
cargo test -p matchlab-rating

# With output
cargo test --workspace -- --nocapture

# Specific test
cargo test -p matchlab-validation test_name

# Validation suite (analytical baselines)
cargo test -p matchlab-validation
```

### Test categories

| Category | Location | Purpose |
|----------|----------|---------|
| Unit tests | `#[cfg(test)] mod tests` in each file | Per-function correctness |
| Integration tests | `crates/*/tests/` | Cross-module behavior |
| Validation tests | `crates/matchlab-validation/tests/` | Analytical baselines |
| Metamorphic tests | `crates/matchlab-validation/tests/metamorphic.rs` | Property-based invariants |
| Info-budget tests | `crates/matchlab-validation/tests/info_budget.rs` | Truth-separation enforcement |

---

## Running CI Checks Locally

Run these before pushing to avoid CI failures:

```bash
# Format
cargo fmt --check

# Clippy
cargo clippy --workspace -- -D warnings

# All tests
cargo test --workspace

# Full build
cargo build --workspace

# Release build (for performance testing)
cargo build --workspace --release
```

### CI workflow

The `.github/workflows/ci.yml` runs:

1. `cargo fmt --check`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace`
4. `cargo build --workspace`

All four must pass for a PR to merge.

---

## Breaking Change Policy

### What counts as a breaking change

- Removing or renaming a public type, function, or field
- Changing the signature of a public function
- Changing the serialization format of `ExperimentResult`, `StudyResult`, or
  any other artifact type
- Changing the Lua script contract (required functions, argument order, return
  type)
- Changing the YAML manifest schema
- Changing the CLI interface

### What is not a breaking change

- Adding new public types, functions, or fields
- Adding new Lua script functions (backward-compatible extensions)
- Adding new YAML manifest keys (ignored by older versions)
- Changing internal implementation details
- Adding new metric collectors or rating systems
- Bug fixes that change behavior to match the spec

### Versioning

We follow SemVer. Breaking changes bump the major version. Non-breaking
additions bump the minor version. Bug fixes bump the patch version.

Until 1.0, minor version bumps may include breaking changes with a migration
guide in the changelog.

---

## Commit Message Conventions

Format:

```
<type>: <short summary>

<optional body>

<optional footer>
```

### Types

| Type | When to use |
|------|-------------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code restructuring, no behavior change |
| `test` | Adding or updating tests |
| `chore` | Build, CI, dependency updates |
| `perf` | Performance improvement |
| `ci` | CI configuration changes |

### Examples

```
feat: add Glicko-2 rating system Lua script
fix: correct win probability clamp in logistic outcome model
docs: add reproduction studies guide
test: add metamorphic test for population scaling invariance
chore: bump mlua to 0.10
```

### Rules

- Subject line: imperative mood, lowercase, no period, max 72 characters
- Body: explain *what* and *why*, not *how*
- Reference issues with `#123`
- Breaking changes: add `BREAKING CHANGE:` footer

---

## PR Process

### Before opening a PR

1. **Create a branch** from `main`:
   ```bash
   git checkout -b feat/my-feature main
   ```

2. **Make your changes** following the guidelines above.

3. **Run all checks locally:**
   ```bash
   cargo fmt --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```

4. **Write tests** for new functionality. Bug fixes should include a regression
   test that fails before the fix and passes after.

5. **Update documentation** if your change affects the public API, config format,
   or plugin contract.

### Opening a PR

- Use a descriptive title matching commit message conventions.
- Reference related issues (`Closes #123`).
- Include a summary of changes in the PR description.
- Tag reviewers if you know who should review.

### Review checklist

Reviewers will check:

- [ ] Code passes `cargo fmt --check`
- [ ] Code passes `cargo clippy --workspace -- -D warnings`
- [ ] All tests pass
- [ ] New code has tests
- [ ] No comments in code (unless explicitly needed)
- [ ] Public API items have `#[doc]` attributes
- [ ] Config schema changes are reflected in documentation
- [ ] Breaking changes are noted in the PR description

### After review

- Address feedback with new commits (don't force-push during review).
- Re-run checks after each update.
- Squash-merge when approved.

---

## Plugin Development Guide

### Script contract

Every Lua script receives:

1. **`config`** — a table populated from the `params:` key in the manifest.
2. **`context`** — a persistent table stored on the Rust model and threaded
   through every call. Scripts may read/write arbitrary state here.
3. **`matchlab.rng_*`** — deterministic randomness functions. Never use
   `math.random`.

### Available RNG functions

```lua
matchlab.rng_range(min, max)     -- uniform float in [min, max)
matchlab.rng_bool(probability)   -- true with given probability
matchlab.rng_normal(mean, std)   -- normal distribution
matchlab.rng_u64()               -- random u64
```

### Truth separation

Scripts receive **observations only**, never `PlayerReality`. The exception is
metric collectors, which legitimately receive ground-truth fields (`true_skill`,
`improvement_rate`, `reality_games_played`) for computing accuracy metrics.

The following fields are **never** available to rating/matchmaking/detection:

- `skill_vector` (true multidimensional skill)
- `hidden_mmr` (true MMR)
- `improvement_rate`
- `reality_games_played`

### Information budget

Rating systems declare what data they are allowed to see:

```lua
M.information_budget = { "WinLoss" }                    -- Elo, Flat
M.information_budget = { "WinLoss", "Score" }           -- score-aware
M.information_budget = { "WinLoss", "Score", "Duration", "PerformanceData" }  -- full
```

The loop sanitizes `MatchResult` data through `filter_match_result` before
calling `update`. Scripts that declare `WinLoss` but read score data will
receive zeroed scores.

### Testing scripts

```bash
# Validate a script has required functions and no math.random
cargo test -p matchlab-lua -- validate

# Run an experiment with the script
cargo run -- run experiments/my_experiment.yaml

# Compare against a baseline
cargo run -- compare results/baseline.json results/my_change.json
```
