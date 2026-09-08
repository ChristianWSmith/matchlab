# API Reference Quality Standards

Standards and current status of the public API reference documentation.

---

## Standards for `rustdoc` on Public Types

Every public type must have a `///` doc comment that explains:

1. **What it is** — one sentence describing the type's purpose.
2. **When to use it** — the typical context or use case.
3. **Key invariants** — any constraints the caller must respect.

### Example

```rust
/// Monotonic simulation clock with nanosecond resolution.
///
/// `SimTime` is used throughout the simulation to schedule events and measure
/// durations. It is always monotonically increasing — time never goes backward.
///
/// Use `SimTime::from_secs` or `SimTime::from_millis` to create instances
/// from real-world units. The internal representation is `u64` nanoseconds,
/// giving a range of ~584 years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimTime(u64);
```

### Required fields

| Field | Required | Notes |
|-------|----------|-------|
| Summary sentence | Yes | One line, starts with the type name or "A" |
| Description paragraph | Yes | 2–5 sentences explaining purpose and usage |
| Field docs | Yes (on public fields) | Each field documented with units and constraints |
| Method docs | Yes (on public methods) | See section below |
| Examples | Recommended | At least one `/// # Examples` block on the primary constructor |
| Panics | If applicable | Document conditions that cause a panic |
| Errors | If applicable | Document error variants and when they occur |
| Safety | If applicable | Required for `unsafe` items |

---

## Required Documentation for Public Functions

Every public function must document:

1. **What it does** — one sentence describing the function's behavior.
2. **Parameters** — each parameter's purpose and constraints.
3. **Return value** — what the function returns and its semantics.
4. **Panics** — conditions that cause a panic (if any).
5. **Errors** — error conditions (if the return type is `Result`).
6. **Examples** — at least one working example in `/// # Examples`.

### Example

```rust
/// Compute win probability for team A against team B.
///
/// Returns a probability in [0.0, 1.0] representing the likelihood that
/// team A wins. The computation uses each player's visible rating only —
/// ground truth skill is never accessed (truth separation).
///
/// # Arguments
///
/// * `team_a` — observations of players on team A
/// * `team_b` — observations of players on team B
///
/// # Returns
///
/// A value in [0.0, 1.0]. Values near 0.5 indicate balanced teams;
/// values near 0.0 or 1.0 indicate a strong mismatch.
///
/// # Panics
///
/// Panics if either team is empty.
///
/// # Examples
///
/// ```
/// use matchlab_core::player::{PlayerObservation, SkillVector};
///
/// let a = PlayerObservation { rating: 1500.0, ..Default::default() };
/// let b = PlayerObservation { rating: 1000.0, ..Default::default() };
/// let prob = win_probability(&[a], &[b]);
/// assert!(prob > 0.5);
/// ```
pub fn win_probability(team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
    // ...
}
```

### Documentation for trait methods

Trait methods must document the contract that implementations must satisfy:

```rust
/// Update ratings based on a match result.
///
/// Implementations receive a budget-sanitized `MatchResult` (scores, durations,
/// and performances are zeroed for non-permitted data) and the current
/// observations of all participants.
///
/// # Contract
///
/// - Must return a `RatingState` for every player in `match_result.participants`.
/// - Must not panic.
/// - Must produce finite values for `rating` and `rating_deviation`.
/// - The returned states are applied to `World.observations` by the loop.
fn update(
    &self,
    match_result: &MatchResult,
    observations: &HashMap<PlayerId, PlayerObservation>,
) -> HashMap<PlayerId, RatingState>;
```

---

## Naming Conventions

### Types

| Category | Convention | Examples |
|----------|-----------|----------|
| Core types | Noun, PascalCase | `SimTime`, `PlayerId`, `MatchResult` |
| Traits | Verb/noun, PascalCase | `RatingSystem`, `OutcomeModel`, `MetricCollector` |
| Enums | PascalCase, descriptive | `EventKind`, `ObservationType`, `InterventionAction` |
| Structs | PascalCase, descriptive | `PlayerReality`, `QueueEntry`, `ProposedMatch` |
| Error types | `Error` suffix | `StoreError`, `ConfigError` |

### Functions

| Category | Convention | Examples |
|----------|-----------|----------|
| Constructors | `new` or `from_*` | `SimTime::from_secs`, `World::new` |
| Getters | field name | `rating()`, `player_id()`, `time()` |
| Predicates | `is_*` or `has_*` | `is_empty()`, `has_history()` |
| Conversions | `to_*` or `as_*` | `as_secs_f64()`, `into_match_result()` |
| Queries | noun or `*_of` | `rank_of()`, `top_n()`, `entries()` |
| Mutations | verb | `record()`, `update()`, `advance()` |
| Calculations | descriptive verb | `win_probability()`, `match_quality()`, `compute()` |

### Modules

| Location | Convention | Examples |
|----------|-----------|----------|
| Crate root | `lib.rs` | Re-exports public API |
| Domain modules | `snake_case.rs` | `outcome.rs`, `population.rs` |
| Lua adapters | `lua.rs` | One per crate that has Lua scripts |
| Plugin registries | `plugins.rs` | Maps names to script paths |

---

## Units Documentation Requirements

All numeric fields and parameters must document their units:

| Field | Units | Example doc |
|-------|-------|-------------|
| `SimTime` | nanoseconds internally | "Nanosecond resolution simulation clock" |
| `rating` | Elo-scale points | "Player rating on the Elo scale (centered at 1500)" |
| `rating_deviation` | Elo-scale points | "Rating uncertainty; 350 = completely uncertain" |
| `volatility` | Glicko-2 scale | "Rating volatility parameter (σ)" |
| `games_played` | count | "Total number of completed matches" |
| `win_rate` | [0.0, 1.0] | "Fraction of matches won" |
| `skill` | game-specific scale | "Player skill; game-specific units (default: Elo-scale)" |
| `quality_score` | [0.0, 1.0] | "Match balance quality; 1.0 = perfectly balanced" |
| `queue_time` | seconds | "Time spent waiting in queue before match formation" |
| `duration` | seconds | "Simulated match duration" |
| `latency_ms` | milliseconds | "Estimated network latency" |
| `confidence` | [0.0, 1.0] | "Statistical confidence level (e.g. 0.95 for 95% CI)" |
| `effect_size` | Cohen's d units | "Standardized mean difference between conditions" |

### Unit documentation pattern

```rust
/// Time since epoch in nanoseconds (SimTime internally).
///
/// Convert to seconds with `as_secs_f64()`.
pub time: SimTime,

/// Player rating on the Elo scale (centered at 1500).
///
/// Higher values indicate stronger players. Typical range is 800–2400.
pub rating: f64,
```

---

## Example Requirements for Public APIs

Every public API item must have at least one example that:

1. **Compiles** — `cargo test --doc` must pass.
2. **Runs** — the example must execute without panicking.
3. **Is realistic** — use realistic values, not placeholder data.
4. **Shows the primary use case** — demonstrate the typical calling pattern.

### Example block format

```rust
/// # Examples
///
/// ```
/// use matchlab_core::time::SimTime;
///
/// let t = SimTime::from_secs(3600);
/// assert_eq!(t.as_secs_f64(), 3600.0);
/// ```
```

### Multiple examples

For functions with multiple use cases, provide multiple examples:

```rust
/// # Examples
///
/// Balanced teams (high quality):
/// ```
/// # use matchlab_matchmaking::matchmaker::match_quality;
/// let q = match_quality(&[1000.0, 1000.0], &[1000.0, 1000.0]);
/// assert!((q - 1.0).abs() < 0.01);
/// ```
///
/// Mismatched teams (low quality):
/// ```
/// # use matchlab_matchmaking::matchmaker::match_quality;
/// let q = match_quality(&[1000.0, 1000.0], &[2000.0, 2000.0]);
/// assert!(q < 0.1);
/// ```
```

### Testing examples

```bash
# Run all doc tests
cargo test --workspace --doc

# Run doc tests for a specific crate
cargo test -p matchlab-core --doc
```

---

## Current Status of API Documentation

### Coverage by crate

| Crate | `rustdoc` coverage | Doc tests | Status |
|-------|-------------------|-----------|--------|
| `matchlab-core` | Types documented, methods partially | Minimal | Needs work |
| `matchlab-lua` | Types documented | None | Needs examples |
| `matchlab-players` | Types documented | None | Needs examples |
| `matchlab-game` | Types documented | Minimal | Needs examples |
| `matchlab-matchmaking` | Types documented | Minimal | Needs examples |
| `matchlab-rating` | Types documented | None | Needs examples |
| `matchlab-detection` | Types documented | None | Needs examples |
| `matchlab-ranking` | Types documented | None | Needs examples |
| `matchlab-loop` | Types documented | None | Needs examples |
| `matchlab-metrics` | Types documented | None | Needs examples |
| `matchlab-experiments` | Types documented | Minimal | Needs examples |
| `matchlab-analysis` | Types documented | None | Needs examples |
| `matchlab-objective` | Types documented | None | Needs examples |
| `matchlab-adversarial` | Types documented | None | Needs examples |
| `matchlab-utility` | Types documented | None | Needs examples |
| `matchlab-validation` | Test-only, no public API | N/A | N/A |

### What needs documentation

1. **All public functions** — add `///` doc comments with the required fields.
2. **All public types** — ensure summary + description + fields are documented.
3. **Doc examples** — add at least one `# Examples` block to each primary constructor
   and each public method.
4. **Panics/errors** — document all panic conditions and error variants.
5. **Units** — add units to all numeric field docs.

### Building the docs locally

```bash
# Generate docs
cargo doc --workspace --no-deps

# Open in browser
open target/doc/matchlab_core/index.html
```

### Checking for missing docs

```bash
# Warn on missing docs (add to lib.rs temporarily)
#![warn(missing_docs)]

# Then build and count warnings
cargo doc --workspace 2>&1 | grep "warning:" | wc -l
```

---

## Documentation Testing

### Doc tests

Every code example in `///` doc comments is compiled and run as a test:

```bash
cargo test --workspace --doc
```

Doc tests verify that examples compile and produce the expected output. They
catch:
- API misuse in examples
- Outdated examples after API changes
- Missing imports

### Doctests for Lua systems

Lua scripts cannot be tested via `rustdoc`, but the Lua adapter can be tested
via integration tests:

```bash
cargo test -p matchlab-rating -- lua
cargo test -p matchlab-game -- lua
cargo test -p matchlab-matchmaking -- lua
```

### Link checking

External links in documentation should be validated:

```bash
# Install cargo-deadlinks
cargo install cargo-deadlinks

# Check for broken links
cargo deadlinks --workspace
```

---

## Documentation Best Practices

### Write for the reader

- Start with what the reader needs to know, not background information.
- Use concrete examples, not abstract descriptions.
- Put the most important information first.

### Keep documentation close to code

- Doc comments (`///`) live on the item they document.
- Module-level docs (`//!`) go at the top of the file.
- Avoid separate documentation files for things that belong in rustdoc.

### Test your documentation

- `cargo test --doc` catches broken examples.
- Run examples manually to verify they make sense.
- Ask someone unfamiliar with the code to read your docs.

### Document decisions, not just behavior

- Explain *why* a function works the way it does.
- Document non-obvious constraints or requirements.
- Reference the spec when the behavior is defined there.
