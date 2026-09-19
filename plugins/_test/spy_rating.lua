-- plugins/_test/spy_rating.lua
-- TEST-ONLY (never referenced by a manifest in experiments/): a mirror of
-- elo.lua whose `update` errors loudly if it can see data outside its declared
-- data_requirements. The simulation must hand this script only the fields it
-- declared — scores, duration, performances, and ground-truth skill must be
-- absent. A passing loop run proves the field-gating is wired.

data_requirements = {
    observation_fields = { "player_id", "rating", "rating_deviation", "volatility", "games_played" },
    match_result_fields = { "winner", "team_a", "team_b" },
    request_fields = { "player_id" },
}

function initialize(data, config, context)
    return {
        rating = config.initial_rating,
        rating_deviation = 350.0,
        volatility = 0.06,
        games_played = 0,
    }, context
end

function predict(data, context)
    local config = _matchlab_config
    return expected_score(team_average(data.team_a), team_average(data.team_b), config.beta)
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

function update(data, context)
    local match_result = data.match_result
    local observations = data.observations
    local config = _matchlab_config
    if match_result.team_a_score ~= nil then
        error("budget leak: team_a_score present")
    end
    if match_result.team_b_score ~= nil then
        error("budget leak: team_b_score present")
    end
    if match_result.duration_secs ~= nil then
        error("budget leak: duration_secs present")
    end
    if match_result.performances ~= nil then
        error("budget leak: performances present")
    end
    for _, o in pairs(observations) do
        if o.skill_overall ~= nil then
            error("budget leak: observation carries skill_overall")
        end
        if o.skill_vector ~= nil then
            error("budget leak: observation carries skill_vector")
        end
        if o.true_skill ~= nil then
            error("budget leak: observation carries true_skill")
        end
    end

    local team_a = match_result.team_a
    local team_b = match_result.team_b
    local expected_a = expected_score(team_average_ratings(team_a, observations),
                                      team_average_ratings(team_b, observations),
                                      config.beta)
    local expected_b = 1.0 - expected_a
    local actual_a = match_result.winner == "A" and 1.0 or 0.0
    local actual_b = 1.0 - actual_a

    local updates = {}
    update_team(updates, team_a, observations, config.k_factor, actual_a, expected_a)
    update_team(updates, team_b, observations, config.k_factor, actual_b, expected_b)
    return updates, context
end

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

function update_team(updates, ids, observations, k, actual, expected)
    for _, id in ipairs(ids) do
        local o = observations[id]
        if o then
            table.insert(updates, {
                player_id = id,
                rating = o.rating + k * (actual - expected),
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
end