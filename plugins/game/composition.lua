-- plugins/game/composition.lua
-- Team composition model: effective skill is each player's weighted skill
-- vector; team totals add a synergy bonus per player. Can a 1D rating
-- represent multidimensional skill?
-- config: beta, dimension_weights = { dim -> weight }, synergy_bonus

function effective_skill(o, config)
    if not o.skill_vector or next(o.skill_vector) == nil then
        return o.rating
    end
    local weighted, weight_sum = 0.0, 0.0
    for dim, val in pairs(o.skill_vector) do
        local w = 1.0
        if config.dimension_weights ~= nil and config.dimension_weights[dim] ~= nil then
            w = config.dimension_weights[dim]
        end
        weighted = weighted + val * w
        weight_sum = weight_sum + w
    end
    if weight_sum == 0.0 then return o.rating end
    return weighted / weight_sum
end

function team_effective(team, config)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + effective_skill(o, config)
    end
    return sum + config.synergy_bonus * #team
end

function win_probability(team_a, team_b, config, context)
    local diff = team_effective(team_a, config) - team_effective(team_b, config)
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