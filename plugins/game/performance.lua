-- plugins/game/performance.lua
-- Performance model: hot/cold streaks tilt win probability. The mean of a
-- player's recent performances (scaled by performance_weight * beta) shifts
-- their effective skill.
-- config: beta, performance_weight

function base_skill(o)
    if o.skill_overall ~= nil then
        return o.skill_overall
    end
    return o.rating
end

function performance_boost(o, config)
    local recent = o.recent_performances
    if not recent or #recent == 0 then
        return 0.0
    end
    local sum = 0.0
    for _, v in ipairs(recent) do
        sum = sum + v
    end
    local mean = sum / #recent
    return (mean - 0.5) * config.performance_weight * config.beta
end

function effective_skill(o, config)
    return base_skill(o) + performance_boost(o, config)
end

function team_average(team, config)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + effective_skill(o, config)
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function win_probability(team_a, team_b, config, context)
    local diff = team_average(team_a, config) - team_average(team_b, config)
    return 1.0 / (1.0 + math.exp(-diff / config.beta))
end

function simulate(match_id, team_a, team_b, config, context)
    local base_p = win_probability(team_a, team_b, config, context)
    local noise = matchlab.rng_range(-0.05, 0.05)
    local adjusted_p = math.max(0.01, math.min(0.99, base_p + noise))
    local team_a_wins = matchlab.rng_bool(adjusted_p)

    local team_a_ids, team_b_ids = {}, {}
    for _, o in ipairs(team_a) do table.insert(team_a_ids, o.player_id) end
    for _, o in ipairs(team_b) do table.insert(team_b_ids, o.player_id) end

    local performances = {}
    for _, o in ipairs(team_a) do table.insert(performances, build_performance(o, config)) end
    for _, o in ipairs(team_b) do table.insert(performances, build_performance(o, config)) end

    local duration_secs = matchlab.rng_range(1200.0, 2400.0)
    local a_score = team_a_wins and 13.0 or matchlab.rng_range(4.0, 12.0)
    local b_score = team_a_wins and matchlab.rng_range(4.0, 12.0) or 13.0

    local result = {
        winner = team_a_wins and "A" or "B",
        team_a = team_a_ids,
        team_b = team_b_ids,
        team_a_score = a_score,
        team_b_score = b_score,
        duration_secs = duration_secs,
        performances = performances,
        variance = math.abs(noise),
        disconnected = false,
        forfeited = false,
    }
    return result, context
end

function build_performance(o, config)
    local skill = effective_skill(o, config)
    local perf_variance = matchlab.rng_range(0.0, 1.0)
    return {
        player_id = o.player_id,
        kills = math.floor(math.max(skill / 100.0 + matchlab.rng_range(-2.0, 2.0), 0.0)),
        deaths = math.floor(math.max(5.0 - (skill / 1000.0) * 1.5 + matchlab.rng_range(-2.0, 2.0), 0.0)),
        assists = math.floor(math.max(3.0 + matchlab.rng_range(-2.0, 2.0), 0.0)),
        objective_score = matchlab.rng_range(0.0, 100.0) * (1.0 + skill / 3000.0),
        impact = matchlab.rng_range(-1.0, 1.0) + (skill - 1000.0) / 1500.0,
        variance = perf_variance,
    }
end