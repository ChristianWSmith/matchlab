# Plugin Cookbook

Practical guide to writing matchlab plugins. Every algorithm in matchlab is a Lua script implementing a layer-specific contract. This cookbook walks through each layer with a working reference, then covers common patterns.

## Table of Contents

- [Rating System](#rating-system)
- [Outcome Model](#outcome-model)
- [Matchmaker](#matchmaker)
- [Metric Collector](#metric-collector)
- [Detection System](#detection-system)
- [Adversarial Agent](#adversarial-agent)
- [Satisfaction Model](#satisfaction-model)
- [Common Patterns](#common-patterns)
- [Debugging Tips](#debugging-tips)
- [Testing Your Plugin](#testing-your-plugin)

---

## Rating System

A rating system adjusts player ratings after each match. Reference: `plugins/rating/elo.lua`.

### Contract

Your script must declare these globals and functions:

```lua
information_budget = { "WinLoss" }  -- what match data you need (or nil for full)

function initialize(player_id, config, context)
    -- Return a rating state table and the (possibly updated) context.
    return {
        rating = config.initial_rating,
        rating_deviation = 350.0,
        volatility = 0.06,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    -- Return the probability that team_a wins.
    -- team_a and team_b are arrays of observation tables.
    return 0.5
end

function update(match_result, observations, config, context)
    -- Return an array of rating update tables and the (possibly updated) context.
    -- Each update: { player_id, rating, rating_deviation, volatility, games_played }
    return updates, context
end
```

### Information Budget

The `information_budget` global controls what data the loop sanitizes before calling your `update`. Valid values:

- `"WinLoss"` -- only winner/loser, no scores or performances (Elo, Glicko-2, TrueSkill use this)
- `"Scores"` -- scores are available
- `"Performances"` -- per-player stats (kills, deaths, etc.) are available
- `nil` or absent -- full match result passed through

If your budget is `"WinLoss"`, the `match_result` table your `update` receives will have scores zeroed and performances emptied.

### Step-by-Step: Elo

```lua
information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    return {
        rating = config.initial_rating,    -- e.g. 1000.0
        rating_deviation = 350.0,
        volatility = 0.06,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local avg_a = team_average(team_a)
    local avg_b = team_average(team_b)
    return expected_score(avg_a, avg_b, config.beta)
end

function update(match_result, observations, config, context)
    local team_a = match_result.team_a
    local team_b = match_result.team_b
    local avg_a = team_average_ratings(team_a, observations)
    local avg_b = team_average_ratings(team_b, observations)
    local expected_a = expected_score(avg_a, avg_b, config.beta)
    local expected_b = 1.0 - expected_a
    local actual_a = match_result.winner == "A" and 1.0 or 0.0
    local actual_b = 1.0 - actual_a

    local updates = {}
    update_team(updates, team_a, observations, config.k_factor, actual_a, expected_a)
    update_team(updates, team_b, observations, config.k_factor, actual_b, expected_b)
    return updates, context
end
```

Key points:
- `team_a` / `team_b` in `match_result` are arrays of **player IDs**, not observation objects. Look up observations via `observations[id]`.
- The `observations` table is keyed by player ID (integer).
- `config` comes from the YAML `params:` block under your rating system entry.
- The divisor `beta * ln(10)` keeps Elo on the same log10 scale as the logistic game model.

### Helper Pattern: team average

```lua
function team_average_ratings(ids, observations)
    local sum, n = 0.0, 0
    for _, id in ipairs(ids) do
        local o = observations[id]
        if o then
            sum = sum + o.rating
            n = n + 1
        end
    end
    if n == 0 then return 0.0 end
    return sum / n
end
```

### Using in a Manifest

```yaml
rating:
  systems:
    - script: plugins/rating/elo.lua
      k_factor: 32.0
      initial_rating: 1000.0
      beta: 400.0
```

---

## Outcome Model

An outcome model decides who wins each match and generates match results. Reference: `plugins/game/logistic.lua`.

### Contract

```lua
function win_probability(team_a, team_b, config, context)
    -- team_a and team_b are arrays of observation tables.
    -- Return a probability in [0, 1] that team_a wins.
    return 0.5
end

function simulate(match_id, team_a, team_b, config, context)
    -- Decide the winner and build a full MatchResult.
    -- Return the result table and the (possibly updated) context.
    return result, context
end
```

The `simulate` function must return a table with these fields:

```lua
{
    winner = "A" or "B",
    team_a = { player_id_1, player_id_2, ... },   -- integer IDs
    team_b = { player_id_1, player_id_2, ... },
    team_a_score = 13.0,
    team_b_score = 5.0,
    duration_secs = 1800.0,
    performances = {
        { player_id = 1, kills = 10, deaths = 2, assists = 4,
          objective_score = 55.0, impact = 0.8, variance = 0.2 },
        ...
    },
    variance = 0.05,
    disconnected = false,
    forfeited = false,
}
```

### Truth Separation

Outcome models **may** read `skill_overall` and `skill_vector` from observation tables. These carry the ground-truth skill from `PlayerReality` -- this is the only layer that decides match outcomes from true skill. Rating systems, matchmakers, and detection systems must **never** read these fields.

```lua
function effective_skill(o)
    if o.skill_overall ~= nil then
        return o.skill_overall    -- ground truth, available to game model only
    end
    return o.rating               -- fallback for vacuum-seeded observations
end
```

### Step-by-Step: Logistic

```lua
function win_probability(team_a, team_b, config, context)
    local diff = team_average(team_a) - team_average(team_b)
    return 1.0 / (1.0 + math.exp(-diff / config.beta))
end

function simulate(match_id, team_a, team_b, config, context)
    local base_p = win_probability(team_a, team_b, config, context)

    -- Add noise (skip when noise == 0 for deterministic outcomes)
    local noise = 0.0
    if config.noise and config.noise > 0.0 then
        noise = matchlab.rng_range(-config.noise, config.noise)
    end
    local adjusted_p = math.max(0.01, math.min(0.99, base_p + noise))
    local team_a_wins = matchlab.rng_bool(adjusted_p)

    -- Build the result (see full example in plugins/game/logistic.lua)
    local result = { ... }
    return result, context
end
```

Key points:
- Use `matchlab.rng_range(low, high)` for noise -- never `math.random`.
- Clamp `adjusted_p` to [0.01, 0.99] to avoid degenerate outcomes.
- The `simulate` function is called inside a guarded RNG region, so all `matchlab.rng_*` calls draw deterministically from the simulation's seed.

### Using in a Manifest

```yaml
game:
  teams: { a: 5, b: 5 }
  script: plugins/game/logistic.lua
  beta: 400.0
  noise: 0.05
```

---

## Matchmaker

A matchmaker pairs players from the queue into matches. Reference: `plugins/matchmaking/batch.lua`.

### Contract

```lua
function find_matches(queue, teams, now_secs, config, context)
    -- queue: array of queue entry tables
    -- teams: { a = { size, role }, b = { size, role } }
    -- now_secs: current simulation time in seconds
    -- Return an array of proposed matches and the context.
    return matches, context
end
```

Each proposed match:

```lua
{
    team_a = { player_id_1, player_id_2, ... },
    team_b = { player_id_1, player_id_2, ... },
    quality_score = 0.95,  -- optional, computed by the script or adapter
}
```

### Queue Entry Fields

Each entry in `queue` is a table with these fields:

| Field | Type | Description |
|-------|------|-------------|
| `player_id` | integer | Unique player ID |
| `rating` | number | Current visible rating |
| `rating_deviation` | number | Current RD |
| `games_played` | integer | Lifetime matches |
| `win_rate` | number | Lifetime win rate |
| `idx` | integer | Position in the queue snapshot |
| `joined_at_secs` | number | When the player joined the queue |
| `wait_secs` | number | Current wait time |
| `region` | string | "na", "eu", "asia", or "other" |
| `party_id` | integer or nil | Party membership |
| `latency_ms` | number or nil | Estimated latency |
| `game_mode` | string | Game mode |
| `role` | string or nil | Queued role (nil = "any") |

### Truth Separation

Queue entries carry **only** observation fields. Never access `skill_vector`, `skill_overall`, or `hidden_mmr` -- those are ground-truth fields.

### Step-by-Step: Batch (Rating-Balanced)

```lua
function find_matches(queue, teams, now_secs, config, context)
    local size_a = teams.a.size
    local size_b = teams.b.size

    -- Copy and sort by rating (ties by join order, then idx)
    local candidates = {}
    for _, e in ipairs(queue) do
        table.insert(candidates, e)
    end
    table.sort(candidates, function(a, b)
        if a.rating ~= b.rating then
            return a.rating < b.rating
        end
        if a.joined_at_secs ~= b.joined_at_secs then
            return a.joined_at_secs < b.joined_at_secs
        end
        return a.idx < b.idx
    end)

    -- Alternate adjacent-by-rating players onto opposite teams
    local matches = {}
    local team_a, team_b = {}, {}
    local alternate = false

    for _, e in ipairs(candidates) do
        if alternate then
            if #team_b < size_b then
                table.insert(team_b, e.player_id)
            else
                table.insert(team_a, e.player_id)
            end
        else
            if #team_a < size_a then
                table.insert(team_a, e.player_id)
            else
                table.insert(team_b, e.player_id)
            end
        end
        alternate = not alternate

        if #team_a == size_a and #team_b == size_b then
            table.insert(matches, {
                team_a = team_a,
                team_b = team_b,
                quality_score = match_quality(team_a, team_b, ratings),
            })
            team_a, team_b = {}, {}
            alternate = false
        end
    end

    return matches, context
end
```

### Role-Aware Matching

When `teams.a.role` / `teams.b.role` are set, fill each side exclusively from entries whose `role` matches. This is essential for asymmetric games (e.g., 1v4 Dead-by-Daylight):

```lua
local role_a = teams.a.role
local role_b = teams.b.role

if role_a and role_b then
    local pool_a, pool_b = {}, {}
    for _, e in ipairs(queue) do
        if e.role == role_a then table.insert(pool_a, e) end
        if e.role == role_b then table.insert(pool_b, e) end
    end
    -- Sort each pool by rating, alternate consumption
    table.sort(pool_a, by_rating)
    table.sort(pool_b, by_rating)
    -- Fill team_a from pool_a, team_b from pool_b
end
```

### Match Quality

A simple quality metric:

```lua
function match_quality(team_a, team_b, ratings)
    local diff = math.abs(average_rating(team_a, ratings) - average_rating(team_b, ratings))
    return 1.0 - math.min(diff / 400.0, 1.0)
end
```

### Using in a Manifest

```yaml
matchmaking:
  script: plugins/matchmaking/batch.lua
  batch_interval: 10
  max_queue_time: 60.0
```

---

## Metric Collector

A metric collector records data from each match and computes a summary. Reference: `plugins/metrics/match_quality.lua`.

### Contract

```lua
name = "my_metric"  -- REQUIRED global

function on_record(match_result, snapshot, config, context)
    -- Called after each match. Accumulate data in context.
    -- Return the (possibly updated) context.
    return context
end

function compute(config, context)
    -- Return a result table describing the metric.
    return { kind = "summary", values = context.samples or {} }
end
```

The `on_record` snapshot contains:

| Field | Description |
|-------|-------------|
| `match_result` | The full match result table |
| `tick` | Current simulation tick (nanoseconds) |
| `time_secs` | Current simulation time (seconds) |
| `players` | Array of participant tables with observation + reality fields |

### Snapshot Fields

Participant tables in `snapshot.players` carry:

- `player_id`, `rating`, `rating_deviation`, `volatility`, `games_played`, `win_rate`
- `skill_overall`, `skill_vector` -- ground truth (metrics may read these)
- `true_skill`, `improvement_rate`, `reality_games_played`, `archetype` -- reality fields

Metric scripts are the **only** layer (besides the outcome model) that legitimately reads `PlayerReality` fields.

### Compute Return Types

Your `compute` function can return:

```lua
-- Summary (percentiles + mean)
{ kind = "summary", values = { 0.5, 0.7, 0.9, ... } }

-- Scalar
{ kind = "scalar", value = 42.0 }

-- Distribution (histogram)
{ kind = "distribution", values = { 0.1, 0.2, 0.3, ... } }
```

### Time Series

To produce a time-series metric (e.g., rating accuracy over time), declare:

```lua
name = "rating_accuracy"
time_buckets = { 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0 }  -- optional global
```

When `time_buckets` is present, the engine automatically generates a `{name}_by_time` `TimeSeries` metric by folding your samples into time buckets.

### Step-by-Step: Match Quality

```lua
name = "match_quality"

function on_record(match_result, snapshot, config, context)
    context.samples = context.samples or {}

    -- Index ratings from the snapshot
    local ratings = {}
    for _, p in ipairs(snapshot.players) do
        ratings[p.player_id] = p.rating
    end

    -- Compute average rating per team
    local avg_a = team_average(match_result.team_a, ratings)
    local avg_b = team_average(match_result.team_b, ratings)

    -- Quality = 1 - (|avg_a - avg_b| / 400), clamped to [0, 1]
    local diff = math.abs(avg_a - avg_b)
    table.insert(context.samples, 1.0 - math.min(diff / 400.0, 1.0))

    return context
end

function compute(config, context)
    return { kind = "summary", values = context.samples or {} }
end
```

Key points:
- Accumulate samples in `context.samples` (or any key). The context persists across calls.
- Always initialize `context.samples` with `or {}` on first call.
- The `compute` function is called once at the end of the experiment.

### Using in a Manifest

```yaml
metrics:
  - match_quality
  - queue_time
  - rating_accuracy
```

Metric names must match the `name` global in the script. Built-in metrics ship as scripts under `plugins/metrics/`.

---

## Detection System

A detection system identifies anomalous players (e.g., smurfs) and recommends interventions. Reference: `plugins/detection/smurf.lua`.

### Contract

```lua
function observe(match_result, observations, config, context)
    -- Called after each match. Update per-player evidence in context.
    -- Return the (possibly updated) context.
    return context
end

function evaluate(player_id, observations, config, context)
    -- Return a detection result and the context.
    return {
        player_id = player_id,
        probability_of_anomaly = 0.8,
        confidence = 0.9,
        evidence = { "consecutive_anomalous=5", "min_required=5" },
    }, context
end

function recommend_action(result, config, context)
    -- Return an action string and the context.
    return "RestrictQueue", context
end
```

### Action Strings

Valid actions returned by `recommend_action`:

| Action | Effect |
|--------|--------|
| `"None"` | No action |
| `"AccelerateRating"` | Speed up rating convergence |
| `"FlagForReview"` | Flag for human review |
| `"RestrictQueue"` | Restrict matchmaking options |
| `"TempBan"` | Temporary ban |
| `"Probation"` | Probationary status |
| `"Ban"` | Permanent removal |

### Per-Player State

The `context` table persists across `observe` calls. Use it to accumulate per-player evidence:

```lua
function observe(match_result, observations, config, context)
    for _, pid in ipairs(match_result.team_a) do
        record_player(pid, match_result, observations, context)
    end
    for _, pid in ipairs(match_result.team_b) do
        record_player(pid, match_result, observations, context)
    end
    return context
end

function record_player(pid, match_result, observations, context, sigma_threshold)
    local o = observations[pid]
    local perf = find_perf(match_result.performances, pid)
    if not o or not perf then return end

    local key = tostring(pid)
    local state = context[key]
    if not state then
        state = { recent = {}, consecutive = 0, games = 0, interventions = 0 }
        context[key] = state
    end

    -- Accumulate evidence...
    state.games = state.games + 1
end
```

Key points:
- The `context` is shared across all players. Use `tostring(player_id)` as the key.
- Track `interventions` count for escalation -- prior interventions raise the threshold.
- The `recommend_action` function implements an escalation ladder with a configurable `escalation_factor`.

---

## Adversarial Agent

An adversarial agent simulates disruptive player behavior (AFK, deranking, etc.). Reference: `plugins/adversarial/afk.lua`.

### Contract

```lua
function tick(player_id, behavior, observation, config, context)
    -- Modify the behavior table and return it plus the context.
    -- The behavior table has: quit_probability, party_id, tilt_level, win_rate, is_online
    return behavior, context
end

function objective(config, context)
    -- Return the agent's objective.
    return { kind = "MinimizeGamesPlayed" }
end
```

### Behavior Fields

The `behavior` table is read-write:

| Field | Type | Description |
|-------|------|-------------|
| `quit_probability` | number | 0.0-1.0, chance the player quits after a match |
| `party_id` | integer or nil | Party assignment |
| `tilt_level` | number | 0.0-1.0, frustration level |
| `win_rate` | number | Target win rate (for boosters) |
| `is_online` | boolean | Whether the player is online |

### Objective Kinds

| Objective | Description |
|-----------|-------------|
| `MaximizeRating` | Wants the highest rating possible |
| `MinimizeGamesPlayed` | Wants to play as few games as possible |
| `MaximizeWinRate` | Wants the highest win rate (with `target_games`) |
| `MaintainLowRating` | Keeps rating low (deranking) |
| `WinTrade` | Wins traded with a partner |
| `Derate` | Actively loses rating |

### Step-by-Step: AFK Agent

```lua
function tick(player_id, behavior, observation, config, context)
    if matchlab.rng_bool(config.go_afk_probability) then
        behavior.quit_probability = 1.0
    end
    return behavior, context
end

function objective(config, context)
    return { kind = "MinimizeGamesPlayed" }
end
```

This is the simplest agent -- it just has a probability of quitting each tick.

### Step-by-Step: Deranker

```lua
function tick(player_id, behavior, observation, config, context)
    local target = config.target_rating or 800.0
    if observation.rating > target then
        behavior.quit_probability = 0.9
        behavior.tilt_level = 1.0
    end
    return behavior, context
end

function objective(config, context)
    return { kind = "MaintainLowRating" }
end
```

### Using in a Manifest

```yaml
adversarial:
  agents:
    - script: plugins/adversarial/afk.lua
      go_afk_probability: 0.05
    - script: plugins/adversarial/deranker.lua
      target_rating: 800.0
```

---

## Satisfaction Model

A satisfaction model computes player satisfaction and retention probability. Reference: `plugins/utility/satisfaction.lua`.

### Contract

```lua
function satisfaction(experience, config, context)
    -- Return a satisfaction score (higher = more satisfied).
    return 0.5
end

function retention_probability(satisfaction, config, context)
    -- Return probability in [0, 1] that the player stays.
    return 0.8
end

function rematch_probability(satisfaction, config, context)
    -- Return probability in [0, 1] that the player re-queues.
    return 0.6
end
```

### Experience Fields

The `experience` table contains:

| Field | Type | Description |
|-------|------|-------------|
| `recent_match_qualities` | array | Recent match quality scores |
| `recent_queue_times` | array | Recent queue wait times |
| `recent_outcomes` | array | Recent win/loss booleans |
| `current_streak` | integer | Positive = win streak, negative = loss streak |
| `rank_change` | number | Recent rank change |
| `perceived_fairness` | number | 0.0-1.0, perceived match fairness |
| `rematch_rate` | number | Rate of rematches |

### Step-by-Step: Weighted Sum

```lua
function satisfaction(experience, config, context)
    local avg_quality = mean_or(experience.recent_match_qualities, 0.5)
    local avg_queue = mean_or(experience.recent_queue_times, 30.0)

    local wins = 0
    for _, won in ipairs(experience.recent_outcomes) do
        if won then wins = wins + 1 end
    end
    local win_rate = wins / math.max(#experience.recent_outcomes, 1)

    -- Loss streak penalty only kicks in past -3
    local streak_penalty = 0.0
    if experience.current_streak < -3 then
        streak_penalty = -0.3 * (math.abs(experience.current_streak) - 3.0)
    end

    return (config.match_quality or 1.0) * avg_quality
        + (config.queue_time_penalty or -0.01) * avg_queue
        + (config.win_bonus or 0.5) * win_rate
        + streak_penalty
        + (config.rank_progression_bonus or 0.2) * experience.rank_change
        + (config.fairness_sensitivity or -0.8) * (1.0 - experience.perceived_fairness)
        + (config.rematch_bonus or 0.1) * experience.rematch_rate
end

function retention_probability(satisfaction, config, context)
    return 1.0 / (1.0 + math.exp(-satisfaction))
end

function rematch_probability(satisfaction, config, context)
    return 1.0 / (1.0 + math.exp(-0.5 * (satisfaction - 2.0)))
end
```

### Using in a Manifest

```yaml
satisfaction:
  script: plugins/utility/satisfaction.lua
  match_quality: 1.0
  queue_time_penalty: -0.01
  win_bonus: 0.5
  loss_streak_penalty: -0.3
```

---

## Common Patterns

### RNG Usage

**Never** call `math.random` or `math.randomseed`. All randomness must flow through the simulation's `SimRng`:

```lua
local value = matchlab.rng_range(0.0, 1.0)       -- uniform [low, high)
local hit = matchlab.rng_bool(0.7)                 -- true with probability p
local sample = matchlab.rng_normal(1000.0, 250.0)  -- normal(mean, stddev)
local id = matchlab.rng_u64()                       -- random u64
```

Calling `matchlab.rng_*` outside a guarded region produces an error. This is intentional -- it catches scripts that draw at load time instead of during simulation.

### Config Reading

Your `config` table comes from the YAML `params:` block:

```yaml
rating:
  systems:
    - script: plugins/rating/elo.lua
      k_factor: 32.0          -- becomes config.k_factor
      initial_rating: 1000.0  -- becomes config.initial_rating
      beta: 400.0             -- becomes config.beta
```

Use `config.key or default_value` for optional parameters:

```lua
local k = config.k_factor or 32.0
local threshold = config.sigma_threshold or 3.0
```

### Context Threading

The `context` is an opaque Lua table persisted on the Rust side. Every call receives it and must return it (possibly mutated):

```lua
function my_func(args, config, context)
    context.counter = (context.counter or 0) + 1
    context.samples = context.samples or {}
    table.insert(context.samples, compute_value(args))
    return result, context
end
```

The context is round-tripped through YAML: Lua tables with keys `1..=n` become YAML sequences; all others become YAML mappings. Stick to simple types (numbers, strings, booleans, arrays, maps) for reliable serialization.

### Observation Fields

Rating/detection/adversarial scripts see these observation fields:

| Field | Type | Notes |
|-------|------|-------|
| `player_id` | integer | |
| `rating` | number | Visible rating |
| `rating_deviation` | number | Uncertainty |
| `volatility` | number | |
| `games_played` | integer | |
| `win_rate` | number | |
| `tilt_level` | number | |
| `is_online` | boolean | |
| `recent_performances` | array | Recent performance scores |
| `queue_joined_at_secs` | number or nil | |
| `party_id` | integer or nil | |
| `role` | string or nil | Player's role |

The outcome model and metrics additionally see:

| Field | Type | Notes |
|-------|------|-------|
| `skill_overall` | number | Ground truth |
| `skill_vector` | table | Dimension → value map |
| `true_skill` | number | Reality field (metrics only) |
| `improvement_rate` | number | Reality field (metrics only) |
| `archetype` | string | Reality field (metrics only) |

---

## Debugging Tips

### Print Statements

Lua `print()` output goes to stderr during simulation runs:

```lua
function update(match_result, observations, config, context)
    print("DEBUG: match " .. match_result.match_id .. " winner=" .. match_result.winner)
    return updates, context
end
```

### Validating Script Loads

The `matchlab validate` command checks your script for syntax errors and required functions:

```bash
cargo run -- validate plugins/rating/elo.lua
```

### Checking for `math.random` Usage

Scripts that call `math.random` are rejected at load time. If you get a validation error about banned functions, replace `math.random` calls with `matchlab.rng_*` equivalents.

### Inspecting Context

Add `print(context)` calls to debug accumulated state. The context is converted to a Lua table before each call, so you can read any key directly.

### Common Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `"matchlab.rng_* called outside a guarded region"` | RNG call at load time | Move RNG calls inside functions |
| `"missing required function: update"` | Function not defined | Check the contract for your layer |
| `"attempt to index nil value"` | Missing nil check on `observations[id]` | Add `if o then` guard |
| NaN in output | Division by zero or uninitialized variables | Guard all divisions, initialize sums to 0.0 |

---

## Testing Your Plugin

### Unit-Level: Manual Smoke Test

Run a minimal experiment that exercises your plugin:

```yaml
experiment:
  name: my_plugin_test
  seed: 42
  population:
    size: 100
    archetypes:
      - name: test
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 250 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
        initial_rating: 1000.0
  game:
    teams: { a: 5, b: 5 }
    script: plugins/game/logistic.lua
    beta: 400.0
    noise: 0.0
  matchmaking:
    script: plugins/matchmaking/batch.lua
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - script: plugins/rating/my_system.lua  # your script
        initial_rating: 1000.0
  metrics: [match_quality, queue_time, rating_accuracy]
  cohorts: []
  duration:
    matches: 100
    max_time: 3600.0
  output:
    directory: results/test/
    formats: [json]
    report: true
```

### Determinism Check

Run twice with the same seed -- output must be byte-identical:

```bash
cargo run -- run experiments/my_plugin_test.yaml
cp results/test/my_plugin_test.json /tmp/first.json
cargo run -- run experiments/my_plugin_test.yaml
diff /tmp/first.json results/test/my_plugin_test.json
```

### Sanity Checks

1. **No NaN/inf**: Verify all metric values are finite.
2. **Rating convergence**: For a homogeneous population, `rating_accuracy` (MAE) should decrease over time.
3. **Match quality**: With a balanced matchmaker, quality should be > 0.9.
4. **Queue time**: Should be reasonable (e.g., < 60s for a batch matchmaker with enough players).

### Reference Scripts

Study the built-in plugins as templates:

- `plugins/rating/elo.lua` -- simplest rating system
- `plugins/rating/glicko2.lua` -- complex rating with volatility tracking
- `plugins/game/logistic.lua` -- standard outcome model
- `plugins/matchmaking/batch.lua` -- balanced formation
- `plugins/metrics/match_quality.lua` -- simplest metric
- `plugins/detection/smurf.lua` -- full detection with escalation
- `plugins/adversarial/afk.lua` -- simplest adversarial agent
- `plugins/utility/satisfaction.lua` -- satisfaction model
