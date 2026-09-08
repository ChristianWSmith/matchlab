-- plugins/rating/elo.lua
-- Classic Elo on a logistic scale consistent with the game model.
-- config: k_factor, initial_rating, beta
--
-- divisor = beta * ln(10) keeps the log10 Elo scale aligned with the logistic
-- game model, so both compute the same win probability for a rating gap.

information_budget = { "WinLoss" }

function initialize(player_id, config, context)
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