# Plugin API Reference

MatchLab's algorithms are implemented as Lua 5.4 plugins. This document specifies the contracts that plugins must satisfy.

## Quick Start

Every plugin is a `.lua` file under `plugins/<layer>/`. To create a new plugin:

1. Copy a shipped plugin from the same layer as a template
2. Implement the required functions (see contracts below)
3. Reference it in your manifest via `script: <path>`
4. Run `cargo run -- run <manifest.yaml>` to test

**Rules that apply to all plugins:**

- **No `math.random`.** Scripts containing `math.random` are rejected at load time. Use `matchlab.rng_*` helpers instead.
- **All randomness through `matchlab.rng_*`** — deterministic per-seed.
- **Config is injected** as the second-to-last argument: a Lua table from the YAML `params:` block.
- **Context is threaded** — a persistent Lua table passed as the final argument. Mutate it in place, or return `(value, context)` to replace it.
- **Lua errors panic the Rust adapter.** Ensure your script handles edge cases gracefully.

## RNG Helpers

Available as globals in every script:

| Function | Signature | Description |
|----------|-----------|-------------|
| `matchlab.rng_range(low, high)` | `(f64, f64) → f64` | Uniform random in `[low, high)` |
| `matchlab.rng_bool(p)` | `(f64) → bool` | True with probability `p` |
| `matchlab.rng_normal(mean, stddev)` | `(f64, f64) → f64` | Box-Muller Gaussian |
| `matchlab.rng_u64()` | `() → u64` | Raw u64 draw |

---

## Rating System

**Layer:** `plugins/rating/`
**Canonical example:** `plugins/rating/elo.lua`

### Required Functions

```lua
function initialize(player_id, config, context)
    -- Returns: {rating, rating_deviation, volatility, games_played}, context
end

function predict(team_a, team_b, config, context)
    -- team_a, team_b: arrays of observation tables
    -- Returns: win_probability (f64), context
end

function update(match_result, observations, config, context)
    -- match_result: table with match outcome
    -- observations: map of player_id → observation table
    -- Returns: array of {player_id, rating, rating_deviation, volatility, games_played}, context
end
```

### Optional Globals

| Global | Type | Description |
|--------|------|-------------|
| `information_budget` | `{string, ...}` | Observation types used. Default: `{"WinLoss"}`. Options: `"WinLoss"`, `"Score"`, `"Kills"`, `"Deaths"`, `"Assists"`, `"ObjectiveScore"`, `"Impact"`, `"Duration"`, `"Disconnects"`, `"SessionHistory"`, `"QuitBehavior"` |

### Information Budget

The loop sanitizes `MatchResult` before calling `update` based on the declared budget. A `WinLoss`-only system never sees scores, per-player performances, or durations.

### Observation Table Fields

`player_id`, `rating`, `rating_deviation`, `volatility`, `games_played`, `win_rate`, `tilt_level`, `is_online`, `recent_performances` (array), `queue_joined_at_secs`, `party_id`, `role` (string or nil)

### Lifecycle

1. `initialize(player_id)` — called once per player at population generation
2. `predict(team_a, team_b)` — called pre-match (informational)
3. `update(match_result, observations)` — called post-match after budget filter

### Worked Example: Elo

```lua
-- plugins/rating/elo.lua
information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    return {
        rating = config.initial_rating or 1000.0,
        rating_deviation = 350.0,
        volatility = 0.0,
        games_played = 0,
    }
end

function predict(team_a, team_b, config, context)
    local avg_a = 0
    for _, p in ipairs(team_a) do avg_a = avg_a + p.rating end
    avg_a = avg_a / #team_a
    local avg_b = 0
    for _, p in ipairs(team_b) do avg_b = avg_b + p.rating end
    avg_b = avg_b / #team_b
    local beta = config.beta or 400.0
    local diff = avg_a - avg_b
    local divisor = beta * math.log(10)
    return 1.0 / (1.0 + 10 ^ (-diff / divisor))
end

function update(match_result, observations, config, context)
    local k = config.k_factor or 32.0
    local beta = config.beta or 400.0
    local divisor = beta * math.log(10)
    local updates = {}
    local teams = {A = match_result.team_a, B = match_result.team_b}
    local actual = match_result.winner == "A" and 1.0 or 0.0
    for _, pid in ipairs(match_result.team_a) do
        local obs = observations[pid]
        local expected = 1.0 / (1.0 + 10 ^ ((obs.rating - avg_b) / divisor))
        local new_rating = obs.rating + k * (actual - expected)
        table.insert(updates, {
            player_id = pid,
            rating = new_rating,
            rating_deviation = obs.rating_deviation,
            volatility = obs.volatility,
            games_played = obs.games_played + 1,
        })
    end
    -- (mirror for team_b with actual = 0.0)
    return updates
end
```

---

## Outcome Model

**Layer:** `plugins/game/`
**Canonical example:** `plugins/game/logistic.lua`

### Required Functions

```lua
function win_probability(team_a, team_b, config, context)
    -- team_a, team_b: arrays of observation tables WITH skill fields
    -- Returns: probability (f64), context
end

function simulate(match_id, team_a, team_b, config, context)
    -- match_id: integer
    -- Returns: result_table, context
end
```

### Observation Table Fields (with skill)

All rating-system fields, plus: `skill_overall` (f64), `skill_vector` (table of `{dim_name → f64}`)

### Result Table

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `winner` | `"A"` or `"B"` | `"A"` | Match winner |
| `team_a` | `{id, ...}` | `{}` | Team A player ids |
| `team_b` | `{id, ...}` | `{}` | Team B player ids |
| `team_a_score` | `f64` | `13.0` | Team A score |
| `team_b_score` | `f64` | `5.0` | Team B score |
| `duration_secs` | `f64` | `1800.0` | Match duration in seconds |
| `variance` | `f64` | `0.0` | Outcome noise magnitude |
| `disconnected` | `bool` | `false` | Disconnect occurred |
| `forfeited` | `bool` | `false` | Forfeit occurred |
| `performances` | `{row, ...}` | `{}` | Per-player performance rows |

Performance row: `{player_id, kills, deaths, assists, objective_score, impact, variance}`

### Truth Separation

Outcome models are the **only** subsystem that reads ground-truth skill. The observation tables carry `skill_overall` and `skill_vector` so match winners are decided by true skill, not by ratings.

### Worked Example: Logistic

```lua
-- plugins/game/logistic.lua
function win_probability(team_a, team_b, config, context)
    local function effective_skill(player)
        return player.skill_overall or player.rating
    end
    local sum_a, sum_b = 0, 0
    for _, p in ipairs(team_a) do sum_a = sum_a + effective_skill(p) end
    for _, p in ipairs(team_b) do sum_b = sum_b + effective_skill(p) end
    local avg_a = sum_a / #team_a
    local avg_b = sum_b / #team_b
    local diff = avg_a - avg_b
    local beta = config.beta or 400.0
    return 1.0 / (1.0 + 10 ^ (-diff / (beta * math.log(10))))
end

function simulate(match_id, team_a, team_b, config, context)
    local p = win_probability(team_a, team_b, config, context)
    local noise = config.noise or 0.2
    local roll = matchlab.rng_range(0.0, 1.0)
    local adjusted_p = p + matchlab.rng_normal(0.0, noise)
    adjusted_p = math.max(0.0, math.min(1.0, adjusted_p))
    local winner = adjusted_p > roll and "A" or "B"
    local a_ids, b_ids = {}, {}
    for _, p in ipairs(team_a) do table.insert(a_ids, p.player_id) end
    for _, p in ipairs(team_b) do table.insert(b_ids, p.player_id) end
    return {
        winner = winner,
        team_a = a_ids,
        team_b = b_ids,
        team_a_score = winner == "A" and 13.0 or 5.0,
        team_b_score = winner == "A" and 5.0 or 13.0,
        duration_secs = 1800.0,
        variance = noise,
    }
end
```

---

## Matchmaker

**Layer:** `plugins/matchmaking/`
**Canonical example:** `plugins/matchmaking/batch.lua`

### Required Functions

```lua
function find_matches(queue, teams, now_secs, config, context)
    -- queue: array of queue entry tables
    -- teams: {a = {size, role?}, b = {size, role?}}
    -- now_secs: current simulation time in seconds
    -- Returns: array of {team_a, team_b, quality_score?}, context
end
```

### Queue Entry Table

| Field | Type | Description |
|-------|------|-------------|
| `idx` | `int` | Original queue position (stable tiebreaker) |
| `player_id` | `u64` | Player identifier |
| `rating` | `f64` | Visible rating |
| `rating_deviation` | `f64` | Rating uncertainty |
| `games_played` | `u64` | Games played |
| `win_rate` | `f64` | Win rate |
| `joined_at_secs` | `f64` | Join time in seconds |
| `wait_secs` | `f64` | Current wait time |
| `region` | `string` | Region (`"na"`, `"eu"`, `"asia"`, `"other"`) |
| `party_id` | `u64` or `nil` | Party identifier |
| `latency_ms` | `f64` | Estimated latency |
| `game_mode` | `string` | Game mode label |
| `role` | `string` or `nil` | Queued role |

### Return Value

Array of proposals. Each:
- `team_a`: array of `player_id` integers
- `team_b`: array of `player_id` integers
- `quality_score` (optional): computed automatically if absent

### Truth Separation

Queue entries carry **observations only** — never `PlayerReality`. The matchmaker cannot see ground-truth skill.

### Worked Example: Batch (Rating-Balanced)

```lua
-- plugins/matchmaking/batch.lua
function find_matches(queue, teams, now_secs, config, context)
    local size_a = teams.a.size
    local size_b = teams.b.size
    local total = size_a + size_b
    -- Sort by rating (ties by idx for determinism)
    table.sort(queue, function(a, b)
        if a.rating ~= b.rating then return a.rating < b.rating end
        return a.idx < b.idx
    end)
    local matches = {}
    local used = {}
    for i = 1, #queue - total + 1, total do
        local team_a, team_b = {}, {}
        local slot = 0
        for j = 0, total - 1 do
            local entry = queue[i + j]
            if entry and not used[entry.player_id] then
                if slot < size_a then
                    table.insert(team_a, entry.player_id)
                else
                    table.insert(team_b, entry.player_id)
                end
                used[entry.player_id] = true
                slot = slot + 1
            end
        end
        if #team_a == size_a and #team_b == size_b then
            table.insert(matches, {team_a = team_a, team_b = team_b})
        end
    end
    return matches
end
```

---

## Metric Collector

**Layer:** `plugins/metrics/`
**Canonical example:** `plugins/metrics/match_quality.lua`

### Required Functions

```lua
function on_record(match_result, snapshot, config, context)
    -- Accumulate data for one match. Store in context.
    -- Returns: context
end

function compute(config, context)
    -- Finalize the metric.
    -- Returns: result_table, context
end
```

### Optional Globals

| Global | Type | Description |
|--------|------|-------------|
| `name` | `string` | **Required.** Metric name used as the key in results. |
| `needs_population` | `bool` | If `true`, snapshot includes full population. Default: `false`. |
| `time_buckets` | `function(config, context)` | Returns bucket edges for time-series metric. |

### Snapshot Table

| Field | Type | Description |
|-------|------|-------------|
| `match_result` | table | Match outcome (same fields as Outcome Model) |
| `tick` | `u64` | Current tick |
| `time_secs` | `f64` | Current time in seconds |
| `players` | `{row, ...}` | Per-participant rows with all observation fields plus `true_skill`, `skill_overall`, `skill_vector`, `improvement_rate`, `reality_games_played`, `archetype` |

### Compute Return Table

| `kind` | Shape | Description |
|--------|-------|-------------|
| `"scalar"` | `{kind="scalar", value=f64}` | Single value |
| `"distribution"` | `{kind="distribution", values={f64,...}}` | Raw distribution |
| `"summary"` | `{kind="summary", values={f64,...}}` | Auto-computed summary stats |

### Truth Separation

Metrics are the **legitimate** reader of `PlayerReality`. The snapshot includes `true_skill` and other reality fields.

### Worked Example: Match Quality

```lua
-- plugins/metrics/match_quality.lua
name = "match_quality"

function on_record(match_result, snapshot, config, context)
    context.samples = context.samples or {}
    -- Compute quality from observation ratings
    local sum_a, sum_b = 0, 0
    for _, p in ipairs(snapshot.players) do
        -- (simplified — real script reads team assignment)
    end
    -- Quality = 1 - |avg_a - avg_b| / 400
    local q = 1.0  -- placeholder
    table.insert(context.samples, q)
end

function compute(config, context)
    local samples = context.samples or {}
    if #samples == 0 then return {kind = "scalar", value = 0.0} end
    local sum = 0
    for _, v in ipairs(samples) do sum = sum + v end
    return {kind = "scalar", value = sum / #samples}
end
```

---

## Detection System

**Layer:** `plugins/detection/`
**Canonical example:** `plugins/detection/smurf.lua`

### Required Functions

```lua
function observe(match_result, observations, config, context)
    -- Ingest match result. Accumulate per-player evidence in context.
    -- Returns: context
end

function evaluate(player_id, observations, config, context)
    -- Assess a specific player.
    -- Returns: {player_id, probability_of_anomaly, confidence, evidence}, context
end

function recommend_action(result, config, context)
    -- Map detection result to intervention.
    -- Returns: action_string, context
end
```

### Action Strings

| String | Effect |
|--------|--------|
| `"None"` | No action |
| `"AccelerateRating"` | Rating multiplier (1.5x) |
| `"IncreaseKFactor"` | Higher K-factor (32.0) |
| `"FlagForReview"` | Flag for human review |
| `"RestrictQueue"` | Queue restriction (100 ticks) |
| `"TempBan"` | Temporary ban (500 ticks) |
| `"Probation"` | Probation (1000 ticks) |
| `"Ban"` | Permanent ban |

### Truth Separation

Detection systems receive **observations only** — never `PlayerReality`. Smurf status must be inferred from behavior, never from ground-truth skill.

---

## Adversarial Agent

**Layer:** `plugins/adversarial/`
**Canonical example:** `plugins/adversarial/afk.lua`

### Required Functions

```lua
function tick(player_id, behavior, observation, config, context)
    -- Modify player behavior each tick.
    -- behavior: read/write table (quit_probability, party_id, tilt_level, win_rate, is_online)
    -- observation: read-only observation table
    -- Returns: behavior, context
end

function objective(config, context)
    -- Declare the agent's goal. Called once at load time.
    -- Returns: {kind = "..."}
end
```

### Behavior Table (read/write)

| Field | Type | Description |
|-------|------|-------------|
| `quit_probability` | `f64` | 0.0–1.0, probability of quitting |
| `party_id` | `u64` or `nil` | Party to join |
| `tilt_level` | `f64` | 0.0–1.0, tilt level |
| `win_rate` | `f64` | 0.0–1.0, win rate |
| `is_online` | `bool` | Whether player is online |

### Objective Kinds

`"MaximizeRating"`, `"MinimizeGamesPlayed"`, `"MaximizeWinRate"`, `"MaintainLowRating"`, `"Derate"`, `"WinTrade"`

### Worked Example: AFK

```lua
-- plugins/adversarial/afk.lua
function tick(player_id, behavior, observation, config, context)
    local go_afk = config.go_afk_probability or 0.1
    if matchlab.rng_bool(go_afk) then
        behavior.quit_probability = 1.0
    end
    return behavior
end

function objective(config, context)
    return {kind = "MinimizeGamesPlayed"}
end
```

---

## Satisfaction Model

**Layer:** `plugins/utility/`
**Canonical example:** `plugins/utility/satisfaction.lua`

### Required Functions

```lua
function satisfaction(experience, config, context)
    -- Compute satisfaction score.
    -- Returns: f64
end

function retention_probability(satisfaction, config, context)
    -- Probability of player retention.
    -- Returns: f64 (0.0–1.0)
end

function rematch_probability(satisfaction, config, context)
    -- Probability of rematch.
    -- Returns: f64 (0.0–1.0)
end
```

### Experience Table

| Field | Type | Description |
|-------|------|-------------|
| `recent_match_qualities` | `{f64, ...}` | Recent match quality scores |
| `recent_queue_times` | `{f64, ...}` | Recent queue wait times (seconds) |
| `recent_outcomes` | `{bool, ...}` | Recent win/loss outcomes |
| `current_streak` | `i64` | Win/loss streak (+ wins, − losses) |
| `rank_change` | `f64` | Rank change since last evaluation |
| `perceived_fairness` | `f64` | 0.0–1.0 perceived fairness |
| `rematch_rate` | `f64` | Rate of rematches |

### Lifecycle

1. `satisfaction(experience)` — called per-player after match end
2. `retention_probability(s)` — called with the satisfaction score
3. If retention is below threshold, the loop schedules `PlayerQuit` instead of re-queue

### Worked Example: Weighted Sum

```lua
-- plugins/utility/satisfaction.lua
function satisfaction(exp, config, context)
    local q_weight = config.match_quality or 1.0
    local t_penalty = config.queue_time_penalty or 0.5
    local w_bonus = config.win_bonus or 1.0
    local streak_penalty = config.loss_streak_penalty or 0.3
    local score = 0
    -- Average match quality contribution
    for _, q in ipairs(exp.recent_match_qualities) do
        score = score + q * q_weight
    end
    -- Queue time penalty
    for _, t in ipairs(exp.recent_queue_times) do
        score = score - t * t_penalty
    end
    -- Win bonus
    for _, w in ipairs(exp.recent_outcomes) do
        if w then score = score + w_bonus end
    end
    -- Loss streak penalty
    if exp.current_streak < -3 then
        score = score + exp.current_streak * streak_penalty
    end
    return score
end

function retention_probability(s, config, context)
    return 1.0 / (1.0 + math.exp(-s))
end

function rematch_probability(s, config, context)
    return 1.0 / (1.0 + math.exp(-0.5 * (s - 2.0)))
end
```

---

## Script Validation

At load time, `validate_script` enforces:

1. **Source scan** — rejects any script containing `math.random`
2. **Execution** — runs the script in a fresh Lua state
3. **Function check** — verifies every required function exists as a global
4. **Report** — returns a `ValidationReport` listing defined functions

Scripts that fail validation produce an error before the experiment starts.
