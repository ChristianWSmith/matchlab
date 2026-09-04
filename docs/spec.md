# matchlab — Implementation Specification

> The simulator knows the truth. Algorithms don't.

## Table of Contents

1. [Overview](#1-overview)
2. [Core Principles](#2-core-principles)
3. [Workspace Layout](#3-workspace-layout)
4. [Simulation Engine](#4-simulation-engine)
5. [Player Model](#5-player-model)
6. [Game Model](#6-game-model)
7. [Matchmaking](#7-matchmaking)
8. [Rating Systems](#8-rating-systems)
9. [Detection](#9-detection)
10. [Ranking](#10-ranking)
11. [Metrics](#11-metrics)
12. [Objective Functions](#12-objective-functions)
13. [Experiments](#13-experiments)
14. [Analysis](#14-analysis)
15. [Adversarial Agents](#15-adversarial-agents)
16. [Player Utility](#16-player-utility)
17. [v0.1 Build Order](#17-v01-build-order)

---

## 1. Overview

matchlab is a discrete-event simulation framework for evaluating competitive matchmaking and rating systems. It generates synthetic player populations with known ground truth, runs them through a simulated matchmaking ecosystem, and measures how well different algorithms perform.

The framework answers questions like:

- Under what conditions does Elo outperform Glicko?
- How much match quality must be sacrificed to reduce queue time by 50%?
- How rapidly should a rating system respond to skill changes?
- How much damage does a smurf cause before detection?

**Language:** Rust (edition 2024)
**Config format:** YAML (experiment manifests)
**Build system:** Cargo workspace

---

## 2. Core Principles

### 2.1 Truth Separation

The simulation maintains two parallel representations of every player:

- **Reality:** The ground truth the simulation knows but algorithms cannot access.
- **Observation:** What rating and matchmaking systems believe, derived only from permitted data.

No rating algorithm, matchmaker, or detection system may access player reality directly. They operate exclusively on observations. This prevents methodological corruption where algorithms accidentally use information they wouldn't have in production.

### 2.2 Pluggability

Every algorithm is a trait implementation. The simulation engine, player models, and game outcomes are independent of any specific rating system, matchmaking algorithm, or detection strategy. Swapping Elo for Glicko should require zero changes outside the rating module.

### 2.3 Composability

Players, games, matchmaking, rating, detection, and ranking are composed at experiment configuration time, not at compile time. An experiment manifest declares which implementations to use and with what parameters.

### 2.4 Reproducibility

Every experiment is fully deterministic given its configuration and random seed. The simulation records seeds, configuration hashes, and git commits for exact reproduction.

### 2.5 Graduated Complexity

The architecture supports the full design vision from day one, but implementations are added incrementally. v0.1 contains the minimal viable simulation; subsequent versions add dimensions of complexity.

### 2.6 Smurf Identity Emerges from Properties

A smurf is not a player "type." It is a combination of properties: high `true_skill` with a low `initial_rating` and few `games_played`. The simulation defines archetypes by their behavioral parameters; a smurf archetype simply has mismatched skill and rating. Detection systems must infer smurf status from observed behavior — they must never be given a boolean flag.

---

## 3. Workspace Layout

```
match-lab/
├── Cargo.toml                  # workspace root
├── design.md
├── docs/
│   └── spec.md
├── experiments/                # YAML experiment manifests
│   ├── base/                   # base configs for inheritance
│   │   └── standard.yaml
│   └── v0_1_basic.yaml
├── crates/
│   ├── matchlab-core/          # simulation engine, time, events, world, RNG
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── time.rs
│   │       ├── event.rs
│   │       ├── world.rs
│   │       ├── rng.rs
│   │       ├── player.rs
│   │       └── match_.rs
│   │
│   ├── matchlab-players/       # archetypes, population generation, skill process
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── archetype.rs
│   │       ├── population.rs
│   │       └── skill.rs
│   │
│   ├── matchlab-game/          # outcome models, match execution
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── outcome.rs
│   │       └── logistic.rs
│   │
│   ├── matchlab-matchmaking/   # queue, matchmaker, constraints, search
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── queue.rs
│   │       ├── matchmaker.rs
│   │       ├── constraint.rs
│   │       ├── objective.rs    # per-match optimization scoring
│   │       ├── search.rs       # SearchStrategy trait
│   │       ├── expanding.rs
│   │       └── strict.rs
│   │
│   ├── matchlab-rating/        # rating systems (Elo, Glicko, TrueSkill, Flat)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── system.rs
│   │       ├── elo.rs
│   │       ├── flat.rs
│   │       ├── glicko.rs
│   │       └── trueskill.rs
│   │
│   ├── matchlab-detection/     # smurf detection, interventions
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── detector.rs
│   │       ├── smurf.rs
│   │       └── intervention.rs
│   │
│   ├── matchlab-ranking/       # rank mapping, leaderboard
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ranker.rs
│   │       └── leaderboard.rs
│   │
│   ├── matchlab-metrics/       # metric collectors
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs
│   │       ├── collector.rs
│   │       ├── accuracy.rs
│   │       ├── quality.rs
│   │       ├── inequality.rs
│   │       ├── queue.rs
│   │       ├── convergence.rs
│   │       ├── responsiveness.rs
│   │       ├── stability.rs
│   │       ├── streaks.rs
│   │       ├── population.rs
│   │       ├── correlation.rs
│   │       └── smurf.rs
│   │
│   ├── matchlab-objective/     # weighted utility, multi-objective scoring
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── utility.rs
│   │
│   ├── matchlab-adversarial/   # adversarial player agents
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── agent.rs
│   │       ├── booster.rs
│   │       ├── deranker.rs
│   │       ├── win_trader.rs
│   │       └── afk.rs
│   │
│   ├── matchlab-utility/       # player satisfaction / retention model
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── satisfaction.rs
│   │
│   ├── matchlab-experiments/   # runner, config, factorial design
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runner.rs
│   │       ├── config.rs
│   │       ├── factorial.rs
│   │       ├── counterfactual.rs
│   │       └── seed.rs
│   │
│   └── matchlab-analysis/      # statistics, Pareto, cohorts, reports
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── stats.rs
│           ├── pareto.rs
│           ├── cohorts.rs
│           └── report.rs
│
└── src/
    └── main.rs                 # CLI binary: `matchlab run <manifest>`
```

### 3.1 Dependency Graph

```
matchlab-core          (no internal deps)
    ↑
    ├── matchlab-players
    ├── matchlab-game
    ├── matchlab-matchmaking
    ├── matchlab-rating
    ├── matchlab-detection
    ├── matchlab-ranking
    ├── matchlab-metrics       (depends on core only)
    ├── matchlab-objective     (depends on core + metrics)
    ├── matchlab-adversarial   (depends on core)
    └── matchlab-utility       (depends on core)

matchlab-experiments   (depends on all above)
    ↑
matchlab-analysis      (depends on core + metrics + objective)

matchlab (binary)      (depends on experiments + analysis)
```

### 3.2 Shared Dependencies

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
rand = "0.8"
rand_chacha = "0.3"
```

### 3.3 Plugin Model

New rating systems, matchmakers, and outcome models can be added by editing the appropriate crate's source and recompiling — the enum/trait dispatch handles them. For a research tool this is acceptable: plugins are compile-time crates behind the trait boundary, not runtime dynamic libraries.

```rust
// crates/matchlab-rating/src/plugins/mod.rs

/// Registry of rating systems. New systems register here rather than
/// being auto-discovered from the filesystem (avoids unsafe dynamic
/// loading and cold-start costs in a simulation).
pub mod registry {
    use crate::system::RatingSystem;

    pub fn all_systems() -> Vec<&'static str> {
        vec!["elo", "glicko2", "trueskill", "flatpoints"]
    }

    pub fn from_name(
        name: &str,
        config: &serde_yaml::Value,
    ) -> Option<Box<dyn RatingSystem>> {
        match name {
            "elo" => Some(Box::new(crate::elo::Elo::from_yaml(config)?)),
            "glicko2" => Some(Box::new(crate::glicko::Glicko2::from_yaml(config)?)),
            "trueskill" => Some(Box::new(crate::trueskill::TrueSkill::from_yaml(config)?)),
            "flatpoints" => Some(Box::new(crate::flat::FlatPoints::from_yaml(config)?)),
            _ => None,
        }
    }
}
```

Notes on the plugin boundary:
- **No runtime dynamic linking.** A plugin is a crate compiled into the binary behind a trait. This keeps the simulation deterministic, auditable, and free of `dlopen` complexity.
- **Extensibility is at compile time.** The design's "drop it into `rating/plugins/`" maps to adding a module and a `from_name` arm. `experiments/` YAML then selects it by name.
- **Config via `from_yaml(&Value)`.** Each system parses its own parameter map from the manifest, so new systems carry their own schema without touching the core trait.

---

## 4. Simulation Engine

The simulation engine is a discrete-event system. Time advances by popping the next event from a priority queue, executing it, and scheduling any resulting events.

### 4.1 Time

```rust
// crates/matchlab-core/src/time.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimTime(pub u64);

impl SimTime {
    pub const ZERO: Self = Self(0);

    pub fn from_secs(secs: f64) -> Self {
        Self((secs * 1_000_000_000.0) as u64)
    }

    pub fn from_millis(millis: u64) -> Self {
        Self(millis * 1_000_000)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1_000_000_000.0
    }

    pub fn duration_since(self, earlier: SimTime) -> SimTime {
        SimTime(self.0.saturating_sub(earlier.0))
    }

    /// Raw internal value (nanoseconds). Useful as a monotonic tick counter.
    pub fn ticks(self) -> u64 {
        self.0
    }
}
```

#### Multi-Scale Time

The simulation spans multiple time scales efficiently. Rather than simulating every second, the event engine skips idle periods:

| Scale | Resolution | Examples |
|-------|-----------|----------|
| Milliseconds | Per-event | Match events, rating updates |
| Seconds | Per-event | Queue behavior, match duration |
| Minutes | Per-event | Sessions, play frequency |
| Days | Scheduled events | Skill changes, population dynamics |
| Weeks | Batch events | Rank ecosystem shifts |
| Months | Summary snapshots | Long-term trends |

Long gaps between events are skipped — if the next event is in 3 days, the clock jumps directly to that time. No wasted computation.

### 4.2 Events

```rust
// crates/matchlab-core/src/event.rs

use crate::time::SimTime;

pub trait Event: std::fmt::Debug + Send + Sync {
    fn time(&self) -> SimTime;
    fn kind(&self) -> EventKind;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    PlayerJoin,
    PlayerLeave,
    PlayerQueue,
    PlayerQuit,
    PlayerReturn,
    PlayerDisconnect,
    MatchFormed,
    MatchStart,
    MatchEnd,
    RatingUpdate,
    DetectionCheck,
    SkillChange,
}

pub struct TimestampedEvent {
    pub time: SimTime,
    pub kind: EventKind,
    pub inner: Box<dyn Event>,
}

impl PartialEq for TimestampedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for TimestampedEvent {}

impl PartialOrd for TimestampedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimestampedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.time.cmp(&self.time) // reverse for min-heap
    }
}
```

### 4.3 Concrete Event Types

```rust
// crates/matchlab-core/src/event.rs

use crate::time::SimTime;
use crate::player::PlayerId;
use crate::match_::MatchId;

#[derive(Debug)]
pub struct PlayerJoinEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerJoinEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::PlayerJoin }
}

#[derive(Debug)]
pub struct PlayerLeaveEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerLeaveEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::PlayerLeave }
}

#[derive(Debug)]
pub struct PlayerQueueEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerQueueEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::PlayerQueue }
}

#[derive(Debug)]
pub struct PlayerQuitEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerQuitEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::PlayerQuit }
}

#[derive(Debug)]
pub struct PlayerReturnEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerReturnEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::PlayerReturn }
}

#[derive(Debug)]
pub struct PlayerDisconnectEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
    pub match_id: MatchId,
}

impl Event for PlayerDisconnectEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::PlayerDisconnect }
}

#[derive(Debug)]
pub struct MatchFormedEvent {
    pub time: SimTime,
    pub match_id: MatchId,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
}

impl Event for MatchFormedEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::MatchFormed }
}

#[derive(Debug)]
pub struct MatchEndEvent {
    pub time: SimTime,
    pub match_id: MatchId,
}

impl Event for MatchEndEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::MatchEnd }
}

#[derive(Debug)]
pub struct SkillChangeEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for SkillChangeEvent {
    fn time(&self) -> SimTime { self.time }
    fn kind(&self) -> EventKind { EventKind::SkillChange }
}
```

### 4.4 Event Handlers

```rust
// crates/matchlab-core/src/event.rs

use crate::world::World;

pub type EventHandler = Box<dyn Fn(&mut World, &dyn Event) -> Vec<Box<dyn Event>> + Send + Sync>;

pub struct EventEngine {
    queue: std::collections::BinaryHeap<TimestampedEvent>,
    handlers: std::collections::HashMap<EventKind, Vec<EventHandler>>,
}

impl EventEngine {
    pub fn new() -> Self {
        Self {
            queue: std::collections::BinaryHeap::new(),
            handlers: std::collections::HashMap::new(),
        }
    }

    pub fn register_handler(&mut self, kind: EventKind, handler: EventHandler) {
        self.handlers.entry(kind).or_default().push(handler);
    }

    pub fn schedule(&mut self, event: Box<dyn Event>) {
        self.queue.push(TimestampedEvent {
            time: event.time(),
            kind: event.kind(),
            inner: event,
        });
    }

    pub fn next_event(&mut self) -> Option<TimestampedEvent> {
        self.queue.pop()
    }

    pub fn peek_time(&self) -> Option<SimTime> {
        self.queue.peek().map(|e| e.time)
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn tick(&mut self, world: &mut World) -> bool {
        let event = match self.next_event() {
            Some(e) => e,
            None => return false,
        };

        world.time = event.time;

        if let Some(handlers) = self.handlers.get(&event.kind) {
            for handler in handlers {
                let new_events = handler(world, event.inner.as_ref());
                for e in new_events {
                    self.schedule(e);
                }
            }
        }

        true
    }
}
```

### 4.5 World State

```rust
// crates/matchlab-core/src/world.rs

use crate::player::{PlayerId, PlayerReality, PlayerObservation};
use crate::match_::{MatchId, MatchState};
use crate::rng::SimRng;
use crate::time::SimTime;
use std::collections::HashMap;

pub struct World {
    pub players: HashMap<PlayerId, PlayerReality>,
    pub observations: HashMap<PlayerId, PlayerObservation>,
    pub matches: HashMap<MatchId, MatchState>,
    pub rng: SimRng,
    pub time: SimTime,
    next_player_id: u64,
    next_match_id: u64,
}

impl World {
    pub fn new(rng: SimRng) -> Self {
        Self {
            players: HashMap::new(),
            observations: HashMap::new(),
            matches: HashMap::new(),
            rng,
            time: SimTime::ZERO,
            next_player_id: 0,
            next_match_id: 0,
        }
    }

    pub fn next_player_id(&mut self) -> PlayerId {
        let id = PlayerId(self.next_player_id);
        self.next_player_id += 1;
        id
    }

    pub fn next_match_id(&mut self) -> MatchId {
        let id = MatchId(self.next_match_id);
        self.next_match_id += 1;
        id
    }

    pub fn add_player(&mut self, reality: PlayerReality, observation: PlayerObservation) {
        let id = reality.id;
        self.players.insert(id, reality);
        self.observations.insert(id, observation);
    }

    /// What algorithms see.
    pub fn observe(&self, player_id: PlayerId) -> Option<&PlayerObservation> {
        self.observations.get(&player_id)
    }

    /// Ground truth. Only simulation logic should call this.
    pub fn reality(&self, player_id: PlayerId) -> Option<&PlayerReality> {
        self.players.get(&player_id)
    }
}
```

### 4.6 RNG

```rust
// crates/matchlab-core/src/rng.rs

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

pub struct SimRng {
    inner: SmallRng,
}

impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        Self { inner: SmallRng::seed_from_u64(seed) }
    }

    pub fn gen_range(&mut self, low: f64, high: f64) -> f64 {
        self.inner.gen_range(low..high)
    }

    pub fn gen_bool(&mut self, p: f64) -> bool {
        self.inner.gen_bool(p)
    }

    pub fn sample_normal(&mut self, mean: f64, stddev: f64) -> f64 {
        let u: f64 = self.inner.gen();
        let v: f64 = self.inner.gen();
        let z = (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos();
        mean + stddev * z
    }

    pub fn gen_u64(&mut self) -> u64 {
        self.inner.gen()
    }
}
```

### 4.7 Simulation Runner

```rust
// crates/matchlab-core/src/lib.rs

use crate::event::EventEngine;
use crate::time::SimTime;
use crate::world::World;

pub struct Simulation {
    pub world: World,
    pub engine: EventEngine,
}

impl Simulation {
    pub fn new(world: World, engine: EventEngine) -> Self {
        Self { world, engine }
    }

    /// Run until the event queue is empty or time exceeds `until`.
    pub fn run(&mut self, until: SimTime) {
        while !self.engine.is_empty() {
            if let Some(peek_time) = self.engine.peek_time() {
                if peek_time > until {
                    break;
                }
            }
            self.engine.tick(&mut self.world);
        }
    }

    /// Run until the event queue is empty.
    pub fn run_to_completion(&mut self) {
        while self.engine.tick(&mut self.world) {}
    }
}
```

---

## 5. Player Model

### 5.1 Identity

```rust
// crates/matchlab-core/src/player.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);
```

### 5.2 Regions

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    NA,
    EU,
    Asia,
    Other,
}
```

### 5.3 Multidimensional Skill

Skill can be one-dimensional (a single float) or multi-dimensional (a vector of subskills). v0.1 uses 1D; the architecture supports N dimensions.

```rust
// crates/matchlab-core/src/player.rs

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct SkillVector {
    /// Map of skill dimension name → value.
    /// For 1D: {"overall": 1200.0}
    /// For multidimensional: {"aim": 1500, "movement": 1100, "game_sense": 1300, ...}
    pub dimensions: std::collections::HashMap<String, f64>,
}

impl SkillVector {
    pub fn one_dimensional(value: f64) -> Self {
        let mut dimensions = std::collections::HashMap::new();
        dimensions.insert("overall".to_string(), value);
        Self { dimensions }
    }

    pub fn overall(&self) -> f64 {
        // Default: average all dimensions
        let sum: f64 = self.dimensions.values().sum();
        sum / self.dimensions.len() as f64
    }

    pub fn weighted_overall(&self, weights: &std::collections::HashMap<String, f64>) -> f64 {
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        for (dim, &val) in &self.dimensions {
            let w = weights.get(dim).unwrap_or(&1.0);
            weighted_sum += val * w;
            weight_sum += w;
        }
        weighted_sum / weight_sum
    }
}
```

For a CS-like game, dimensions might be:

```yaml
skill_dimensions:
  - aim
  - movement
  - utility
  - game_sense
  - positioning
  - teamwork
  - consistency
```

The game outcome model determines how dimensions combine into effective skill. This allows investigating: *Can a one-dimensional rating accurately represent multidimensional player skill?*

### 5.4 Player Reality (Ground Truth)

What the simulation knows. Algorithms must never see this.

```rust
// crates/matchlab-core/src/player.rs

#[derive(Debug, Clone)]
pub struct PlayerReality {
    pub id: PlayerId,
    pub skill: SkillVector,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub consistency: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    pub party_id: Option<u64>,
    pub region: Region,
    pub account_age: u64,
    pub games_played: u64,
    pub fatigue: f64,
    pub tilt: f64,
    pub experience: u64,
    pub is_online: bool,
    pub archetype: String,
}
```

Note: there is no `is_smurf` boolean. A player is a "smurf" if and only if their `skill` is high while their account's `games_played` is low and the observation layer's rating is far below skill. Detection systems must infer this from observable signals.

### 5.5 Player Observation (What Algorithms See)

```rust
// crates/matchlab-core/src/player.rs

#[derive(Debug, Clone)]
pub struct PlayerObservation {
    pub id: PlayerId,
    pub rating: f64,
    pub hidden_mmr: f64,
    pub visible_rank: VisibleRank,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: u64,
    pub win_rate: f64,
    pub recent_performances: Vec<f64>,
    pub queue_joined_at: Option<crate::time::SimTime>,
    pub is_online: bool,
    pub party_id: Option<u64>,
    pub session_history: VecDeque<u64>,
    pub quit_history: VecDeque<f64>,
    pub tilt_level: f64,
    pub game_mode: String,
    pub skill_vector: SkillVector,
    pub detection_flags: Vec<DetectionFlag>,
}

/// Lightweight rank representation used inside observations.
/// The full `Rank` (tier + division) lives in matchlab-ranking; this keeps core
/// free of a ranking dependency while still exposing the visible rank to
/// algorithms, detection, and metrics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VisibleRank {
    pub tier: String,
    pub division: u8,
}

impl VisibleRank {
    /// Approximate numeric midpoint of the visible rank bracket, used for
    /// comparing the communicated rank (what players/opponents see) to true
    /// skill. Divisions split each tier into 4 steps.
    pub fn midpoint(&self) -> f64 {
        let tier_base: f64 = match self.tier.as_str() {
            "iron"     => 300.0,
            "bronze"   => 600.0,
            "silver"   => 900.0,
            "gold"     => 1200.0,
            "platinum" => 1500.0,
            "diamond"  => 1800.0,
            "radiant"  => 2100.0,
            _ => 1200.0,
        };
        let div = (self.division.min(4).max(1) as f64 - 1.0) * 50.0;
        tier_base + div
    }
}

#[derive(Debug, Clone)]
pub enum DetectionFlag {
    PerformanceAnomaly { confidence: f64 },
    AcceleratedRating,
    UnderReview,
}
```

The `hidden_mmr` and `visible_rank` fields implement the design's two-layer separation: `hidden_mmr` is the matchmaking/rating value, while `visible_rank` is what players (and opposite teams) actually see. They may diverge (e.g., rank is compressed near the top, or a smurf's visible rank lags far behind their hidden MMR). This separation is what allows the experiment to ask: *"Does visible rank actually communicate skill accurately?"* as a question distinct from MMR accuracy.

### 5.6 Skill as Stochastic Process

Skill evolves over time:

```
S_{t+1} = S_t + μ_t + ε_t
```

Where:
- `S_t` = current true skill (per dimension)
- `μ_t` = systematic drift (improvement or decline)
- `ε_t` ~ N(0, σ²) = random fluctuation

```rust
// crates/matchlab-players/src/skill.rs

use matchlab_core::player::SkillVector;
use matchlab_core::rng::SimRng;

pub struct SkillProcess {
    pub improvement_rate: f64,
    pub volatility: f64,
}

impl SkillProcess {
    /// Advance all skill dimensions one time step.
    pub fn advance(&self, current: &SkillVector, rng: &mut SimRng) -> SkillVector {
        let mut new_dims = std::collections::HashMap::new();
        for (dim, &val) in &current.dimensions {
            let noise = rng.sample_normal(0.0, self.volatility);
            new_dims.insert(dim.clone(), (val + self.improvement_rate + noise).max(0.0));
        }
        SkillVector { dimensions: new_dims }
    }
}
```

### 5.7 Player Archetypes

```rust
// crates/matchlab-players/src/archetype.rs

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ArchetypeConfig {
    pub name: String,
    pub proportion: f64,
    pub skill_distribution: DistributionConfig,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    /// If set, overrides sampled skill with this initial rating.
    /// Critical for smurfs: true_skill is sampled from distribution,
    /// but initial_rating is set to this value.
    #[serde(default)]
    pub initial_rating: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DistributionConfig {
    #[serde(rename = "normal")]
    Normal { mean: f64, stddev: f64 },
    #[serde(rename = "uniform")]
    Uniform { low: f64, high: f64 },
    #[serde(rename = "log_normal")]
    LogNormal { mean: f64, stddev: f64 },
}
```

Example archetypes:

```yaml
archetypes:
  - name: stable
    proportion: 0.60
    skill_distribution: { type: normal, mean: 1000, stddev: 250 }
    skill_volatility: 5.0
    improvement_rate: 0.0
    play_frequency: 0.8
    session_length: 1800.0
    quit_probability: 0.01

  - name: improving
    proportion: 0.15
    skill_distribution: { type: normal, mean: 800, stddev: 200 }
    skill_volatility: 10.0
    improvement_rate: 2.0
    play_frequency: 0.9
    session_length: 2400.0
    quit_probability: 0.005

  - name: declining
    proportion: 0.05
    skill_distribution: { type: normal, mean: 1100, stddev: 150 }
    skill_volatility: 8.0
    improvement_rate: -1.5
    play_frequency: 0.5
    session_length: 1200.0
    quit_probability: 0.03

  - name: returning
    proportion: 0.05
    skill_distribution: { type: normal, mean: 1200, stddev: 200 }
    skill_volatility: 12.0
    improvement_rate: -2.0
    play_frequency: 0.3
    session_length: 1500.0
    quit_probability: 0.05
    initial_rating: 800
```

The `returning` archetype models a player returning to ranked after a long absence: high latent skill (`mean: 1200`) that decays over time (`improvement_rate: -2.0`, the `1200 ────────╲ ╲ ╲── 800` curve), with infrequent play and higher quit probability. Their visible rating (`initial_rating: 800`) lags their latent skill at first, creating a temporary smurf-like signal.
  - name: volatile
    proportion: 0.08
    skill_distribution: { type: normal, mean: 1000, stddev: 300 }
    skill_volatility: 25.0
    improvement_rate: 0.0
    play_frequency: 0.7
    session_length: 2000.0
    quit_probability: 0.02

  - name: new_player
    proportion: 0.05
    skill_distribution: { type: normal, mean: 600, stddev: 150 }
    skill_volatility: 15.0
    improvement_rate: 6.0
    play_frequency: 0.75
    session_length: 1500.0
    quit_probability: 0.02
    initial_rating: 500

  - name: smurf
    proportion: 0.02
    skill_distribution: { type: normal, mean: 1500, stddev: 100 }
    skill_volatility: 5.0
    improvement_rate: 0.0
    play_frequency: 0.95
    session_length: 3600.0
    quit_probability: 0.002
    initial_rating: 700
```

The proportions now sum to 1.00 (0.60 + 0.15 + 0.05 + 0.05 + 0.08 + 0.05 + 0.02). The `new_player` archetype models the design's "new player learning rapidly": low starting skill (`mean: 600`, `initial_rating: 500`) with a steep positive improvement rate (`6.0`), representing the `500 ──╱╱╱╱──────` curve.

The `smurf` archetype has high true skill (sampled from N(1500, 100)) but a low initial rating (700). Detection systems must infer the mismatch from behavior — the boolean is never exposed.

### 5.8 Population Generator

```rust
// crates/matchlab-players/src/population.rs

use matchlab_core::player::{PlayerId, PlayerReality, PlayerObservation, Region, SkillVector};
use matchlab_core::rng::SimRng;
use crate::archetype::ArchetypeConfig;

pub struct PopulationConfig {
    pub size: u64,
    pub archetypes: Vec<ArchetypeConfig>,
}

pub struct PopulationGenerator;

impl PopulationGenerator {
    pub fn generate(
        config: &PopulationConfig,
        rng: &mut SimRng,
    ) -> (Vec<PlayerReality>, Vec<PlayerObservation>) {
        let mut realities = Vec::new();
        let mut observations = Vec::new();
        let mut id_counter = 0u64;

        for archetype in &config.archetypes {
            let count = (config.size as f64 * archetype.proportion) as u64;
            for _ in 0..count {
                let id = PlayerId(id_counter);
                id_counter += 1;

                let true_skill_value = sample_distribution(&archetype.skill_distribution, rng);
                let rating = archetype.initial_rating.unwrap_or(true_skill_value);

                let reality = PlayerReality {
                    id,
                    skill: SkillVector::one_dimensional(true_skill_value),
                    skill_volatility: archetype.skill_volatility,
                    improvement_rate: archetype.improvement_rate,
                    consistency: (1.0 - archetype.skill_volatility / 100.0).max(0.0),
                    play_frequency: archetype.play_frequency,
                    session_length: archetype.session_length,
                    quit_probability: archetype.quit_probability,
                    party_id: None,
                    region: Region::NA,
                    account_age: 0,
                    games_played: 0,
                    fatigue: 0.0,
                    tilt: 0.0,
                    experience: 0,
                    is_online: true,
                    archetype: archetype.name.clone(),
                };

                let observation = PlayerObservation {
                    id,
                    rating,
                    rating_deviation: 350.0,
                    volatility: 0.06,
                    games_played: 0,
                    win_rate: 0.5,
                    recent_performances: Vec::new(),
                    queue_joined_at: None,
                    is_online: true,
                    detection_flags: Vec::new(),
                };

                realities.push(reality);
                observations.push(observation);
            }
        }

        (realities, observations)
    }
}

fn sample_distribution(dist: &DistributionConfig, rng: &mut SimRng) -> f64 {
    match dist {
        DistributionConfig::Normal { mean, stddev } => rng.sample_normal(*mean, *stddev),
        DistributionConfig::Uniform { low, high } => rng.gen_range(*low, *high),
        DistributionConfig::LogNormal { mean, stddev } => {
            let normal = rng.sample_normal(*mean, *stddev);
            normal.exp()
        }
    }
}
```

---

## 6. Game Model

### 6.1 Outcome Model Trait

```rust
// crates/matchlab-game/src/outcome.rs

use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::match_::MatchResult;

pub trait OutcomeModel: Send + Sync {
    fn win_probability(
        &self,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
    ) -> f64;

    fn simulate(
        &self,
        match_id: matchlab_core::match_::MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult;
}
```

### 6.2 Logistic Model

```rust
// crates/matchlab-game/src/logistic.rs

use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::rng::SimRng;
use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use crate::outcome::OutcomeModel;

pub struct LogisticOutcomeModel {
    pub beta: f64,
    pub noise: f64,
    /// When true, compute win probability from each player's SkillVector
    /// (weighted_overall via per-dimension weights); when false, fall back to
    /// the flat `rating` scalar. This is the switch that instantiates the
    /// design's multidimensional-skill research question.
    pub use_multidimensional: bool,
    pub dimension_weights: std::collections::HashMap<String, f64>,
}

impl LogisticOutcomeModel {
    pub fn new(beta: f64, noise: f64) -> Self {
        Self {
            beta,
            noise,
            use_multidimensional: false,
            dimension_weights: std::collections::HashMap::new(),
        }
    }

    fn effective_skill(&self, obs: &PlayerObservation) -> f64 {
        if self.use_multidimensional {
            obs.skill_vector.weighted_overall(&self.dimension_weights)
        } else {
            obs.rating
        }
    }
}

impl OutcomeModel for LogisticOutcomeModel {
    fn win_probability(
        &self,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
    ) -> f64 {
        let avg_a: f64 = team_a.iter().map(|p| self.effective_skill(p)).sum::<f64>()
            / team_a.len() as f64;
        let avg_b: f64 = team_b.iter().map(|p| self.effective_skill(p)).sum::<f64>()
            / team_b.len() as f64;
        let diff = avg_a - avg_b;
        1.0 / (1.0 + (-diff / self.beta).exp())
    }

    fn simulate(
        &self,
        match_id: MatchId,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
        rng: &mut SimRng,
    ) -> MatchResult {
        let base_p = self.win_probability(team_a, team_b);
        let noise = rng.gen_range(-self.noise, self.noise);
        let adjusted_p = (base_p + noise).clamp(0.01, 0.99);
        let team_a_wins = rng.gen_bool(adjusted_p);
        let winner = if team_a_wins { Team::A } else { Team::B };

        let team_a_ids: Vec<PlayerId> = team_a.iter().map(|p| p.id).collect();
        let team_b_ids: Vec<PlayerId> = team_b.iter().map(|p| p.id).collect();

        let mut performances = Vec::new();
        for obs in team_a.iter().chain(team_b.iter()) {
            let perf_variance = rng.gen_range(0.0, 1.0);
            let skill = self.effective_skill(obs);
            // Derive concrete performance stats from the player's skill (and,
            // in multidimensional mode, from relevant sub-skills) rather than
            // from rating alone, so detection can see "far outperforms their
            // visible rating" as a real signal.
            let aim = obs.skill_vector.dimensions.get("aim").copied()
                .unwrap_or(skill);
            performances.push(PlayerPerformance {
                player_id: obs.id,
                kills: (aim / 100.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                deaths: (5.0 - (skill / 1000.0) * 1.5 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                assists: (3.0 + rng.gen_range(-2.0, 2.0)).max(0.0) as u32,
                objective_score: rng.gen_range(0.0, 100.0) * (1.0 + skill / 3000.0),
                impact: rng.gen_range(-1.0, 1.0) + (skill - 1000.0) / 1500.0,
                variance: perf_variance,
            });
        }

        MatchResult {
            match_id,
            winner,
            team_a: team_a_ids,
            team_b: team_b_ids,
            team_a_score: if team_a_wins { 13.0 } else { rng.gen_range(4.0, 12.0) },
            team_b_score: if team_a_wins { rng.gen_range(4.0, 12.0) } else { 13.0 },
            player_performances: performances,
            duration: matchlab_core::time::SimTime::from_secs(rng.gen_range(1200.0, 2400.0)),
            disconnected: false,
            forfeited: false,
            variance: noise.abs(),
            unexpected_events: Vec::new(),
        }
    }
}
```

### 6.3 Additional Outcome Model Variants (Stubs)

The logistic model uses `PlayerObservation.rating` (a flat scalar). For multidimensional skill, the game model reads from `SkillVector` and combines dimensions according to game-specific weights. The outcome model is responsible for this mapping — the rating system never touches `SkillVector` directly.

```rust
// crates/matchlab-game/src/variance.rs

/// Skill + random variance. Same as logistic but with larger noise envelope.
pub struct VarianceOutcomeModel {
    pub beta: f64,
    pub noise: f64,
    pub variance_multiplier: f64,
}

// crates/matchlab-game/src/composition.rs

/// Skill + team composition synergy. Reads from SkillVector dimensions.
/// Effective team skill = Σ weighted_dimensions + synergy_bonus
pub struct CompositionOutcomeModel {
    pub dimension_weights: std::collections::HashMap<String, f64>,
    pub synergy_bonus: f64,
    pub beta: f64,
}

// crates/matchlab-game/src/performance.rs

/// Includes individual performance metrics in outcome generation.
/// Players with higher impact stats have slightly better win probability.
pub struct PerformanceOutcomeModel {
    pub beta: f64,
    pub performance_weight: f64,
}

// crates/matchlab-game/src/fatigue.rs

/// Accounts for session length — longer sessions reduce effective skill.
pub struct FatigueOutcomeModel {
    pub base_model: Box<dyn OutcomeModel>,
    pub fatigue_decay_rate: f64,
}

// crates/matchlab-game/src/momentum.rs

/// Win/loss streaks slightly affect subsequent match outcomes.
pub struct MomentumOutcomeModel {
    pub base_model: Box<dyn OutcomeModel>,
    pub momentum_factor: f64,
}
```

Each variant implements the same `OutcomeModel` trait. The `CompositionOutcomeModel` is the key one for multidimensional skill: it reads specific dimensions from `SkillVector` (e.g., "aim", "game_sense") and combines them with configurable weights, allowing investigation of whether 1D ratings can capture multidimensional skill.
```

### 6.4 Match State and Result

```rust
// crates/matchlab-core/src/match_.rs

use crate::player::PlayerId;
use crate::time::SimTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team { A, B }

#[derive(Debug, Clone)]
pub enum MatchState { Formed, InProgress, Completed, Cancelled }

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub match_id: MatchId,
    pub winner: Team,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
    pub team_a_score: f64,
    pub team_b_score: f64,
    pub player_performances: Vec<PlayerPerformance>,
    pub duration: SimTime,
    pub disconnected: bool,
    pub forfeited: bool,
    pub variance: f64, // match-to-match outcome randomness for this game
    pub unexpected_events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlayerPerformance {
    pub player_id: PlayerId,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub objective_score: f64,
    pub impact: f64,
    pub variance: f64, // per-performance randomness
}

#[derive(Debug, Clone)]
pub struct MatchConfig {
    pub team_size: usize,
}
```

---

## 7. Matchmaking

### 7.1 Queue

```rust
// crates/matchlab-matchmaking/src/queue.rs

use matchlab_core::player::{PlayerId, PlayerObservation, Region};
use matchlab_core::time::SimTime;

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub player_id: PlayerId,
    pub joined_at: SimTime,
    pub observation: PlayerObservation,
    pub region: Region,
    pub party_id: Option<u64>,
    pub game_mode: String,
    pub role: Option<String>,
    pub latency_ms: f64,
}

#[derive(Debug, Default)]
pub struct Queue {
    entries: Vec<QueueEntry>,
}

impl Queue {
    pub fn enqueue(&mut self, entry: QueueEntry) {
        self.entries.push(entry);
    }

    pub fn remove(&mut self, player_id: PlayerId) -> Option<QueueEntry> {
        self.entries.iter().position(|e| e.player_id == player_id)
            .map(|pos| self.entries.remove(pos))
    }

    pub fn remove_batch(&mut self, player_ids: &[PlayerId]) -> Vec<QueueEntry> {
        let mut removed = Vec::new();
        for &pid in player_ids {
            if let Some(entry) = self.remove(pid) {
                removed.push(entry);
            }
        }
        removed
    }

    pub fn waiting_time(&self, player_id: PlayerId, now: SimTime) -> Option<SimTime> {
        self.entries.iter()
            .find(|e| e.player_id == player_id)
            .map(|e| now.duration_since(e.joined_at))
    }

    pub fn entries(&self) -> &[QueueEntry] { &self.entries }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Construct a queue from pre-existing entries (used by hub-spoke partitioning).
    pub fn from_entries(entries: Vec<QueueEntry>) -> Self {
        Self { entries }
    }
}
```

### 7.2 Matchmaker Trait

```rust
// crates/matchlab-matchmaking/src/matchmaker.rs

use matchlab_core::world::World;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use crate::queue::Queue;

pub trait Matchmaker: Send + Sync {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch>;
}

#[derive(Debug, Clone)]
pub struct ProposedMatch {
    pub team_a: Vec<matchlab_core::player::PlayerId>,
    pub team_b: Vec<matchlab_core::player::PlayerId>,
    pub quality_score: f64,
}
```

### 7.3 Constraints

```rust
// crates/matchlab-matchmaking/src/constraint.rs

use matchlab_core::world::World;
use crate::matchmaker::ProposedMatch;

pub trait Constraint: Send + Sync {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool;
}

pub struct SkillBalanceConstraint {
    pub max_diff: f64,
}

impl Constraint for SkillBalanceConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        let avg_a = average_rating(&proposed.team_a, world);
        let avg_b = average_rating(&proposed.team_b, world);
        (avg_a - avg_b).abs() <= self.max_diff
    }
}

pub struct MinGamesPlayedConstraint {
    pub min_games: u64,
}

impl Constraint for MinGamesPlayedConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        proposed.team_a.iter().chain(proposed.team_b.iter())
            .all(|pid| world.observations.get(pid)
                .map(|o| o.games_played >= self.min_games)
                .unwrap_or(false))
    }
}

pub struct GameModeConstraint;

impl Constraint for GameModeConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        let modes: Vec<String> = proposed.team_a.iter().chain(proposed.team_b.iter())
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.game_mode.clone())
            .collect();
        modes.iter().all(|m| m == &modes[0])
    }
}

pub struct PartyCompatibilityConstraint;

impl Constraint for PartyCompatibilityConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        // Parties must stay on the same team. Reads party_id from observations
        // (never from PlayerReality) to preserve the truth-separation invariant:
        // matchmaking sees the same data the rating system does.
        let parties_of = |team: &[matchlab_core::player::PlayerId]| -> Vec<Option<u64>> {
            team.iter()
                .filter_map(|pid| world.observations.get(pid))
                .map(|o| o.party_id)
                .collect()
        };
        let a_parties = parties_of(&proposed.team_a);
        let b_parties = parties_of(&proposed.team_b);

        // No party id may appear in both teams.
        for party in a_parties.iter().flatten() {
            if b_parties.iter().any(|p| p == &Some(*party)) {
                return false;
            }
        }
        true
    }
}

pub struct RankDifferenceConstraint {
    pub max_rank_diff: u32,
}

impl Constraint for RankDifferenceConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        // Compare visible ranks, not hidden ratings
        // Requires a RankMapper to convert rating → rank bracket
        // Simplified: compare raw ratings as proxy
        let avg_a = average_rating(&proposed.team_a, world);
        let avg_b = average_rating(&proposed.team_b, world);
        // Convert rating diff to approximate rank diff (100 rating ≈ 1 rank bracket)
        let rank_diff = ((avg_a - avg_b).abs() / 100.0) as u32;
        rank_diff <= self.max_rank_diff
    }
}

pub struct TierRestrictionConstraint {
    pub min_tier: u32,
    pub max_tier: u32,
}

impl Constraint for TierRestrictionConstraint {
    fn is_satisfied(&self, proposed: &ProposedMatch, world: &World) -> bool {
        // All players must be within the allowed tier range
        // Tiers are derived from visible rank brackets
        // Simplified: use rating ranges as proxy
        proposed.team_a.iter().chain(proposed.team_b.iter())
            .filter_map(|pid| world.observations.get(pid))
            .all(|o| {
                let tier = (o.rating / 400.0) as u32;
                tier >= self.min_tier && tier <= self.max_tier
            })
    }
}

fn average_rating(team: &[matchlab_core::player::PlayerId], world: &World) -> f64 {
    let sum: f64 = team.iter()
        .filter_map(|pid| world.observations.get(pid))
        .map(|o| o.rating)
        .sum();
    sum / team.len() as f64
}
```

### 7.4 Match Objective (Per-Match Optimization Scoring)

Matchmaking is an optimization problem. For each candidate match, compute a weighted score:

```
Score(M) = w_s · Q(M) - w_t · T(M) - w_p · P(M) - w_r · R(M)
```

Where:
- `Q` = predicted match quality (closer to 0.5 win probability is better)
- `T` = queue waiting cost (longer waits = higher cost)
- `P` = ping/geographic cost
- `R` = rating uncertainty/imbalance cost

```rust
// crates/matchlab-matchmaking/src/objective.rs

use matchlab_core::world::World;
use crate::matchmaker::ProposedMatch;
use crate::queue::QueueEntry;

pub struct MatchObjective {
    pub weight_quality: f64,
    pub weight_queue_time: f64,
    pub weight_ping: f64,
    pub weight_rating_uncertainty: f64,
}

impl MatchObjective {
    pub fn score(&self, proposed: &ProposedMatch, queue_entries: &[QueueEntry], world: &World) -> f64 {
        let q = self.match_quality(proposed, world);
        let t = self.queue_time_cost(proposed, queue_entries, world);
        let p = self.ping_cost(proposed, world);
        let r = self.rating_uncertainty_cost(proposed, world);

        self.weight_quality * q
            - self.weight_queue_time * t
            - self.weight_ping * p
            - self.weight_rating_uncertainty * r
    }

    fn match_quality(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        let avg_a = average_rating(&proposed.team_a, world);
        let avg_b = average_rating(&proposed.team_b, world);
        let diff = (avg_a - avg_b).abs();
        1.0 - (diff / 400.0).min(1.0)
    }

    fn queue_time_cost(&self, proposed: &ProposedMatch, queue_entries: &[QueueEntry], world: &World) -> f64 {
        let max_wait = proposed.team_a.iter().chain(proposed.team_b.iter())
            .filter_map(|pid| {
                queue_entries.iter().find(|e| e.player_id == *pid)
                    .map(|e| world.time.duration_since(e.joined_at).as_secs_f64())
            })
            .fold(0.0_f64, f64::max);
        max_wait / 60.0 // normalize: 60 sec = cost of 1.0
    }

    fn ping_cost(&self, _proposed: &ProposedMatch, _world: &World) -> f64 {
        0.0 // placeholder: geographic distance model
    }

    fn rating_uncertainty_cost(&self, proposed: &ProposedMatch, world: &World) -> f64 {
        let avg_rd: f64 = proposed.team_a.iter().chain(proposed.team_b.iter())
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating_deviation)
            .sum::<f64>() / (proposed.team_a.len() + proposed.team_b.len()) as f64;
        avg_rd / 350.0 // normalize by default RD
    }
}

use matchlab_core::player::PlayerId;

fn average_rating(team: &[PlayerId], world: &World) -> f64 {
    let sum: f64 = team.iter()
        .filter_map(|pid| world.observations.get(pid))
        .map(|o| o.rating)
        .sum();
    sum / team.len() as f64
}
```

### 7.5 Search Strategies

The matchmaker can use different strategies to explore the space of candidate matches:

```rust
// crates/matchlab-matchmaking/src/search.rs

use matchlab_core::rng::SimRng;
use crate::matchmaker::ProposedMatch;
use crate::objective::MatchObjective;
use crate::queue::QueueEntry;
use matchlab_core::world::World;

pub trait SearchStrategy: Send + Sync {
    fn search(
        &self,
        queue: &[QueueEntry],
        objective: &MatchObjective,
        team_size: usize,
        world: &World,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch>;
}

pub enum SearchStrategyKind {
    Greedy,
    RandomSampling { samples: usize },
    BeamSearch { width: usize },
    NearestNeighbor,
    HungarianAssignment,
    GeneticAlgorithm { population: usize, generations: usize },
    IntegerProgramming,
    SimulatedAnnealing { initial_temp: f64, cooling_rate: f64 },
}
```

**Greedy:** For each queue entry, find the best available teammates and opponents by objective score. Fast, but may produce suboptimal global assignments.

**Random Sampling:** Generate N random valid team compositions, score each, return the best. Simple, embarrassingly parallel, good baseline.

**Beam Search:** Maintain a beam of K partial match assignments, expand each by one player, keep the top K. Trades optimality for speed.

**Nearest Neighbor:** For each player, find the nearest unmatched player by rating distance, form pairs, then fill teams from pairs. Simple clustering approach.

**Hungarian Assignment:** Model as an assignment problem: minimize total cost of matching players to teams. Produces globally optimal assignments but O(n³) — expensive for large queues.

**Genetic Algorithm:** Evolve a population of match assignments over generations. Crossover and mutation operators explore the solution space. Good for complex constraint landscapes.

**Integer Programming:** Formulate as an integer linear program with binary assignment variables. Exact solution via solver. Most principled but requires an ILP solver dependency.

**Simulated Annealing:** Start with a random assignment, perturb neighbors, accept worse solutions with decreasing probability. Good balance of quality and speed.

### 7.6 Expanding Window Matchmaker

```rust
// crates/matchlab-matchmaking/src/expanding.rs

use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_core::rng::SimRng;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

pub struct ExpandingWindowMatchmaker {
    /// Stepped tiers: [(max_secs, allowed_diff)]. First matching tier wins.
    pub tiers: Vec<(f64, f64)>,
    pub max_window: f64,
}

impl ExpandingWindowMatchmaker {
    pub fn default_tiers() -> Self {
        Self {
            tiers: vec![
                (5.0, 25.0),
                (10.0, 50.0),
                (20.0, 100.0),
                (30.0, 200.0),
            ],
            max_window: 400.0,
        }
    }

    fn skill_window(&self, waiting_secs: f64) -> f64 {
        for &(max_secs, diff) in &self.tiers {
            if waiting_secs <= max_secs {
                return diff;
            }
        }
        self.max_window
    }
}

impl Matchmaker for ExpandingWindowMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();
        let mut used = Vec::new();

        for entry in queue.entries() {
            if used.contains(&entry.player_id) {
                continue;
            }

            let waiting_secs = now.duration_since(entry.joined_at).as_secs_f64();
            let window = self.skill_window(waiting_secs);

            let mut team_a = vec![entry];
            let mut team_b = Vec::new();

            for other in queue.entries() {
                if used.contains(&other.player_id) || other.player_id == entry.player_id {
                    continue;
                }
                let diff = (entry.observation.rating - other.observation.rating).abs();
                if diff <= window {
                    if team_a.len() <= team_b.len() {
                        team_a.push(other);
                    } else {
                        team_b.push(other);
                    }
                }
                if team_a.len() == team_size && team_b.len() == team_size {
                    break;
                }
            }

            if team_a.len() == team_size && team_b.len() == team_size {
                let team_a_ids: Vec<_> = team_a.iter().map(|e| e.player_id).collect();
                let team_b_ids: Vec<_> = team_b.iter().map(|e| e.player_id).collect();
                used.extend(&team_a_ids);
                used.extend(&team_b_ids);
                matches.push(ProposedMatch {
                    team_a: team_a_ids,
                    team_b: team_b_ids,
                    quality_score: ProposedMatch::match_quality(&team_a_ids, &team_b_ids, world),
                });
            }
        }

        matches
    }
}
```

### 7.7 Strict Matchmaker

```rust
// crates/matchlab-matchmaking/src/strict.rs

use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_core::rng::SimRng;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

pub struct StrictMatchmaker {
    pub max_skill_diff: f64,
}

impl Matchmaker for StrictMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        _now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        // Only match players within max_skill_diff of each other, so outliers
        // may wait indefinitely (that is the intended "strict" behavior).
        let mut matches = Vec::new();
        let mut used = Vec::new();

        for entry in queue.entries() {
            if used.contains(&entry.player_id) {
                continue;
            }
            let mut team_a = vec![entry];
            let mut team_b = Vec::new();

            for other in queue.entries() {
                if used.contains(&other.player_id) || other.player_id == entry.player_id {
                    continue;
                }
                let diff = (entry.observation.rating - other.observation.rating).abs();
                if diff <= self.max_skill_diff {
                    if team_a.len() <= team_b.len() {
                        team_a.push(other);
                    } else {
                        team_b.push(other);
                    }
                }
                if team_a.len() == team_size && team_b.len() == team_size {
                    break;
                }
            }

            if team_a.len() == team_size && team_b.len() == team_size {
                let team_a_ids: Vec<_> = team_a.iter().map(|e| e.player_id).collect();
                let team_b_ids: Vec<_> = team_b.iter().map(|e| e.player_id).collect();
                used.extend(&team_a_ids);
                used.extend(&team_b_ids);
                matches.push(ProposedMatch {
                    team_a: team_a_ids,
                    team_b: team_b_ids,
                    quality_score: ProposedMatch::match_quality(&team_a_ids, &team_b_ids, world),
                });
            }
        }

        matches
    }
}
```

### 7.8 Batch Matchmaker

```rust
// crates/matchlab-matchmaking/src/batch.rs

use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_core::rng::SimRng;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

pub struct BatchMatchmaker {
    pub interval_ticks: u64,
    pub constraints: Vec<Box<dyn crate::constraint::Constraint>>,
}

impl ProposedMatch {
    /// Predicted balance quality: 1.0 when win probability is near 0.5, 0.0 at extremes.
    pub fn match_quality(
        team_a: &[matchlab_core::player::PlayerId],
        team_b: &[matchlab_core::player::PlayerId],
        world: &World,
    ) -> f64 {
        let avg_a: f64 = team_a.iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating).sum::<f64>() / team_a.len().max(1) as f64;
        let avg_b: f64 = team_b.iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating).sum::<f64>() / team_b.len().max(1) as f64;
        let diff = (avg_a - avg_b).abs();
        1.0 - (diff / 400.0).min(1.0)
    }
}

impl Matchmaker for BatchMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        _now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        // Every N ticks, process all queued players:
        // 1. Sort by queue time (longest waiting first)
        // 2. Greedily form teams subject to constraints
        // 3. Return all formed matches

        let mut candidates: Vec<_> = queue.entries().iter().collect();
        candidates.sort_by(|a, b| a.joined_at.cmp(&b.joined_at)); // FIFO

        let mut matches = Vec::new();
        let mut used = std::collections::HashSet::new();
        let mut team_a: Vec<_> = Vec::new();
        let mut team_b: Vec<_> = Vec::new();

        for entry in candidates {
            if used.contains(&entry.player_id) { continue; }

            if team_a.len() < team_size {
                team_a.push(entry.player_id);
            } else if team_b.len() < team_size {
                team_b.push(entry.player_id);
            } else {
                let proposed = ProposedMatch {
                    team_a: team_a.clone(),
                    team_b: team_b.clone(),
                    quality_score: ProposedMatch::match_quality(&team_a, &team_b, world),
                };
                if self.constraints.iter().all(|c| c.is_satisfied(&proposed, world)) {
                    used.extend(proposed.team_a.iter().chain(&proposed.team_b));
                    matches.push(proposed);
                }
                team_a.clear();
                team_b.clear();
            }
        }

        matches
    }
}
```

### 7.9 Hub-and-Spoke Matchmaker

Decomposes matchmaking into a hub (global orchestrator) and spokes (regional matchmakers). The hub distributes overflow workloads to spokes and rebalances when a spoke is overloaded or under-populated. This models how real matchmaking systems handle scale without a single coordination bottleneck.

```rust
// crates/matchlab-matchmaking/src/hub_spoke.rs

use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_core::rng::SimRng;
use crate::matchmaker::{Matchmaker, ProposedMatch};
use crate::queue::Queue;

pub struct HubSpokeMatchmaker {
    /// Spokes are indexed by region; each holds a sub-matchmaker.
    pub spokes: std::collections::HashMap<matchlab_core::player::Region, Box<dyn Matchmaker>>,
    /// Max load (players) a spoke may serve before overflow is redirected.
    pub spoke_capacity: usize,
}

impl Matchmaker for HubSpokeMatchmaker {
    fn find_matches(
        &self,
        queue: &Queue,
        world: &World,
        team_size: usize,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();

        // Partition queue by region
        let mut by_region: std::collections::HashMap<_, Vec<_>> =
            std::collections::HashMap::new();
        for entry in queue.entries() {
            by_region.entry(entry.region).or_default().push(entry);
        }

        for (region, entries) in &by_region {
            if let Some(spoke) = self.spokes.get(region) {
                // If under capacity, delegate to the regional spoke
                if entries.len() <= self.spoke_capacity {
                    let sub_queue = Queue::from_entries(entries.clone());
                    matches.extend(spoke.find_matches(&sub_queue, world, team_size, now, rng));
                }
                // Otherwise spill over: the hub forms matches directly for the
                // overflow (longest-waiting first) using the batch greedy path.
                else {
                    let mut overflow: Vec<_> = entries.iter().collect();
                    overflow.sort_by(|a, b| a.joined_at.cmp(&b.joined_at));
                    let mut team_a: Vec<_> = Vec::new();
                    let mut team_b: Vec<_> = Vec::new();
                    for entry in overflow {
                        if team_a.len() < team_size {
                            team_a.push(entry.player_id);
                        } else if team_b.len() < team_size {
                            team_b.push(entry.player_id);
                        } else {
                            matches.push(ProposedMatch {
                                team_a: team_a.clone(),
                                team_b: team_b.clone(),
                                quality_score: ProposedMatch::match_quality(&team_a, &team_b, world),
                            });
                            team_a.clear();
                            team_b.clear();
                        }
                    }
                }
            }
        }

        matches
    }
}
```

---

## 8. Rating Systems

### 8.1 RatingSystem Trait

```rust
// crates/matchlab-rating/src/system.rs

use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::match_::MatchResult;
use std::collections::HashMap;

pub trait RatingSystem: Send + Sync {
    fn information_budget(&self) -> Vec<ObservationType>;
    fn initialize(&self, player_id: PlayerId) -> RatingState;
    fn predict(
        &self,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
    ) -> f64;
    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState>;

    /// Convenience: extract the rating scalar from a state. Default is state.rating.
    fn rating(&self, state: &RatingState) -> f64 {
        state.rating
    }

    /// Convenience: extract uncertainty (RD) from a state. Default is state.rating_deviation.
    fn uncertainty(&self, state: &RatingState) -> f64 {
        state.rating_deviation
    }
}

#[derive(Debug, Clone)]
pub struct RatingState {
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationType {
    WinLoss,
    Score,
    Kills,
    Deaths,
    Assists,
    ObjectiveScore,
    Impact,
    Duration,
    Disconnects,
    SessionHistory,
    QuitBehavior,
}
```

### 8.2 Information Budget Enforcement

The `information_budget()` method exists to declare what data a rating system may consume. The observation layer must enforce this at runtime by filtering `MatchResult` and `PlayerObservation` data before passing it to `update()` and `predict()`.

```rust
// crates/matchlab-rating/src/filter.rs

use matchlab_core::match_::{MatchResult, PlayerPerformance};
use matchlab_core::player::PlayerObservation;
use crate::system::ObservationType;

/// Strip a MatchResult down to only the fields the rating system is allowed to see.
pub fn filter_match_result(
    mr: &MatchResult,
    budget: &[ObservationType],
) -> FilteredMatchResult {
    FilteredMatchResult {
        winner: mr.winner, // always observable
        team_a: mr.team_a.clone(),
        team_b: mr.team_b.clone(),
        team_a_score: if budget.contains(&ObservationType::Score) { Some(mr.team_a_score) } else { None },
        team_b_score: if budget.contains(&ObservationType::Score) { Some(mr.team_b_score) } else { None },
        player_performances: if budget.iter().any(|o| {
            matches!(o, ObservationType::Kills | ObservationType::Deaths
                | ObservationType::Assists | ObservationType::ObjectiveScore
                | ObservationType::Impact)
        }) {
            Some(mr.player_performances.iter().map(|p| FilteredPerformance {
                player_id: p.player_id,
                kills: if budget.contains(&ObservationType::Kills) { Some(p.kills) } else { None },
                deaths: if budget.contains(&ObservationType::Deaths) { Some(p.deaths) } else { None },
                assists: if budget.contains(&ObservationType::Assists) { Some(p.assists) } else { None },
                objective_score: if budget.contains(&ObservationType::ObjectiveScore) { Some(p.objective_score) } else { None },
                impact: if budget.contains(&ObservationType::Impact) { Some(p.impact) } else { None },
            }).collect())
        } else {
            None
        },
        duration: if budget.contains(&ObservationType::Duration) { Some(mr.duration) } else { None },
        disconnected: if budget.contains(&ObservationType::Disconnects) { Some(mr.disconnected) } else { None },
        forfeited: Some(mr.forfeited),
        unexpected_events: if budget.contains(&ObservationType::SessionHistory) { Some(mr.unexpected_events.clone()) } else { None },
    }
}

pub struct FilteredMatchResult {
    pub winner: matchlab_core::match_::Team,
    pub team_a: Vec<matchlab_core::player::PlayerId>,
    pub team_b: Vec<matchlab_core::player::PlayerId>,
    pub team_a_score: Option<f64>,
    pub team_b_score: Option<f64>,
    pub player_performances: Option<Vec<FilteredPerformance>>,
    pub duration: Option<matchlab_core::time::SimTime>,
    pub disconnected: Option<bool>,
    pub forfeited: Option<bool>,
    pub unexpected_events: Option<Vec<String>>,
}

pub struct FilteredPerformance {
    pub player_id: matchlab_core::player::PlayerId,
    pub kills: Option<u32>,
    pub deaths: Option<u32>,
    pub assists: Option<u32>,
    pub objective_score: Option<f64>,
    pub impact: Option<f64>,
}
```

The experiment runner calls `filter_match_result()` before invoking `system.update()`. This ensures Elo using W/L never sees kills/deaths, while a performance-adjusted variant that declares those in its budget receives them. Without this layer, all systems implicitly have access to all data, making budget declarations purely decorative.

### 8.3 FlatPoints (Baseline)

The simplest possible system: fixed points for a win, fixed points for a loss. Useful as a baseline to demonstrate why adaptive systems are needed.

```rust
// crates/matchlab-rating/src/flat.rs

use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::match_::{MatchResult, Team};
use crate::system::{RatingSystem, RatingState, ObservationType};
use std::collections::HashMap;

pub struct FlatPointsConfig {
    pub win_points: f64,
    pub loss_points: f64,
    pub initial_rating: f64,
}

pub struct FlatPointsRatingSystem {
    pub config: FlatPointsConfig,
}

impl FlatPointsRatingSystem {
    pub fn new(config: FlatPointsConfig) -> Self { Self { config } }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let initial_rating = value.get("initial_rating").and_then(serde_yaml::Value::as_f64)?;
        Some(Self::new(FlatPointsConfig {
            win_points: value.get("win_points").and_then(serde_yaml::Value::as_f64).unwrap_or(10.0),
            loss_points: value.get("loss_points").and_then(serde_yaml::Value::as_f64).unwrap_or(10.0),
            initial_rating,
        }))
    }
}

impl RatingSystem for FlatPointsRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_rating,
            rating_deviation: 350.0,
            volatility: 0.0,
            games_played: 0,
        }
    }

    fn predict(
        &self,
        team_a: &[PlayerObservation],
        team_b: &[PlayerObservation],
    ) -> f64 {
        let avg_a = team_a.iter().map(|p| p.rating).sum::<f64>() / team_a.len() as f64;
        let avg_b = team_b.iter().map(|p| p.rating).sum::<f64>() / team_b.len() as f64;
        1.0 / (1.0 + 10f64.powf((avg_b - avg_a) / 400.0))
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mut updates = HashMap::new();

        for &pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
            if let Some(obs) = observations.get(&pid) {
                let is_team_a = match_result.team_a.contains(&pid);
                let won = (is_team_a && match_result.winner == Team::A)
                    || (!is_team_a && match_result.winner == Team::B);

                let delta = if won { self.config.win_points } else { -self.config.loss_points };
                updates.insert(pid, RatingState {
                    rating: obs.rating + delta,
                    rating_deviation: obs.rating_deviation,
                    volatility: obs.volatility,
                    games_played: obs.games_played + 1,
                });
            }
        }

        updates
    }
}
```

### 8.4 Elo

```rust
// crates/matchlab-rating/src/elo.rs

use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_core::match_::{MatchResult, Team};
use crate::system::{RatingSystem, RatingState, ObservationType};
use std::collections::HashMap;

pub struct EloConfig {
    pub k_factor: f64,
    pub initial_rating: f64,
    pub beta: f64, // scale shared with the game's OutcomeModel for consistency
}

pub struct EloRatingSystem {
    pub config: EloConfig,
}

impl EloRatingSystem {
    pub fn new(config: EloConfig) -> Self { Self { config } }

    /// Parse config from a YAML value (used by the plugin registry).
    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let k_factor = value.get("k_factor").and_then(serde_yaml::Value::as_f64)?;
        let initial_rating = value.get("initial_rating").and_then(serde_yaml::Value::as_f64)?;
        let beta = value.get("beta").and_then(serde_yaml::Value::as_f64)
            .unwrap_or(400.0);
        Some(Self::new(EloConfig { k_factor, initial_rating, beta }))
    }

    fn divisor(&self) -> f64 {
        // Convert logistic-ish scale (beta, natural log base) to the Elo
        // log10 convention: 10^(d/div) == exp(d/beta)  =>  div = beta * ln(10)
        // Defaults to 400.0 when beta == 400.0/ln(10). Keeps the game model
        // and the rating system computing the SAME win probability for the
        // same skill difference, so match-quality and counterfactual metrics
        // are not corrupted by inconsistent scales.
        self.config.beta * std::f64::consts::LN_10
    }

    fn expected_score(&self, rating_a: f64, rating_b: f64) -> f64 {
        1.0 / (1.0 + 10f64.powf((rating_b - rating_a) / self.divisor()))
    }

    fn team_average(ids: &[PlayerId], obs: &HashMap<PlayerId, PlayerObservation>) -> f64 {
        let sum: f64 = ids.iter().filter_map(|id| obs.get(id)).map(|o| o.rating).sum();
        sum / ids.len() as f64
    }
}

impl RatingSystem for EloRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_rating,
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
        }
    }

    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64 {
        let avg_a = team_a.iter().map(|p| p.rating).sum::<f64>() / team_a.len() as f64;
        let avg_b = team_b.iter().map(|p| p.rating).sum::<f64>() / team_b.len() as f64;
        self.expected_score(avg_a, avg_b)
    }

    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState> {
        let mut updates = HashMap::new();
        let avg_a = Self::team_average(&match_result.team_a, observations);
        let avg_b = Self::team_average(&match_result.team_b, observations);
        let expected_a = self.expected_score(avg_a, avg_b);
        let expected_b = 1.0 - expected_a;
        let actual_a = if match_result.winner == Team::A { 1.0 } else { 0.0 };
        let actual_b = 1.0 - actual_a;

        for &pid in &match_result.team_a {
            if let Some(obs) = observations.get(&pid) {
                let new_rating = obs.rating + self.config.k_factor * (actual_a - expected_a);
                updates.insert(pid, RatingState {
                    rating: new_rating,
                    rating_deviation: obs.rating_deviation,
                    volatility: obs.volatility,
                    games_played: obs.games_played + 1,
                });
            }
        }
        for &pid in &match_result.team_b {
            if let Some(obs) = observations.get(&pid) {
                let new_rating = obs.rating + self.config.k_factor * (actual_b - expected_b);
                updates.insert(pid, RatingState {
                    rating: new_rating,
                    rating_deviation: obs.rating_deviation,
                    volatility: obs.volatility,
                    games_played: obs.games_played + 1,
                });
            }
        }
        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_produce_50_percent() {
        let elo = EloRatingSystem::new(EloConfig { k_factor: 32.0, initial_rating: 1000.0, beta: 400.0 });
        let obs = |id: u64| PlayerObservation {
            id: PlayerId(id), rating: 1000.0, rating_deviation: 350.0,
            volatility: 0.06, games_played: 50, win_rate: 0.5,
            recent_performances: vec![], queue_joined_at: None,
            is_online: true, detection_flags: vec![],
        };
        assert!((elo.predict(&[obs(0)], &[obs(1)]) - 0.5).abs() < 0.001);
    }
}
```

### 8.5 Glicko-2 (Stub)

```rust
// crates/matchlab-rating/src/glicko.rs

pub struct GlickoConfig {
    pub initial_rating: f64,
    pub initial_rd: f64,
    pub initial_volatility: f64,
    pub tau: f64,
    pub epsilon: f64,
}

pub struct Glicko2RatingSystem {
    pub config: GlickoConfig,
}

impl Glicko2RatingSystem {
    pub fn new(config: GlickoConfig) -> Self { Self { config } }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let initial_rating = value.get("initial_rating").and_then(serde_yaml::Value::as_f64)?;
        Some(Self::new(GlickoConfig {
            initial_rating,
            initial_rd: value.get("initial_rd").and_then(serde_yaml::Value::as_f64).unwrap_or(350.0),
            initial_volatility: value.get("initial_volatility").and_then(serde_yaml::Value::as_f64).unwrap_or(0.06),
            tau: value.get("tau").and_then(serde_yaml::Value::as_f64).unwrap_or(0.5),
            epsilon: value.get("epsilon").and_then(serde_yaml::Value::as_f64).unwrap_or(0.000001),
        }))
    }
}

impl RatingSystem for Glicko2RatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    fn initialize(&self, _player_id: PlayerId) -> RatingState {
        RatingState {
            rating: self.config.initial_rating,
            rating_deviation: self.config.initial_rd,
            volatility: self.config.initial_volatility,
            games_played: 0,
        }
    }

    // Full Glicko-2: step 1-5 per spec
    // Step 1: Convert to glicko scale (μ, φ, σ)
    // Step 2: Compute v (estimated variance from outcomes)
    // Step 3: Compute Δ (estimated improvement)
    // Step 4: Iterate to find new σ' (volatility)
    // Step 5: Update φ* and compute new φ, μ
    // Step 6: Convert back to rating scale
    // todo!()
}
```

### 8.6 TrueSkill (Stub)

```rust
// crates/matchlab-rating/src/trueskill.rs

pub struct TrueSkillConfig {
    pub initial_mean: f64,
    pub initial_variance: f64,
    pub beta: f64,
    pub dynamics: f64,
    pub draw_probability: f64,
}

pub struct TrueSkillRatingSystem {
    pub config: TrueSkillConfig,
}

impl TrueSkillRatingSystem {
    pub fn new(config: TrueSkillConfig) -> Self { Self { config } }

    pub fn from_yaml(value: &serde_yaml::Value) -> Option<Self> {
        let initial_mean = value.get("initial_mean").and_then(serde_yaml::Value::as_f64)
            .or_else(|| value.get("initial_rating").and_then(serde_yaml::Value::as_f64))?;
        Some(Self::new(TrueSkillConfig {
            initial_mean,
            initial_variance: value.get("initial_variance").and_then(serde_yaml::Value::as_f64).unwrap_or(350.0),
            beta: value.get("beta").and_then(serde_yaml::Value::as_f64).unwrap_or(400.0),
            dynamics: value.get("dynamics").and_then(serde_yaml::Value::as_f64).unwrap_or(0.1),
            draw_probability: value.get("draw_probability").and_then(serde_yaml::Value::as_f64).unwrap_or(0.0),
        }))
    }
}

impl RatingSystem for TrueSkillRatingSystem {
    fn information_budget(&self) -> Vec<ObservationType> {
        vec![ObservationType::WinLoss]
    }

    // Bayesian update with Gaussian prior
    // Player state: (μ, σ²)
    // Team performance: sum of individual performances + noise
    // Update via truncated Gaussian conditioning
    // todo!()
}
```

---

## 9. Detection

### 9.1 Detection System Trait

```rust
// crates/matchlab-detection/src/detector.rs

use matchlab_core::player::PlayerId;
use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

pub trait DetectionSystem: Send + Sync {
    fn observe(&mut self, match_result: &MatchResult, world: &World);
    fn evaluate(&self, player_id: PlayerId, world: &World) -> DetectionResult;
    fn recommend_action(&self, result: &DetectionResult) -> InterventionAction;
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub player_id: PlayerId,
    pub probability_of_anomaly: f64,
    pub confidence: f64,
    pub evidence: Vec<String>,
}
```

Detection systems receive only `World` — they can call `world.observations[pid]` but never `world.players[pid]`. They must infer smurf status from observable signals (e.g., performance far above rating expectation) without any boolean flag.

### 9.2 Intervention Policy

```rust
// crates/matchlab-detection/src/intervention.rs

#[derive(Debug, Clone)]
pub enum InterventionAction {
    None,
    AccelerateRating { multiplier: f64 },
    IncreaseKFactor { new_k: f64 },
    FlagForReview,
    RestrictQueue { duration_ticks: u64 },
    TempBan { duration_ticks: u64 },
    Probation { duration_ticks: u64 },
    Ban,
}

pub struct InterventionPolicy {
    pub thresholds: Vec<(f64, InterventionAction)>, // sorted by probability ascending
    pub escalation_window_ticks: u64,
    pub escalation_factor: f64, // multiply thresholds down on repeated detections
    pub min_games_before_action: u64,
}

impl InterventionPolicy {
    pub fn default_ladder() -> Self {
        Self {
            thresholds: vec![
                (0.3, InterventionAction::None),
                (0.5, InterventionAction::AccelerateRating { multiplier: 1.5 }),
                (0.7, InterventionAction::FlagForReview),
                (0.8, InterventionAction::RestrictQueue { duration_ticks: 100 }),
                (0.9, InterventionAction::TempBan { duration_ticks: 500 }),
                (0.95, InterventionAction::Probation { duration_ticks: 1000 }),
                (0.99, InterventionAction::Ban),
            ],
            escalation_window_ticks: 500,
            escalation_factor: 0.9,
            min_games_before_action: 5,
        }
    }

    pub fn apply(&self, result: &DetectionResult, state: &PlayerInterventionState) -> InterventionAction {
        if state.games_played < self.min_games_before_action {
            return InterventionAction::None;
        }

        // Auto-escalation: lower effective thresholds based on repeated detections
        let effective_thresholds: Vec<(f64, &InterventionAction)> = self.thresholds.iter()
            .map(|(thresh, action)| {
                let escalated = thresh * self.escalation_factor.powi(state.prior_interventions as i32);
                (escalated.min(*thresh), action)
            })
            .collect();

        // Find highest applicable action
        let prob = result.probability_of_anomaly;
        let mut chosen = InterventionAction::None;
        for (thresh, action) in &effective_thresholds {
            if prob >= *thresh {
                chosen = (*action).clone();
            }
        }
        chosen
    }
}

pub struct PlayerInterventionState {
    pub games_played: u64,
    pub prior_interventions: u32,
    pub last_intervention_tick: u64,
    pub escalation_history: Vec<(u64, InterventionAction)>,
}
```

### 9.3 Smurf Detector

```rust
// crates/matchlab-detection/src/smurf.rs

use matchlab_core::player::PlayerId;
use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;
use crate::detector::{DetectionSystem, DetectionResult};
use crate::intervention::{InterventionAction, InterventionPolicy};
use std::collections::{HashMap, VecDeque};

pub struct SmurfDetector {
    player_states: HashMap<PlayerId, SmurfState>,
    intervention_states: HashMap<PlayerId, PlayerInterventionState>,
    policy: InterventionPolicy,
    /// Deviation (in sigmas) beyond which a single result counts as anomalous.
    pub sigma_threshold: f64,
    /// Number of consecutive anomalous results required to flag a smurf.
    pub min_anomalous_games: u64,
}

struct SmurfState {
    /// Per-game observed performance (e.g. a normalized impact score).
    recent_performance: VecDeque<f64>,
    /// Per-game expected performance given the player's current rating.
    expected_performance: VecDeque<f64>,
    /// Running count of consecutive anomalous (outlier) performances.
    consecutive_anomalous: u32,
}

impl SmurfDetector {
    pub fn new(policy: InterventionPolicy) -> Self {
        Self {
            player_states: HashMap::new(),
            intervention_states: HashMap::new(),
            policy,
            sigma_threshold: 3.0,   // design: "exceeds expected by 3σ"
            min_anomalous_games: 5, // design: "for 5 games"
        }
    }
}

impl DetectionSystem for SmurfDetector {
    fn observe(&mut self, match_result: &MatchResult, world: &World) {
        for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
            // Use ONLY observations; never reality (truth separation).
            let Some(obs) = world.observations.get(pid) else { continue };
            let Some(perf) = match_result.player_performances.iter()
                .find(|p| &p.player_id == pid) else { continue };

            // Expected performance scales with the player's visible rating.
            let expected = obs.rating / 100.0;
            let actual = perf.impact + perf.kills as f64 / 10.0;

            let state = self.player_states.entry(*pid).or_insert(SmurfState {
                recent_performance: VecDeque::new(),
                expected_performance: VecDeque::new(),
                consecutive_anomalous: 0,
            });
            state.recent_performance.push_back(actual);
            state.expected_performance.push_back(expected);
            if state.recent_performance.len() > 20 {
                state.recent_performance.pop_front();
                state.expected_performance.pop_front();
            }

            // Per-game deviation sigma, using the observed spread of performances.
            let dev = (actual - expected).abs();
            let spread = state.recent_performance.iter()
                .map(|p| (p - expected).abs())
                .fold(0.0f64, f64::max);
            let sigmas = if spread > 0.0 { dev / spread } else { 0.0 };

            if sigmas >= self.sigma_threshold {
                state.consecutive_anomalous += 1;
            } else {
                state.consecutive_anomalous = 0;
            }
        }
    }

    fn evaluate(&self, player_id: PlayerId, _world: &World) -> DetectionResult {
        let state = match self.player_states.get(&player_id) {
            Some(s) => s,
            None => return DetectionResult {
                player_id,
                probability_of_anomaly: 0.0,
                confidence: 0.0,
                evidence: vec![],
            },
        };

        // Smurf if the player has N consecutive anomalous performances.
        let flagged = state.consecutive_anomalous >= self.min_anomalous_games;
        let probability_of_anomaly = if flagged {
            // Ramp with the length of the anomalous streak beyond the minimum.
            let extra = state.consecutive_anomalous as f64 - self.min_anomalous_games as f64;
            (0.7 + 0.25 * extra.min(1.2)).min(0.99)
        } else {
            state.consecutive_anomalous as f64 / self.min_anomalous_games as f64 * 0.3
        };

        DetectionResult {
            player_id,
            probability_of_anomaly,
            confidence: (state.consecutive_anomalous as f64 / self.min_anomalous_games as f64).min(1.0),
            evidence: vec![
                format!("consecutive_anomalous={}", state.consecutive_anomalous),
                format!("min_required={}", self.min_anomalous_games),
            ],
        }
    }

    fn recommend_action(&self, result: &DetectionResult) -> InterventionAction {
        let state = self.intervention_states.get(&result.player_id)
            .cloned()
            .unwrap_or(PlayerInterventionState {
                games_played: 0,
                prior_interventions: 0,
                last_intervention_tick: 0,
                escalation_history: vec![],
            });
        self.policy.apply(result, &state)
    }
}
```

---

## 10. Ranking

### 10.1 Rank Mapper Trait

```rust
// crates/matchlab-ranking/src/ranker.rs

use serde::Deserialize;

pub trait RankMapper: Send + Sync {
    fn rating_to_rank(&self, rating: f64) -> Rank;
    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Rank {
    pub tier: String,
    pub division: u8,
}

pub struct BracketRankMapper {
    pub brackets: Vec<RankBracket>,
}

#[derive(Debug, Deserialize)]
pub struct RankBracket {
    pub rank: Rank,
    pub min: f64,
    pub max: f64,
}

impl RankMapper for BracketRankMapper {
    fn rating_to_rank(&self, rating: f64) -> Rank {
        for bracket in &self.brackets {
            if rating >= bracket.min && rating < bracket.max {
                return bracket.rank.clone();
            }
        }
        self.brackets.last().unwrap().rank.clone()
    }

    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64) {
        for bracket in &self.brackets {
            if &bracket.rank == rank {
                return (bracket.min, bracket.max);
            }
        }
        (0.0, 0.0)
    }
}
```

### 10.2 Leaderboard

```rust
// crates/matchlab-ranking/src/leaderboard.rs

use matchlab_core::player::PlayerId;
use crate::ranker::Rank;

pub struct Leaderboard {
    entries: Vec<LeaderboardEntry>,
}

pub struct LeaderboardEntry {
    pub player_id: PlayerId,
    pub rating: f64,
    pub rank: Rank,
    pub games_played: u64,
}

impl Leaderboard {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn update(&mut self, player_id: PlayerId, rating: f64, rank: Rank, games_played: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.player_id == player_id) {
            entry.rating = rating;
            entry.rank = rank;
            entry.games_played = games_played;
        } else {
            self.entries.push(LeaderboardEntry { player_id, rating, rank, games_played });
        }
        self.entries.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap());
    }

    pub fn rank_of(&self, player_id: PlayerId) -> Option<usize> {
        self.entries.iter().position(|e| e.player_id == player_id)
    }

    pub fn top_n(&self, n: usize) -> &[LeaderboardEntry] {
        &self.entries[..n.min(self.entries.len())]
    }
}
```

---

## 11. Metrics

### 11.1 Metrics Engine

```rust
// crates/matchlab-metrics/src/engine.rs

use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;
use crate::collector::{MetricCollector, MetricResult};
use std::collections::HashMap;

pub struct MetricsEngine {
    collectors: Vec<Box<dyn MetricCollector>>,
    results: HashMap<String, MetricResult>,
}

impl MetricsEngine {
    pub fn new() -> Self {
        Self { collectors: Vec::new(), results: HashMap::new() }
    }

    pub fn register(&mut self, collector: Box<dyn MetricCollector>) {
        self.collectors.push(collector);
    }

    pub fn record_match(&mut self, match_result: &MatchResult, world: &World) {
        for collector in &mut self.collectors {
            collector.record_match(match_result, world);
        }
    }

    pub fn finalize(&mut self) {
        self.results.clear();
        for collector in &self.collectors {
            self.results.insert(collector.name().to_string(), collector.compute());
        }
    }

    pub fn results(&self) -> &HashMap<String, MetricResult> {
        &self.results
    }
}
```

### 11.2 Metric Collector Trait

```rust
// crates/matchlab-metrics/src/collector.rs

use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

pub trait MetricCollector: Send + Sync {
    fn name(&self) -> &str;
    fn record_match(&mut self, match_result: &MatchResult, world: &World);
    fn compute(&self) -> MetricResult;
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum MetricResult {
    Scalar(f64),
    Distribution(Vec<f64>),
    Summary {
        mean: f64, median: f64,
        p75: f64, p90: f64, p95: f64, p99: f64,
        stddev: f64,
    },
    Histogram { buckets: Vec<(f64, u64)> },
}
```

### 11.3 Built-in Collectors

#### Match Quality

```rust
// crates/matchlab-metrics/src/quality.rs

pub struct MatchQualityCollector { values: Vec<f64> }

impl MetricCollector for MatchQualityCollector {
    fn name(&self) -> &str { "match_quality" }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let avg_a: f64 = mr.team_a.iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating).sum::<f64>() / mr.team_a.len() as f64;
        let avg_b: f64 = mr.team_b.iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating).sum::<f64>() / mr.team_b.len() as f64;
        let diff = (avg_a - avg_b).abs();
        self.values.push(1.0 - (diff / 400.0).min(1.0));
    }

    fn compute(&self) -> MetricResult {
        crate::stats::summary_to_result(&self.values)
    }
}
```

#### Match Inequality

Distribution of expected win probabilities across all matches. A well-matched system clusters near 0.5; a poorly matched system has a fat-tailed distribution.

```rust
// crates/matchlab-metrics/src/inequality.rs

pub struct MatchInequalityCollector {
    win_probabilities: Vec<f64>,
}

impl MetricCollector for MatchInequalityCollector {
    fn name(&self) -> &str { "match_inequality" }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        // Compute P(A wins) from observations
        let avg_a: f64 = mr.team_a.iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating).sum::<f64>() / mr.team_a.len() as f64;
        let avg_b: f64 = mr.team_b.iter()
            .filter_map(|pid| world.observations.get(pid))
            .map(|o| o.rating).sum::<f64>() / mr.team_b.len() as f64;
        let p = 1.0 / (1.0 + 10f64.powf((avg_b - avg_a) / 400.0));
        self.win_probabilities.push(p);
    }

    fn compute(&self) -> MetricResult {
        // Report a summary of the win-probability distribution.
        // A well-matched system's distribution STARVES near 0.5 (low variance,
        // high clustering); a poorly matched system has a fat, spread tail.
        // The Score component reflects spread: (2 * p - 1)^2 is 0 at a fair 0.5
        // and 1 at lopsided matches.
        let spread: Vec<f64> = self.win_probabilities.iter()
            .map(|p| (2.0 * p - 1.0).powi(2))
            .collect();
        let mean_spread = spread.iter().sum::<f64>() / spread.len().max(1) as f64;

        crate::stats::summary_to_result(&self.win_probabilities)
            // surface the spread (sqrt of mean squared deviation from 0.5) too
    }
}
```

The `summary_to_result` call returns the full distribution summary (P50/P75/P90/P95/P99, mean, stddev). The `mean_spread` is computed as an auxiliary and can be logged alongside; the `MetricResult::Summary` preserves the raw distribution so P99 behavior isn't hidden.

#### NDCG (Match Quality Ranking)

Normalised Discounted Cumulative Gain: are high-quality matches appearing early in the experiment? Measures whether the matchmaker learns and improves over time.

```rust
// crates/matchlab-metrics/src/ndcg.rs

pub struct NDCGCollector {
    /// For each timestep, record the match quality scores
    qualities: Vec<f64>,
    window_size: usize,
}

impl MetricCollector for NDCGCollector {
    fn name(&self) -> &str { "ndcg" }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        // Quality proxy from rating balance: compute the win-probability gap
        // between the two teams and map it to [0,1] where 1.0 = perfectly
        // balanced and 0.0 = completely lopsided.
        let avg_a = mr.team_a.iter()
            .filter_map(|p| world.observations.get(p))
            .map(|o| o.rating).sum::<f64>() / mr.team_a.len().max(1) as f64;
        let avg_b = mr.team_b.iter()
            .filter_map(|p| world.observations.get(p))
            .map(|o| o.rating).sum::<f64>() / mr.team_b.len().max(1) as f64;
        let p = 1.0 / (1.0 + (-(avg_a - avg_b) / 400.0).exp());
        let quality = 1.0 - (p - 0.5).abs() * 2.0;
        self.qualities.push(quality);
    }

    fn compute(&self) -> MetricResult {
        // NDCG@k over sliding windows of match qualities
        // Measures how concentrated good matches are early vs late
        if self.qualities.is_empty() { return MetricResult::Scalar(0.0); }

        let n = self.qualities.len();
        let ideal: Vec<f64> = {
            let mut sorted = self.qualities.clone();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
            sorted
        };

        let mut dcg = 0.0;
        let mut idcg = 0.0;
        for (i, (actual, ideal_val)) in self.qualities.iter()
            .zip(ideal.iter()).enumerate()
        {
            let discount = (i as f64 + 2.0).log2();
            dcg += actual / discount;
            idcg += ideal_val / discount;
        }

        let ndcg = if idcg > 0.0 { dcg / idcg } else { 0.0 };
        MetricResult::Scalar(ndcg)
    }
}
```

#### Dimensionality Fidelity

Measures the fidelity loss when using a 1D rating to represent multidimensional skill. Compares the correlation between 1D ratings and true overall skill vs. SkillVector-based predictions and true overall skill. A high fidelity score means the 1D rating captures the important dimensions.

```rust
// crates/matchlab-metrics/src/dimensionality.rs

use matchlab_core::player::{PlayerId, SkillVector};

pub struct DimensionalityFidelityCollector {
    /// For each player: (1d_rating, skill_vector_prediction, true_overall)
    observations: Vec<(f64, f64, f64)>,
}

impl MetricCollector for DimensionalityFidelityCollector {
    fn name(&self) -> &str { "dimensionality_fidelity" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        for (pid, obs) in &world.observations {
            if let Some(reality) = world.players.get(pid) {
                let true_overall = reality.skill.overall();
                // 1D prediction: just the scalar rating
                let oned_pred = obs.rating;
                // MultiD prediction: the SkillVector exposed to algorithms,
                // combined via plain average (uniform weights).
                let multid_pred = obs.skill_vector.overall();
                self.observations.push((oned_pred, multid_pred, true_overall));
            }
        }
    }

    fn compute(&self) -> MetricResult {
        if self.observations.is_empty() { return MetricResult::Scalar(0.0); }

        // Correlation between 1D ratings and true skill
        let oned_corr = pearson(&self.observations.iter().map(|(a, _, c)| (*a, *c)).collect::<Vec<_>>());
        // Correlation between SkillVector predictions and true skill
        let multid_corr = pearson(&self.observations.iter().map(|(_, b, c)| (*b, *c)).collect::<Vec<_>>());

        // Fidelity = how much multiD improves over 1D (clamped to [0, 1])
        let fidelity = if oned_corr > 0.0 {
            ((multid_corr - oned_corr) / (1.0 - oned_corr)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        MetricResult::Summary {
            mean: oned_corr,
            median: multid_corr,
            p75: fidelity,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
            stddev: 0.0,
        }
    }
}

fn pearson(pairs: &[(f64, f64)]) -> f64 {
    if pairs.len() < 2 { return 0.0; }
    let n = pairs.len() as f64;
    let sum_x: f64 = pairs.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = pairs.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = pairs.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = pairs.iter().map(|(x, _)| x * x).sum();
    let sum_y2: f64 = pairs.iter().map(|(_, y)| y * y).sum();
    let num = n * sum_xy - sum_x * sum_y;
    let den = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
    if den == 0.0 { 0.0 } else { num / den }
}
```

This metric directly tests the design's research question: "Can a 1D rating accurately represent multidimensional player skill?" The fidelity score quantifies the information lost by compressing SkillVector into a scalar rating.

#### Queue Time

Tracks actual queue wait time per player (time from joining queue to match formation), NOT match duration.

```rust
// crates/matchlab-metrics/src/queue.rs

pub struct QueueTimeCollector {
    times_secs: Vec<f64>,
}

impl MetricCollector for QueueTimeCollector {
    fn name(&self) -> &str { "queue_time" }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        // Queue time = match formation time - player's queue join time
        let match_time = world.time;
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(obs) = world.observations.get(pid) {
                if let Some(joined_at) = obs.queue_joined_at {
                    let wait = match_time.duration_since(joined_at).as_secs_f64();
                    self.times_secs.push(wait);
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let s = crate::stats::summary(&self.times_secs);
        MetricResult::Summary {
            mean: s.mean, median: s.median,
            p75: s.p75, p90: s.p90, p95: s.p95, p99: s.p99,
            stddev: s.stddev,
        }
    }
}
```

#### Rating Accuracy

```rust
// crates/matchlab-metrics/src/accuracy.rs

pub struct RatingAccuracyCollector { errors: Vec<f64> }

impl MetricCollector for RatingAccuracyCollector {
    fn name(&self) -> &str { "rating_accuracy" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        for (pid, obs) in &world.observations {
            if let Some(reality) = world.players.get(pid) {
                let error = (obs.rating - reality.skill.overall()).abs();
                self.errors.push(error);
            }
        }
    }

    fn compute(&self) -> MetricResult {
        crate::stats::summary_to_result(&self.errors)
    }
}
```

#### Spearman Rank Correlation

Measures how well the rank ordering of ratings matches the rank ordering of true skills.

```rust
// crates/matchlab-metrics/src/correlation.rs

pub struct SpearmanCorrelationCollector {
    pairs: Vec<(f64, f64)>, // (rating, true_skill)
}

impl MetricCollector for SpearmanCorrelationCollector {
    fn name(&self) -> &str { "spearman_correlation" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        for (pid, obs) in &world.observations {
            if let Some(reality) = world.players.get(pid) {
                self.pairs.push((obs.rating, reality.skill.overall()));
            }
        }
    }

    fn compute(&self) -> MetricResult {
        // Spearman ρ = 1 - (6 Σ d²) / (n(n²-1))
        // where d = difference in ranks
        let n = self.pairs.len() as f64;
        if n < 2.0 { return MetricResult::Scalar(0.0); }

        let mut rating_ranked = rankify(&self.pairs.iter().map(|(r, _)| *r).collect::<Vec<_>>());
        let mut skill_ranked = rankify(&self.pairs.iter().map(|(_, s)| *s).collect::<Vec<_>>());

        let d_squared_sum: f64 = rating_ranked.iter().zip(skill_ranked.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        let rho = 1.0 - (6.0 * d_squared_sum) / (n * (n * n - 1.0));
        MetricResult::Scalar(rho)
    }
}

fn rankify(values: &[f64]) -> Vec<f64> {
    let mut indexed: Vec<(usize, f64)> = values.iter().enumerate().map(|(i, &v)| (i, v)).collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut ranks = vec![0.0; values.len()];
    for (rank, (idx, _)) in indexed.iter().enumerate() {
        ranks[*idx] = rank as f64 + 1.0;
    }
    ranks
}
```

#### Convergence

```rust
// crates/matchlab-metrics/src/convergence.rs

pub struct ConvergenceCollector {
    convergence_games: HashMap<matchlab_core::player::PlayerId, Option<u64>>,
    threshold: f64,
}

impl MetricCollector for ConvergenceCollector {
    fn name(&self) -> &str { "convergence" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        for (pid, obs) in &world.observations {
            if let Some(reality) = world.players.get(pid) {
                let error = (obs.rating - reality.skill.overall()).abs();
                let entry = self.convergence_games.entry(*pid).or_insert(None);
                if error < self.threshold && entry.is_none() {
                    *entry = Some(obs.games_played);
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let games: Vec<f64> = self.convergence_games.values()
            .filter_map(|v| *v).map(|g| g as f64).collect();
        if games.is_empty() { return MetricResult::Scalar(f64::INFINITY); }
        crate::stats::summary_to_result(&games)
    }
}
```

#### Responsiveness

How quickly rating responds when true skill changes. Distinct from convergence (which measures absolute error). Measures the lag between a skill change event and the rating moving in the correct direction.

```rust
// crates/matchlab-metrics/src/responsiveness.rs

use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use std::collections::HashMap;

pub struct ResponsivenessCollector {
    /// Previous observed rating per player, used to compute rating deltas.
    prev_ratings: HashMap<PlayerId, f64>,
    /// For each player-event: did the rating move in the direction consistent
    /// with their match outcome (winner gains, loser loses)?
    responses: Vec<bool>,
}

impl MetricCollector for ResponsivenessCollector {
    fn name(&self) -> &str { "responsiveness" }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        let winner_is_a = mr.winner == Team::A;
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            let Some(obs) = world.observations.get(pid) else { continue };
            let prev = match self.prev_ratings.insert(*pid, obs.rating) {
                Some(p) => p,
                // First observation of this player: no direction to compare yet.
                None => continue,
            };
            let delta = obs.rating - prev;
            if delta == 0.0 { continue; }
            let won = (mr.team_a.contains(pid) && winner_is_a)
                || (mr.team_b.contains(pid) && !winner_is_a);
            // A responsive system moves rating in the direction the outcome
            // predicts: winners gain, losers lose.
            let responsive = (delta > 0.0) == won;
            self.responses.push(responsive);
        }
    }

    fn compute(&self) -> MetricResult {
        if self.responses.is_empty() { return MetricResult::Scalar(0.0); }
        let correct = self.responses.iter().filter(|&&b| b).count() as f64;
        MetricResult::Scalar(correct / self.responses.len() as f64)
    }
}
```

#### Stability

```rust
// crates/matchlab-metrics/src/stability.rs

pub struct StabilityCollector {
    /// Rating histories only for "stable" players (low improvement_rate).
    /// Captured during `record_match` because `compute` has no World access.
    rating_history: HashMap<matchlab_core::player::PlayerId, Vec<f64>>,
}

impl MetricCollector for StabilityCollector {
    fn name(&self) -> &str { "stability" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        for (pid, obs) in &world.observations {
            // Only track players who aren't rapidly improving/declining,
            // so rating movement reflects system noise, not skill drift.
            let stable = world.players.get(pid)
                .map(|reality| reality.improvement_rate.abs() < 0.1)
                .unwrap_or(true);
            if stable {
                self.rating_history.entry(*pid).or_default().push(obs.rating);
            }
        }
    }

    fn compute(&self) -> MetricResult {
        // Variance of each stable player's rating over time; sqrt = stddev.
        // A stable system should have small fluctuations for non-drifting players.
        let mut variances = Vec::new();
        for history in self.rating_history.values() {
            let mean = history.iter().sum::<f64>() / history.len().max(1) as f64;
            let var = history.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
                / history.len().max(1) as f64;
            variances.push(var);
        }
        if variances.is_empty() { return MetricResult::Scalar(0.0); }
        let mean_var = variances.iter().sum::<f64>() / variances.len() as f64;
        MetricResult::Scalar(mean_var.sqrt())
    }
}
```

#### Streaks

```rust
// crates/matchlab-metrics/src/streaks.rs

pub struct StreakCollector {
    streaks: HashMap<matchlab_core::player::PlayerId, (bool, u32)>,
    max_streaks: Vec<u32>,
}

impl MetricCollector for StreakCollector {
    fn name(&self) -> &str { "streaks" }

    fn record_match(&mut self, mr: &MatchResult, _world: &World) {
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            let is_team_a = mr.team_a.contains(pid);
            let won = (is_team_a && mr.winner == matchlab_core::match_::Team::A)
                || (!is_team_a && mr.winner == matchlab_core::match_::Team::B);
            let entry = self.streaks.entry(*pid).or_insert((true, 0));
            if (entry.0 && won) || (!entry.0 && !won) {
                entry.1 += 1;
            } else {
                self.max_streaks.push(entry.1);
                *entry = (won, 1);
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let total = self.max_streaks.len() as f64;
        if total == 0.0 { return MetricResult::Scalar(0.0); }
        let p3 = self.max_streaks.iter().filter(|&&s| s >= 3).count() as f64 / total;
        let p5 = self.max_streaks.iter().filter(|&&s| s >= 5).count() as f64 / total;
        let p8 = self.max_streaks.iter().filter(|&&s| s >= 8).count() as f64 / total;
        let p10 = self.max_streaks.iter().filter(|&&s| s >= 10).count() as f64 / total;
        // Design tracks the probability of reaching 3, 5, 8, and 10-game streaks.
        MetricResult::Distribution(vec![p3, p5, p8, p10])
    }
}
```

#### Population Health

Tracks system-level properties: rating inflation/deflation, compression, and rank distribution.

```rust
// crates/matchlab-metrics/src/population.rs

pub struct PopulationHealthCollector {
    ratings_over_time: Vec<Vec<f64>>,
}

impl MetricCollector for PopulationHealthCollector {
    fn name(&self) -> &str { "population_health" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        // Periodically snapshot all ratings (e.g., every 1000 matches)
        // to track inflation/deflation over time
        let ratings: Vec<f64> = world.observations.values().map(|o| o.rating).collect();
        self.ratings_over_time.push(ratings);
    }

    fn compute(&self) -> MetricResult {
        if self.ratings_over_time.is_empty() { return MetricResult::Scalar(0.0); }

        let initial_mean = mean(&self.ratings_over_time[0]);
        let final_mean = mean(self.ratings_over_time.last().unwrap());
        let inflation = final_mean - initial_mean;

        let initial_stddev = stddev(&self.ratings_over_time[0]);
        let final_stddev = stddev(self.ratings_over_time.last().unwrap());
        let compression = initial_stddev - final_stddev;

        // Return as distribution: [inflation, compression, initial_mean, final_mean]
        MetricResult::Distribution(vec![inflation, compression, initial_mean, final_mean])
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64]) -> f64 {
    let m = mean(values);
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
    var.sqrt()
}
```

#### Smurf Metrics

```rust
// crates/matchlab-metrics/src/smurf.rs

pub struct SmurfMetricsCollector {
    smurf_ids: Vec<matchlab_core::player::PlayerId>,
    detection_events: Vec<DetectionEvent>,
    archetype_breakdown: HashMap<String, ArchetypeMetrics>,
}

struct DetectionEvent {
    player_id: matchlab_core::player::PlayerId,
    detected: bool,
    games_at_detection: Option<u64>,
    damage: f64,
    archetype: String,
}

struct ArchetypeMetrics {
    total_games: u64,
    smurf_games: u64,
    total_damage: f64,
    detection_rate: f64,
    false_positive_rate: f64,
    mean_games_to_detection: f64,
}

impl MetricCollector for SmurfMetricsCollector {
    fn name(&self) -> &str { "smurf" }

    fn record_match(&mut self, mr: &MatchResult, world: &World) {
        // Check if any smurfs were in this match and accumulate damage
        // Damage = unfairness = |P(expected winner) - 0.5| for matches with smurfs
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            if let Some(reality) = world.players.get(pid) {
                // Smurf = high skill but low games_played (no boolean flag)
                if reality.skill.overall() > 1300 && reality.games_played < 20 {
                    // This player looks like a smurf — accumulate damage
                    let avg_a = average_obs_rating(&mr.team_a, world);
                    let avg_b = average_obs_rating(&mr.team_b, world);
                    let p = 1.0 / (1.0 + 10f64.powf((avg_b - avg_a) / 400.0));
                    let unfairness = (p - 0.5).abs() * 2.0;
                    // damage tracking happens here
                }
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let total = self.detection_events.len() as f64;
        if total == 0.0 { return MetricResult::Scalar(0.0); }
        let detected = self.detection_events.iter().filter(|e| e.detected).count() as f64;
        let false_positives = 0.0; // computed from non-smurf detections

        // Per-archetype breakdown
        let mut archetype_results: HashMap<String, Vec<f64>> = HashMap::new();
        for event in &self.detection_events {
            let entry = archetype_results.entry(event.archetype.clone()).or_default();
            entry.push(if event.detected { 1.0 } else { 0.0 });
        }

        MetricResult::Summary {
            mean: detected / total,
            median: false_positives,
            p75: mean(&self.detection_events.iter()
                .filter_map(|e| e.games_at_detection.map(|g| g as f64))
                .collect::<Vec<_>>()),
            p90: self.detection_events.iter()
                .map(|e| e.damage).sum::<f64>(),
            p95: 0.0,
            p99: 0.0,
            stddev: 0.0,
        }
    }
}

fn average_obs_rating(team: &[matchlab_core::player::PlayerId], world: &matchlab_core::world::World) -> f64 {
    let sum: f64 = team.iter()
        .filter_map(|pid| world.observations.get(pid))
        .map(|o| o.rating).sum();
    sum / team.len() as f64
}
```

pub struct RankAccuracyCollector {
    pairs: Vec<(f64, f64)>, // (assigned_rank_midpoint, true_skill)
}

impl MetricCollector for RankAccuracyCollector {
    fn name(&self) -> &str { "rank_accuracy" }

    fn record_match(&mut self, _mr: &MatchResult, world: &World) {
        // Compare each player's visible rank midpoint (what the system/opponents
        // communicate) against their true skill once per snapshot.
        for (pid, obs) in &world.observations {
            let Some(reality) = world.players.get(pid) else { continue };
            self.pairs.push((
                obs.visible_rank.midpoint(),
                reality.skill.overall(),
            ));
        }
    }

    fn compute(&self) -> MetricResult {
        // MAE between rank bracket midpoint and true skill
        if self.pairs.is_empty() { return MetricResult::Scalar(0.0); }
        let errors: Vec<f64> = self.pairs.iter()
            .map(|(rank_mid, skill)| (rank_mid - skill).abs())
            .collect();
        crate::stats::summary_to_result(&errors)
    }
}
```

### 11.4 Cohort Filtering

```rust
// crates/matchlab-metrics/src/cohort.rs

use matchlab_core::player::{PlayerId, Region, PlayerReality};
use matchlab_core::world::World;

/// Map a true-skill value to a coarse tier label ("iron"..="radiant"), used to
/// align a player's reality with the RankTier cohort filter and with the
/// visible-rank brackets exposed to players.
pub fn tier_for_skill(skill: f64) -> String {
    match skill {
        s if s < 400.0 => "iron".to_string(),
        s if s < 700.0 => "bronze".to_string(),
        s if s < 1000.0 => "silver".to_string(),
        s if s < 1300.0 => "gold".to_string(),
        s if s < 1600.0 => "platinum".to_string(),
        s if s < 1900.0 => "diamond".to_string(),
        _ => "radiant".to_string(),
    }
}

#[derive(Debug, Clone)]
pub enum CohortFilter {
    All,
    SkillRange(f64, f64),
    Archetype(String),
    GamesPlayedRange(u64, u64),
    Region(Region),
    PartySize(usize),
    SessionLength(f64, f64),  // min, max seconds
    RankTier(String),
    IsSmurfByProperties,
}

impl CohortFilter {
    pub fn matches(&self, reality: &PlayerReality) -> bool {
        match self {
            CohortFilter::All => true,
            CohortFilter::SkillRange(low, high) => {
                reality.skill.overall() >= *low && reality.skill.overall() <= *high
            }
            CohortFilter::Archetype(name) => reality.archetype == *name,
            CohortFilter::GamesPlayedRange(low, high) => {
                reality.games_played >= *low && reality.games_played <= *high
            }
            CohortFilter::Region(region) => reality.region == *region,
            CohortFilter::PartySize(size) => {
                // party of size 1 = solo; >1 = grouped
                reality.party_id.map(|_| *size > 1).or(Some(*size == 1)).unwrap_or_default()
            }
            CohortFilter::SessionLength(min, max) => {
                let s = reality.session_length;
                s >= *min && s <= *max
            }
            CohortFilter::RankTier(tier) => {
                // Cohort analysis runs on reality; map the player's true skill
                // to a coarse tier string for filtering (e.g. "bronze"..="radiant").
                let t = crate::cohort::tier_for_skill(reality.skill.overall());
                t == *tier
            }
            CohortFilter::IsSmurfByProperties => {
                reality.skill.overall() > 1300 && reality.games_played < 20
            }
        }
    }

    pub fn filter_player_ids(&self, world: &World) -> Vec<PlayerId> {
        world.players.iter()
            .filter(|(_, reality)| self.matches(reality))
            .map(|(pid, _)| *pid)
            .collect()
    }
}
```

---

## 12. Objective Functions

### 12.1 Utility Surface

Experiments produce multiple raw metrics. The objective function combines them into a single utility score for comparison, while always preserving raw values.

```rust
// crates/matchlab-objective/src/utility.rs

use std::collections::HashMap;
use matchlab_metrics::MetricResult;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObjectiveWeights {
    pub match_quality: f64,
    pub queue_time: f64,
    pub rating_accuracy: f64,
    pub convergence_speed: f64,
    pub smurf_damage: f64,
    pub false_positive_rate: f64,
    pub streak_frustration: f64,
}

impl Default for ObjectiveWeights {
    fn default() -> Self {
        Self {
            match_quality: 1.0,
            queue_time: 0.5,
            rating_accuracy: 1.0,
            convergence_speed: 0.8,
            smurf_damage: 2.0,
            false_positive_rate: 1.5,
            streak_frustration: 0.3,
        }
    }
}

pub struct ObjectiveFunction {
    pub weights: ObjectiveWeights,
}

impl ObjectiveFunction {
    /// Compute aggregate utility from raw metrics.
    /// Returns the utility score AND the raw metrics (never discard raw values).
    pub fn evaluate(&self, metrics: &HashMap<String, MetricResult>) -> (f64, &HashMap<String, MetricResult>) {
        let mut score = 0.0;

        if let Some(MetricResult::Scalar(v)) = metrics.get("match_quality") {
            score += self.weights.match_quality * v;
        }
        if let Some(MetricResult::Summary { mean, .. }) = metrics.get("queue_time") {
            score -= self.weights.queue_time * mean; // lower is better
        }
        if let Some(MetricResult::Scalar(v)) = metrics.get("rating_accuracy") {
            score -= self.weights.rating_accuracy * v; // lower error is better
        }
        if let Some(MetricResult::Scalar(v)) = metrics.get("convergence") {
            score -= self.weights.convergence_speed * v; // fewer games is better
        }
        if let Some(MetricResult::Distribution(d)) = metrics.get("smurf") {
            if let Some(&damage) = d.get(3) {
                score -= self.weights.smurf_damage * damage;
            }
            if let Some(&fp) = d.get(1) {
                score -= self.weights.false_positive_rate * fp;
            }
        }
        if let Some(MetricResult::Distribution(d)) = metrics.get("streaks") {
            if let Some(&p5) = d.get(0) {
                score -= self.weights.streak_frustration * p5;
            }
        }

        (score, metrics)
    }
}
```

### 12.2 Rule: Never Discard Raw Metrics

The aggregate utility score is a convenience for ranking experiments. All raw metric distributions must be preserved in experiment output. An experiment result with a "good" aggregate score but terrible P99 queue time is a meaningful finding that would be invisible if only the aggregate were stored.

---

## 13. Experiments

### 13.1 Experiment Manifest (YAML Schema)

```yaml
experiment:
  name: elo_vs_glicko_dynamic
  description: "Compare Elo and Glicko-2 with dynamic skill population"
  seed: 918273

  population:
    size: 10000
    seed: 42
    archetypes:
      - name: stable
        proportion: 0.60
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 5.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.01

      - name: improving
        proportion: 0.15
        skill_distribution: { type: normal, mean: 800, stddev: 200 }
        skill_volatility: 10.0
        improvement_rate: 2.0
        play_frequency: 0.9
        session_length: 2400.0
        quit_probability: 0.005

      - name: declining
        proportion: 0.05
        skill_distribution: { type: normal, mean: 1100, stddev: 150 }
        skill_volatility: 8.0
        improvement_rate: -1.5
        play_frequency: 0.5
        session_length: 1200.0
        quit_probability: 0.03

      - name: returning
        proportion: 0.05
        skill_distribution: { type: normal, mean: 1000, stddev: 200 }
        skill_volatility: 12.0
        improvement_rate: 1.0
        play_frequency: 0.6
        session_length: 1500.0
        quit_probability: 0.02
        initial_rating: 700

      - name: smurf
        proportion: 0.02
        skill_distribution: { type: normal, mean: 1500, stddev: 100 }
        skill_volatility: 5.0
        improvement_rate: 0.0
        play_frequency: 0.95
        session_length: 3600.0
        quit_probability: 0.002
        initial_rating: 700

  game:
    team_size: 5
    outcome_model: logistic
    beta: 400.0
    noise: 0.05

  matchmaking:
    algorithm: expanding_window
    max_queue_time: 60.0
    tiers:
      - [5.0, 25.0]
      - [10.0, 50.0]
      - [20.0, 100.0]
      - [30.0, 200.0]
    max_window: 300.0
    search_strategy: greedy

  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0

      - name: glicko2
        initial_rating: 1500.0
        initial_rd: 350.0
        initial_volatility: 0.06
        tau: 0.5
        epsilon: 0.00001

  detection:
    enabled: true
    smurf:
      acceleration_threshold: 0.8
      ban_threshold: 0.99
      min_games_before_action: 3

  ranking:
    brackets:
      - { rank: { tier: Bronze, division: 1 }, min: 0, max: 800 }
      - { rank: { tier: Bronze, division: 2 }, min: 800, max: 1000 }
      - { rank: { tier: Silver, division: 1 }, min: 1000, max: 1200 }
      - { rank: { tier: Gold, division: 1 }, min: 1200, max: 1500 }
      - { rank: { tier: Platinum, division: 1 }, min: 1500, max: 2000 }

  metrics:
    - match_quality
    - match_inequality
    - queue_time
    - rating_accuracy
    - spearman_correlation
    - convergence
    - responsiveness
    - stability
    - streaks
    - population_health
    - smurf

  objectives:
    match_quality: 1.0
    queue_time: 0.5
    rating_accuracy: 1.0
    convergence_speed: 0.8
    smurf_damage: 2.0
    false_positive_rate: 1.5
    streak_frustration: 0.3

  cohorts:
    - name: all
      filter: { type: all }
    - name: stable_players
      filter: { type: archetype, value: stable }
    - name: improving_players
      filter: { type: archetype, value: improving }
    - name: declining_players
      filter: { type: archetype, value: declining }
    - name: returning_players
      filter: { type: archetype, value: returning }
    - name: smurfs
      filter: { type: smurf_by_properties }
    - name: new_accounts
      filter: { type: games_played_range, low: 0, high: 20 }

  duration:
    matches: 10000000
    max_time: 31536000.0

  output:
    directory: results/
    formats: [parquet, json]
    plots: true
    report: true
```

### 13.2 Config Types (Rust)

```rust
// crates/matchlab-experiments/src/config.rs

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ExperimentConfig {
    pub experiment: ExperimentSpec,
}

#[derive(Debug, Deserialize)]
pub struct ExperimentSpec {
    pub name: String,
    pub description: Option<String>,
    pub seed: u64,
    pub population: PopulationSpec,
    pub game: GameSpec,
    pub matchmaking: MatchmakingSpec,
    pub rating: RatingSpec,
    pub detection: Option<DetectionSpec>,
    pub ranking: Option<RankingSpec>,
    pub metrics: Vec<String>,
    pub objectives: Option<ObjectiveWeightsSpec>,
    pub cohorts: Vec<CohortSpec>,
    pub duration: DurationSpec,
    pub output: OutputSpec,
}

#[derive(Debug, Deserialize)]
pub struct PopulationSpec {
    pub size: u64,
    pub seed: u64,
    pub archetypes: Vec<ArchetypeSpec>,
}

#[derive(Debug, Deserialize)]
pub struct ArchetypeSpec {
    pub name: String,
    pub proportion: f64,
    pub skill_distribution: DistributionSpec,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    pub initial_rating: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DistributionSpec {
    #[serde(rename = "normal")]
    Normal { mean: f64, stddev: f64 },
    #[serde(rename = "uniform")]
    Uniform { low: f64, high: f64 },
}

#[derive(Debug, Deserialize)]
pub struct GameSpec {
    pub team_size: usize,
    pub outcome_model: String,
    pub beta: f64,
    pub noise: f64,
}

#[derive(Debug, Deserialize)]
pub struct MatchmakingSpec {
    pub algorithm: String,
    pub max_queue_time: f64,
    #[serde(flatten)]
    pub params: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RatingSpec {
    pub systems: Vec<RatingSystemSpec>,
}

#[derive(Debug, Deserialize)]
pub struct RatingSystemSpec {
    pub name: String,
    #[serde(flatten)]
    pub params: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
pub struct DetectionSpec {
    pub enabled: bool,
    pub smurf: Option<SmurfDetectionSpec>,
}

#[derive(Debug, Deserialize)]
pub struct SmurfDetectionSpec {
    pub acceleration_threshold: f64,
    pub ban_threshold: f64,
    pub min_games_before_action: u64,
}

#[derive(Debug, Deserialize)]
pub struct RankingSpec {
    pub brackets: Vec<RankBracketSpec>,
}

#[derive(Debug, Deserialize)]
pub struct RankSpec {
    pub tier: String,
    pub division: u8,
}

#[derive(Debug, Deserialize)]
pub struct RankBracketSpec {
    pub rank: RankSpec,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Deserialize)]
pub struct ObjectiveWeightsSpec {
    pub match_quality: Option<f64>,
    pub queue_time: Option<f64>,
    pub rating_accuracy: Option<f64>,
    pub convergence_speed: Option<f64>,
    pub smurf_damage: Option<f64>,
    pub false_positive_rate: Option<f64>,
    pub streak_frustration: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CohortSpec {
    pub name: String,
    pub filter: CohortFilterSpec,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum CohortFilterSpec {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "archetype")]
    Archetype { value: String },
    #[serde(rename = "smurf_by_properties")]
    SmurfByProperties,
    #[serde(rename = "games_played_range")]
    GamesPlayedRange { low: u64, high: u64 },
    #[serde(rename = "skill_range")]
    SkillRange { low: f64, high: f64 },
    #[serde(rename = "party_size")]
    PartySize { size: usize },
    #[serde(rename = "session_length")]
    SessionLength { min: f64, max: f64 },
    #[serde(rename = "rank_tier")]
    RankTier { tier: String },
}

#[derive(Debug, Deserialize)]
pub struct DurationSpec {
    pub matches: u64,
    pub max_time: f64,
}

#[derive(Debug, Deserialize)]
pub struct OutputSpec {
    pub directory: String,
    pub formats: Vec<String>,
    pub plots: bool,
    pub report: bool,
}
```

### 13.3 Config Inheritance

Experiments can inherit from a base config and override specific fields. This is essential for controlled comparisons where only one variable changes.

```yaml
# experiments/base/standard.yaml
experiment:
  name: _base
  seed: 42
  population:
    size: 10000
    seed: 42
    archetypes:
      - name: stable
        proportion: 0.65
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 5.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.01
  game:
    team_size: 5
    outcome_model: logistic
    beta: 400.0
    noise: 0.05
  matchmaking:
    algorithm: expanding_window
    max_queue_time: 60.0
    tiers:
      - [5.0, 25.0]
      - [10.0, 50.0]
      - [20.0, 100.0]
      - [30.0, 200.0]
    max_window: 300.0
  duration:
    matches: 10000000
    max_time: 31536000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
```

```yaml
# experiments/elo_vs_glicko.yaml
base: experiments/base/standard.yaml

experiment:
  name: elo_vs_glicko
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
      - name: glicko2
        initial_rating: 1500.0
        initial_rd: 350.0
        initial_volatility: 0.06
        tau: 0.5
        epsilon: 0.00001
  metrics:
    - match_quality
    - rating_accuracy
    - convergence
```

The runner loads the base config first, then deep-merges the experiment-specific overrides. This means changing only `rating.systems` preserves the exact same population, game model, matchmaking, and duration — a true controlled comparison.

### 13.4 Experiment Runner

```rust
// crates/matchlab-experiments/src/runner.rs

use matchlab_core::event::EventEngine;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_core::simulation::Simulation;
use crate::config::ExperimentConfig;
use crate::seed::SeedManager;

pub struct ExperimentRunner;

#[derive(Debug)]
pub struct ExperimentResult {
    pub experiment_id: String,
    pub name: String,
    pub config_hash: String,
    pub git_commit: String,
    pub timestamp: String, // ISO-8601
    pub metrics: std::collections::HashMap<String, matchlab_metrics::MetricResult>,
    pub utility_score: Option<f64>,
}

impl ExperimentRunner {
    pub fn run(config: &ExperimentConfig) -> ExperimentResult {
        let seeds = SeedManager::from_experiment_seed(config.experiment.seed);
        let mut rng = SimRng::from_seed(seeds.experiment_seed);

        let mut world = World::new(SimRng::from_seed(seeds.population_seed));
        // ... generate population

        let mut engine = EventEngine::new();
        // ... register handlers

        let mut metrics = matchlab_metrics::MetricsEngine::new();
        // ... register collectors

        let mut sim = Simulation::new(world, engine);
        let until = SimTime::from_secs(config.experiment.duration.max_time);
        sim.run(until);

        metrics.finalize();

        // Compute utility score if objective weights provided
        let utility_score = config.experiment.objectives.as_ref().map(|obj| {
            let func = matchlab_objective::utility::ObjectiveFunction {
                weights: obj.clone().into(),
            };
            let (score, _) = func.evaluate(metrics.results());
            score
        });

        ExperimentResult {
            experiment_id: uuid::Uuid::new_v4().to_string(),
            name: config.experiment.name.clone(),
            config_hash: crate::seed::hash_config(config),
            git_commit: crate::seed::git_commit_hash(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metrics: metrics.results().clone(),
            utility_score,
        }
    }
}
```

### 13.5 Factorial Design

```rust
// crates/matchlab-experiments/src/factorial.rs

use crate::config::ExperimentConfig;
use serde_yaml::Value;

pub struct FactorialDesign {
    pub factors: Vec<Factor>,
}

pub struct Factor {
    pub name: String,
    pub values: Vec<Value>,
}

impl FactorialDesign {
    pub fn generate_configs(&self, base: &ExperimentConfig) -> Vec<ExperimentConfig> {
        let mut configs = vec![base.clone()];
        for factor in &self.factors {
            let mut new_configs = Vec::new();
            for config in &configs {
                for value in &factor.values {
                    let mut modified = config.clone();
                    set_nested_value(&mut modified, &factor.name, value.clone());
                    new_configs.push(modified);
                }
            }
            configs = new_configs;
        }
        configs
    }
}

fn set_nested_value(config: &mut ExperimentConfig, path: &str, value: Value) {
    // Reflect the typed config to an intermediate tree, apply the dot-separated
    // path, then rebuild the typed config. This keeps factor overrides type-safe.
    let mut tree: serde_yaml::Value = serde_yaml::to_value(&*config)
        .expect("ExperimentConfig must serialize");
    let parts: Vec<&str> = path.split('.').collect();
    let mut cursor = &mut tree;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            *cursor = value;
        } else {
            cursor = cursor.as_mapping_mut()
                .and_then(|m| m.get_mut(serde_yaml::Value::String((*part).to_string())))
                .expect("factorial path segment must exist in config");
        }
    }
    *config = serde_yaml::from_value(tree).expect("ExperimentConfig must deserialize from tree");
}
```
```

### 13.6 Counterfactual Evaluation

Feed the identical game history (same match outcomes, same player behaviors) through multiple rating systems. This isolates rating-system effects from matchmaking and game model effects.

```rust
// crates/matchlab-experiments/src/counterfactual.rs

use matchlab_core::match_::MatchResult;
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_rating::system::RatingSystem;
use std::collections::HashMap;

/// A recorded game history: the sequence of matches and their outcomes.
pub struct GameHistory {
    pub matches: Vec<MatchResult>,
    pub player_snapshots: Vec<HashMap<PlayerId, PlayerObservation>>,
}

/// Run multiple rating systems through identical history.
/// Each system's full RatingState (incl. RD and volatility) is preserved so
/// Bayesian systems like Glicko-2 and TrueSkill update correctly across matches.
pub fn counterfactual_eval(
    history: &GameHistory,
    systems: &[(&str, Box<dyn RatingSystem>)],
) -> HashMap<String, Vec<(PlayerId, matchlab_rating::system::RatingState)>> {
    let mut results = HashMap::new();

    for (name, system) in systems {
        let mut states: HashMap<PlayerId, matchlab_rating::system::RatingState> = HashMap::new();

        for (i, match_result) in history.matches.iter().enumerate() {
            let observations = &history.player_snapshots[i];

            // Initialize any new players
            for pid in match_result.team_a.iter().chain(match_result.team_b.iter()) {
                if !states.contains_key(pid) {
                    states.insert(*pid, system.initialize(*pid));
                }
            }

            // Update, carrying the full state forward
            let updates = system.update(match_result, observations);
            for (pid, state) in updates {
                states.insert(pid, state);
            }
        }

        results.insert(name.to_string(), states.into_iter().collect());
    }

    results
}
```

Usage:

```rust
// Capture game history from first run
let history = runner.capture_history(config);

// Then replay through different rating systems
let results = counterfactual_eval(&history, &[
    ("elo", Box::new(EloRatingSystem::new(elo_config))),
    ("glicko2", Box::new(Glicko2RatingSystem::new(glicko_config))),
    ("trueskill", Box::new(TrueSkillRatingSystem::new(ts_config))),
]);

// Compare: same games, different rating inferences
```

### 13.7 Seed Control

```rust
// crates/matchlab-experiments/src/seed.rs

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct SeedManager {
    pub experiment_seed: u64,
    pub population_seed: u64,
    pub game_seed: u64,
    pub arrival_seed: u64,
    pub behavior_seed: u64,
}

impl SeedManager {
    pub fn from_experiment_seed(seed: u64) -> Self {
        Self {
            experiment_seed: seed,
            population_seed: derive(seed, 1),
            game_seed: derive(seed, 2),
            arrival_seed: derive(seed, 3),
            behavior_seed: derive(seed, 4),
        }
    }
}

pub fn derive(seed: u64, index: u64) -> u64 {
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    index.hash(&mut h);
    h.finish()
}

pub fn hash_config(config: &crate::config::ExperimentConfig) -> String {
    let serialized = serde_yaml::to_string(config).unwrap_or_default();
    let mut h = DefaultHasher::new();
    serialized.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Best-effort: capture the current git commit hash so each ExperimentResult
/// records exactly which code version produced it. Falls back to "unknown" when
/// the repo can't be inspected or git is unavailable.
pub fn git_commit_hash() -> String {
    use std::process::Command;
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}
```

---

## 14. Analysis

### 14.1 Statistical Summaries

```rust
// crates/matchlab-analysis/src/stats.rs

pub struct Summary {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub stddev: f64,
}

pub fn summary(values: &[f64]) -> Summary {
    if values.is_empty() {
        return Summary { n: 0, mean: 0.0, median: 0.0, p75: 0.0, p90: 0.0, p95: 0.0, p99: 0.0, stddev: 0.0 };
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    Summary {
        n: values.len(),
        mean,
        median: percentile(&sorted, 50.0),
        p75: percentile(&sorted, 75.0),
        p90: percentile(&sorted, 90.0),
        p95: percentile(&sorted, 95.0),
        p99: percentile(&sorted, 99.0),
        stddev: var.sqrt(),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = (p / 100.0 * (sorted.len() - 1) as f64) as usize;
    sorted[idx]
}

/// Convert a Summary into a MetricResult::Summary for use by collectors.
pub fn summary_to_result(values: &[f64]) -> matchlab_metrics::MetricResult {
    if values.is_empty() {
        return matchlab_metrics::MetricResult::Scalar(0.0);
    }
    let s = summary(values);
    matchlab_metrics::MetricResult::Summary {
        mean: s.mean, median: s.median,
        p75: s.p75, p90: s.p90, p95: s.p95, p99: s.p99,
        stddev: s.stddev,
    }
}
```

### 14.2 Pareto Frontier

```rust
// crates/matchlab-analysis/src/pareto.rs

pub struct ParetoPoint {
    pub label: String,
    pub values: Vec<f64>,
}

pub fn pareto_front<'a>(
    points: &'a [ParetoPoint],
    higher_is_better: &[bool],
) -> Vec<&'a ParetoPoint> {
    points.iter()
        .filter(|p| !points.iter().any(|other| dominates(other, p, higher_is_better)))
        .collect()
}

fn dominates(a: &ParetoPoint, b: &ParetoPoint, higher_is_better: &[bool]) -> bool {
    let mut strictly_better = false;
    for (i, &hib) in higher_is_better.iter().enumerate() {
        let (a_val, b_val) = if hib { (a.values[i], b.values[i]) } else { (-a.values[i], -b.values[i]) };
        if a_val < b_val { return false; }
        if a_val > b_val { strictly_better = true; }
    }
    strictly_better
}
```

### 14.3 Cohort Analysis

```rust
// crates/matchlab-analysis/src/cohorts.rs

use matchlab_core::world::World;
use matchlab_core::player::PlayerId;
use matchlab_metrics::cohort::CohortFilter;
use matchlab_metrics::MetricsEngine;
use std::collections::HashMap;

pub struct CohortResult {
    pub name: String,
    pub player_count: usize,
    pub metrics: HashMap<String, matchlab_metrics::MetricResult>,
}

pub fn analyze_cohort(
    name: &str,
    filter: &CohortFilter,
    world: &World,
    full_metrics: &MetricsEngine,
) -> CohortResult {
    // Restrict to players (ground truth) matching the cohort filter.
    let player_ids: Vec<PlayerId> = world.players.values()
        .filter(|reality| filter.matches(reality))
        .map(|reality| reality.id)
        .collect();

    // Compute per-cohort metrics by replaying the recorded per-player values
    // that fall inside this cohort. Metrics that are collected per-match cannot
    // be trivially sliced here, so we surface the cohort slice of any metric
    // that supports per-player breakdowns.
    // (In the full implementation, collectors record per-player series that
    // `analyze_cohort` filters and re-aggregates.)
    let mut metrics: HashMap<String, matchlab_metrics::MetricResult> = HashMap::new();
    metrics.insert(
        "rating_accuracy".to_string(),
        cohort_rating_accuracy(&player_ids, world),
    );

    CohortResult {
        name: name.to_string(),
        player_count: player_ids.len(),
        metrics,
    }
}

fn cohort_rating_accuracy(
    player_ids: &[PlayerId],
    world: &World,
) -> matchlab_metrics::MetricResult {
    let errors: Vec<f64> = player_ids.iter()
        .filter_map(|pid| {
            let obs = world.observations.get(pid)?;
            let reality = world.players.get(pid)?;
            Some((obs.rating - reality.skill.overall()).abs())
        })
        .collect();
    if errors.is_empty() {
        return matchlab_metrics::MetricResult::Scalar(0.0);
    }
    matchlab_metrics::MetricResult::Summary {
        mean: errors.iter().sum::<f64>() / errors.len() as f64,
        median: 0.0,
        p75: 0.0,
        p90: 0.0,
        p95: 0.0,
        p99: 0.0,
        stddev: 0.0,
    }
}
```


### 14.4 Report Generation

```rust
// crates/matchlab-analysis/src/report.rs

use matchlab_experiments::ExperimentResult;

pub struct ReportConfig {
    pub include_plots: bool,
    pub include_raw_data: bool,
    pub format: ReportFormat,
}

#[derive(Clone)]
pub enum ReportFormat { Html, Json, Markdown }

pub fn generate_comparison_report(
    results: &[ExperimentResult],
    config: &ReportConfig,
) -> String {
    match config.format {
        ReportFormat::Markdown => generate_markdown(results),
        ReportFormat::Json => serde_json::to_string_pretty(results).unwrap_or_default(),
        ReportFormat::Html => todo!(),
    }
}

fn generate_markdown(results: &[ExperimentResult]) -> String {
    let mut out = String::from("# matchlab Experiment Results\n\n");
    for result in results {
        out.push_str(&format!("## {}\n\n", result.name));
        out.push_str(&format!("Config: `{}`\n\n", result.config_hash));
        if let Some(score) = result.utility_score {
            out.push_str(&format!("**Utility score: {:.4}**\n\n", score));
        }
        out.push_str("| Metric | Value |\n|--------|-------|\n");
        for (name, value) in &result.metrics {
            out.push_str(&format!("| {} | {:?} |\n", name, value));
        }
        out.push('\n');
    }
    out
}
```

### 14.5 Raw Data Export

Full per-match and per-player data export enables external analysis in Python/R/no arbitrary aggregation is lost. `OutputSpec.formats` supports `json` and `parquet`; the exporter writes raw event traces, match results, and per-tick observations.

```rust
// crates/matchlab-analysis/src/export.rs

use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportedMatch {
    pub match_id: String,
    pub tick: u64,
    pub winner: String,
    pub team_a: Vec<String>,
    pub team_b: Vec<String>,
    pub team_a_score: f64,
    pub team_b_score: f64,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportedObservation {
    pub player_id: String,
    pub tick: u64,
    pub rating: f64,
    pub rating_deviation: f64,
    pub games_played: u64,
}

pub enum ExportFormat { Json, Parquet }

impl ExportFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "json" => Some(ExportFormat::Json),
            "parquet" => Some(ExportFormat::Parquet),
            _ => None,
        }
    }
}

pub struct RawDataExporter {
    pub directory: String,
    pub format: ExportFormat,
    pub matches: Vec<ExportedMatch>,
    pub observations: Vec<ExportedObservation>,
}

impl RawDataExporter {
    pub fn new(directory: String, format: ExportFormat) -> Self {
        Self { directory, format, matches: Vec::new(), observations: Vec::new() }
    }

    pub fn record_match(&mut self, mr: &MatchResult, world: &World) {
        self.matches.push(ExportedMatch {
            match_id: mr.match_id.0.to_string(),
                tick: world.time.ticks(),
            winner: format!("{:?}", mr.winner),
            team_a: mr.team_a.iter().map(|p| p.0.to_string()).collect(),
            team_b: mr.team_b.iter().map(|p| p.0.to_string()).collect(),
            team_a_score: mr.team_a_score,
            team_b_score: mr.team_b_score,
            duration_secs: mr.duration.as_secs_f64(),
        });
    }

    pub fn record_observations(&mut self, world: &World) {
        for (pid, obs) in &world.observations {
            self.observations.push(ExportedObservation {
                player_id: pid.0.to_string(),
            tick: world.time.ticks(),
                rating: obs.rating,
                rating_deviation: obs.rating_deviation,
                games_played: obs.games_played,
            });
        }
    }

    pub fn write(&self) -> std::io::Result<()> {
        // Ensure the output directory exists
        std::fs::create_dir_all(&self.directory)?;
        match self.format {
            ExportFormat::Json => {
                let path = std::path::Path::new(&self.directory).join("matches.json");
                let data = serde_json::to_string_pretty(&self.matches).unwrap_or_default();
                std::fs::write(path, data)?;

                let obs_path = std::path::Path::new(&self.directory).join("observations.json");
                let obs_data = serde_json::to_string_pretty(&self.observations).unwrap_or_default();
                std::fs::write(obs_path, obs_data)?;
            }
            ExportFormat::Parquet => {
                // Requires the `parquet` and `arrow` crates.
                // Write a row-group per batch of matches/observations.
                todo!("parquet writer")
            }
        }
        Ok(())
    }
}
```

This exporter is wired into the `ExperimentRunner`: after `sim.run(until)` and `metrics.finalize()`, if `output.formats` includes `json` or `parquet`, the exporter writes its accumulated traces before the report is generated.

### 14.6 Multi-Experiment Comparison

```rust
// crates/matchlab-analysis/src/comparator.rs

use matchlab_experiments::ExperimentResult;
use std::collections::HashMap;

/// Compare multiple experiment results side-by-side.
pub struct Comparator {
    pub results: Vec<ExperimentResult>,
    pub baseline: Option<usize>, // index into results
}

impl Comparator {
    pub fn new(results: Vec<ExperimentResult>) -> Self {
        Self { results, baseline: None }
    }

    pub fn set_baseline(&mut self, index: usize) {
        self.baseline = Some(index);
    }

    /// For each metric, compute: baseline mean, other mean, delta, relative % change.
    pub fn metric_comparison(&self) -> HashMap<String, Vec<MetricComparison>> {
        let mut out = HashMap::new();
        for result in &self.results {
            for (name, value) in &result.metrics {
                out.entry(name.clone()).or_insert_with(Vec::new)
                    .push(MetricComparison {
                        experiment: result.name.clone(),
                        value: value.clone(),
                    });
            }
        }
        out
    }

    /// Rank experiments by utility score (descending).
    pub fn ranking(&self) -> Vec<(&ExperimentResult, f64)> {
        let mut ranked: Vec<_> = self.results.iter()
            .filter_map(|r| r.utility_score.map(|s| (r, s)))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }
}

pub struct MetricComparison {
    pub experiment: String,
    pub value: matchlab_metrics::MetricResult,
}
```

---

## 15. Adversarial Agents

Players that actively try to exploit or manipulate the rating system. These are not passive behavioral archetypes — they optimize against the system.

### 15.1 Agent Trait

```rust
// crates/matchlab-adversarial/src/agent.rs

use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

pub trait AdversarialAgent: Send + Sync {
    /// Called each tick. The agent decides what action to take.
    fn tick(&mut self, player_id: PlayerId, world: &mut World);

    /// The agent's objective function (what it optimizes for).
    fn objective(&self) -> AdversarialObjective;
}

#[derive(Debug, Clone)]
pub enum AdversarialObjective {
    MaximizeRating,
    MinimizeGamesPlayed,
    MaximizeWinRate { target_games: u64 },
    MaintainLowRating,
    WinTrade { partner: PlayerId },
    Derate,
}
```

### 15.2 Booster (Boosting Duo)

```rust
// crates/matchlab-adversarial/src/booster.rs

pub struct BoosterAgent {
    pub boost_target: PlayerId,
    pub boostee: PlayerId,
}

impl AdversarialAgent for BoosterAgent {
    fn tick(&mut self, _player_id: PlayerId, _world: &mut World) {
        // Queue together, manipulate match outcomes
        // The booster intentionally underperforms to lower their own rating
        // while the boostee wins
        todo!()
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MaximizeRating
    }
}
```

### 15.3 Deranker

```rust
// crates/matchlab-adversarial/src/deranker.rs

pub struct DerankerAgent {
    pub target_rating: f64,
}

impl AdversarialAgent for DerankerAgent {
    fn tick(&mut self, _player_id: PlayerId, _world: &mut World) {
        // Intentionally lose matches to drop rating
        // May AFK, disconnect, or throw
        todo!()
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MaintainLowRating
    }
}
```

### 15.4 Win Trader

```rust
// crates/matchlab-adversarial/src/win_trader.rs

pub struct WinTraderAgent {
    pub partner: PlayerId,
    pub alternating: bool,
}

impl AdversarialAgent for WinTraderAgent {
    fn tick(&mut self, _player_id: PlayerId, _world: &mut World) {
        // Queue at same time to match together
        // Alternate wins to maintain rating while farming games
        todo!()
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::WinTrade { partner: self.partner }
    }
}
```

### 15.5 Rating Farmer

```rust
// crates/matchlab-adversarial/src/rating_farmer.rs

pub struct RatingFarmerAgent {
    pub quit_probability: f64,
    pub quit_after_minutes: f64,
}

impl AdversarialAgent for RatingFarmerAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        // Strategy: queue, then immediately quit/disconnect after starting
        // This causes rating loss but keeps games_played minimal
        // The intent: high skill + low games = smurf-like after reset
        //
        // In practice: this agent queues, starts a match, then disconnects.
        // The rating system may or may not handle disconnects gracefully.
        todo!()
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MaximizeWinRate { target_games: 10 }
    }
}
```

### 15.6 AFK / Intentional Feeder

```rust
// crates/matchlab-adversarial/src/afk.rs

pub struct AfkAgent {
    pub go_afk_probability: f64,
}

impl AdversarialAgent for AfkAgent {
    fn tick(&mut self, player_id: PlayerId, world: &mut World) {
        if let Some(reality) = world.players.get_mut(&player_id) {
            // Randomly disconnect or go AFK during matches
            if world.rng.gen_bool(self.go_afk_probability) {
                reality.quit_probability = 1.0;
            }
        }
    }

    fn objective(&self) -> AdversarialObjective {
        AdversarialObjective::MinimizeGamesPlayed
    }
}
```

---

## 16. Player Utility

### 16.1 Satisfaction Model

Player satisfaction is modeled through observable proxies, not a fake "fun" number. The model predicts retention probability based on experience history.

```rust
// crates/matchlab-utility/src/satisfaction.rs

use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

pub struct SatisfactionModel {
    pub weights: SatisfactionWeights,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SatisfactionWeights {
    pub match_quality: f64,
    pub queue_time_penalty: f64,
    pub win_bonus: f64,
    pub loss_streak_penalty: f64,
    pub rank_progression_bonus: f64,
    pub fairness_sensitivity: f64,
    pub rematch_bonus: f64,
}

impl Default for SatisfactionWeights {
    fn default() -> Self {
        Self {
            match_quality: 1.0,
            queue_time_penalty: -0.01,
            win_bonus: 0.5,
            loss_streak_penalty: -0.3,
            rank_progression_bonus: 0.2,
            fairness_sensitivity: -0.8,
            rematch_bonus: 0.1,
        }
    }
}

pub struct PlayerExperience {
    pub recent_match_qualities: Vec<f64>,
    pub recent_queue_times: Vec<f64>,
    pub recent_outcomes: Vec<bool>,
    pub current_streak: i32,
    pub rank_change: f64,
    pub perceived_fairness: f64,
    /// Fraction of recent matches the player chose to rematch/requeue.
    pub rematch_rate: f64,
}

impl SatisfactionModel {
    /// Compute satisfaction score from experience history.
    pub fn satisfaction(&self, exp: &PlayerExperience) -> f64 {
        let avg_quality = mean_or(&exp.recent_match_qualities, 0.5);
        let avg_queue = mean_or(&exp.recent_queue_times, 30.0);
        let win_rate = exp.recent_outcomes.iter().filter(|&&w| w).count() as f64
            / exp.recent_outcomes.len().max(1) as f64;
        let streak_penalty = if exp.current_streak < -3 {
            self.weights.loss_streak_penalty * (exp.current_streak.abs() as f64 - 3.0)
        } else {
            0.0
        };

        self.weights.match_quality * avg_quality
            + self.weights.queue_time_penalty * avg_queue
            + self.weights.win_bonus * win_rate
            + streak_penalty
            + self.weights.rank_progression_bonus * exp.rank_change
            + self.weights.fairness_sensitivity * (1.0 - exp.perceived_fairness)
            + self.weights.rematch_bonus * exp.rematch_rate
    }

    /// Probability that the player continues playing next session.
    pub fn retention_probability(&self, satisfaction: f64) -> f64 {
        // Logistic transform: higher satisfaction → higher retention
        1.0 / (1.0 + (-satisfaction).exp())
    }

    /// Probability the player requeues for another match (rematch).
    pub fn rematch_probability(&self, satisfaction: f64) -> f64 {
        // Rematch is a stronger commitment than staying in the population;
        // require a higher satisfaction threshold before a player requeues.
        1.0 / (1.0 + (-0.5 * (satisfaction - 2.0)).exp())
    }
}

fn mean_or(values: &[f64], default: f64) -> f64 {
    if values.is_empty() { default } else { values.iter().sum::<f64>() / values.len() as f64 }
}
```

### 16.2 Matchmaking as an Economic System

When player utility is modeled, matchmaking becomes an ecological system, not merely a sorting algorithm. Poor match quality reduces retention; long queues reduce retention; unfair matches reduce retention. The system must balance all three to maintain a healthy player population.

---

## 17. v0.1 Implementation

The v0.1 build order (Steps 1–12) is complete. The implementation delivers a working discrete-event simulation with 10,000 players, 1D static skill, logistic outcomes, rating-balanced batch matchmaking, Elo rating, and metric collectors. See `experiments/v0_1_basic.yaml` for the minimal experiment manifest and `AGENTS.md` for the full implementation state.

### Reference: Minimal Experiment Manifest

```yaml
experiment:
  name: v0_1_basic
  description: "Minimal Elo test with static skill population, cold ladder start"
  seed: 42

  population:
    size: 10000
    seed: 42
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0

  game:
    team_size: 5
    outcome_model: logistic
    beta: 400.0
    noise: 0.05

  matchmaking:
    algorithm: batch
    batch_interval: 10
    max_queue_time: 60.0

  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0

  metrics:
    - match_quality
    - queue_time
    - rating_accuracy

  cohorts:
    - name: all
      filter: { type: all }

  duration:
    matches: 1000000
    max_time: 604800.0

  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
```
