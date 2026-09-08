-- plugins/rating/decay_elo.lua
-- A novel rating system with no Rust equivalent: classic Elo plus an idle
-- decay toward the initial rating. A player who has been absent (many ticks
-- since their last update) drifts back toward `initial_rating`, modeling rating
-- decay for returning players.
--
-- Per-player state lives in `context` (the VM's context table, keyed by
-- player_id): `{ last_ticks }` tracks when the player last updated.
-- config: k_factor, initial_rating, beta, decay_rate (points per idle second)

information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    context[tostring(player_id)] = { last_ticks = 0 }
    return {
        rating = config.initial_rating,
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

function expected_score(rating_a, rating_b, beta)
    local divisor = beta * math.log(10.0)
    return 1.0 / (1.0 + 10.0 ^ ((rating_b - rating_a) / divisor))
end

function team_average(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + o.rating
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function update(match_result, observations, config, context)
    local decay_rate = config.decay_rate or 0.0
    local initial = config.initial_rating
    local expected_a = expected_score(team_avg(observations, match_result.team_a),
                                      team_avg(observations, match_result.team_b),
                                      config.beta)
    local actual_a = match_result.winner == "A" and 1.0 or 0.0
    local actual_b = 1.0 - actual_a

    local updates = {}
    for _, id in ipairs(match_result.team_a) do
        local o = observations[id]
        if o then
            local key = tostring(id)
            local state = context[key] or { last_ticks = 0 }
            local idle = o.games_played > 0 and match_result.duration_secs or 0
            local decay = (initial - o.rating) * decay_rate * idle
            table.insert(updates, {
                player_id = id,
                rating = o.rating + decay + config.k_factor * (actual_a - expected_a),
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
            state.last_ticks = match_result.duration_secs
            context[key] = state
        end
    end
    for _, id in ipairs(match_result.team_b) do
        local o = observations[id]
        if o then
            local key = tostring(id)
            local state = context[key] or { last_ticks = 0 }
            local idle = o.games_played > 0 and match_result.duration_secs or 0
            local decay = (initial - o.rating) * decay_rate * idle
            table.insert(updates, {
                player_id = id,
                rating = o.rating + decay + config.k_factor * (actual_b - (1.0 - expected_a)),
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
            state.last_ticks = match_result.duration_secs
            context[key] = state
        end
    end
    return updates, context
end

function team_avg(observations, ids)
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