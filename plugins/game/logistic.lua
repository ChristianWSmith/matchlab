-- plugins/game/logistic.lua
-- Logistic outcome model: P(A wins) = logistic of the average-skill difference.
-- config: beta, noise
--
-- Observations carry `skill_overall` (the ground-truth skill binding set from
-- PlayerReality at population generation), so match winners are decided by true
-- skill — this is what makes rating convergence a real property.

function effective_skill(o)
    if o.skill_overall ~= nil then
        return o.skill_overall
    end
    return o.rating
end

function team_average(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + effective_skill(o)
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function win_probability(team_a, team_b, config, context)
    local diff = team_average(team_a) - team_average(team_b)
    return 1.0 / (1.0 + math.exp(-diff / config.beta))
end

-- Draw order matters: it must mirror the reference implementation so results
-- are byte-identical for the same seed.
function simulate(match_id, team_a, team_b, config, context)
    local base_p = win_probability(team_a, team_b, config, context)
    local noise = matchlab.rng_range(-config.noise, config.noise)
    local adjusted_p = math.max(0.01, math.min(0.99, base_p + noise))
    local team_a_wins = matchlab.rng_bool(adjusted_p)

    local team_a_ids, team_b_ids = {}, {}
    for _, o in ipairs(team_a) do table.insert(team_a_ids, o.player_id) end
    for _, o in ipairs(team_b) do table.insert(team_b_ids, o.player_id) end

    local performances = {}
    for _, o in ipairs(team_a) do table.insert(performances, build_performance(o)) end
    for _, o in ipairs(team_b) do table.insert(performances, build_performance(o)) end

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

function build_performance(o)
    local skill = effective_skill(o)
    local perf_variance = matchlab.rng_range(0.0, 1.0)
    local aim = (o.skill_vector and o.skill_vector.aim) or skill
    return {
        player_id = o.player_id,
        kills = math.floor(math.max(aim / 100.0 + matchlab.rng_range(-2.0, 2.0), 0.0)),
        deaths = math.floor(math.max(5.0 - (skill / 1000.0) * 1.5 + matchlab.rng_range(-2.0, 2.0), 0.0)),
        assists = math.floor(math.max(3.0 + matchlab.rng_range(-2.0, 2.0), 0.0)),
        objective_score = matchlab.rng_range(0.0, 100.0) * (1.0 + skill / 3000.0),
        impact = matchlab.rng_range(-1.0, 1.0) + (skill - 1000.0) / 1500.0,
        variance = perf_variance,
    }
end