-- plugins/rating/elo.lua
-- Classic Elo on a logistic scale consistent with the game model.
-- config: k_factor, initial_rating, beta
--
-- divisor = beta * ln(10) keeps the log10 Elo scale aligned with the logistic
-- game model, so both compute the same win probability for a rating gap.

data_requirements = {
    observation_fields = { "rating", "rating_deviation", "volatility", "games_played" },
    match_result_fields = { "winner", "team_a", "team_b" },
    request_fields = { "player_id" },
}

function initialize(data, config, context)
    local player_id = data.player_id
    return {
        rating = config.initial_rating,
        rating_deviation = 350.0,
        volatility = 0.06,
        games_played = 0,
    }, context
end

function predict(data, context)
    local config = _matchlab_config
    local avg_a = team_average(data.team_a)
    local avg_b = team_average(data.team_b)
    return expected_score(avg_a, avg_b, config.beta)
end

function expected_score(rating_a, rating_b, beta)
    local beta = beta or 400.0
    local divisor = beta * math.log(10.0)
    if divisor == 0.0 then return 0.5 end
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
    local config = _matchlab_config
    local match_result = data.match_result
    local observations = data.observations
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