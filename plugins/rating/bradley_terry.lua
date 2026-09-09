-- plugins/rating/bradley_terry.lua
-- Bradley-Terry pairwise comparison model.
-- P(i>j) = theta_i / (theta_i + theta_j) where theta_i = exp(r_i)
-- Updates use online gradient ascent on the log-likelihood.
-- config: initial_rating, learning_rate

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
    return 1.0 / (1.0 + math.exp(-(avg_a - avg_b)))
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
    local lr = config.learning_rate or 1.0
    local team_a_won = match_result.winner == "A"
    local avg_a = team_avg(observations, match_result.team_a)
    local avg_b = team_avg(observations, match_result.team_b)
    local diff = avg_a - avg_b
    local p_a = 1.0 / (1.0 + math.exp(-diff))

    local updates = {}
    local actual_a = team_a_won and 1.0 or 0.0
    local actual_b = 1.0 - actual_a

    for _, id in ipairs(match_result.team_a) do
        local o = observations[id]
        if o then
            table.insert(updates, {
                player_id = id,
                rating = o.rating + lr * (actual_a - p_a),
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    for _, id in ipairs(match_result.team_b) do
        local o = observations[id]
        if o then
            table.insert(updates, {
                player_id = id,
                rating = o.rating + lr * (actual_b - (1.0 - p_a)),
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
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
