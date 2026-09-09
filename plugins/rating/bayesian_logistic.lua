-- plugins/rating/bayesian_logistic.lua
-- Bayesian logistic rating: logistic regression with player identity as feature.
-- MAP estimation with Gaussian prior r_i ~ N(0, tau^2).
-- Online gradient ascent on the log-posterior.
-- config: initial_rating, prior_variance, learning_rate

information_budget = { "WinLoss" }

function sigmoid(x)
    if x > 20.0 then return 1.0 end
    if x < -20.0 then return 0.0 end
    return 1.0 / (1.0 + math.exp(-x))
end

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
    return sigmoid(avg_a - avg_b)
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
    local lr = config.learning_rate or 0.1
    local tau_sq = config.prior_variance or 62500.0
    local team_a_won = match_result.winner == "A"
    local avg_a = team_avg(observations, match_result.team_a)
    local avg_b = team_avg(observations, match_result.team_b)
    local diff = avg_a - avg_b
    local p_a = sigmoid(diff)
    local actual_a = team_a_won and 1.0 or 0.0
    local actual_b = 1.0 - actual_a

    local updates = {}
    local function update_team(ids, actual, sign)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local gradient = (actual - p_a) * sign - o.rating / tau_sq
                local hessian = -p_a * (1.0 - p_a) - 1.0 / tau_sq
                local delta = gradient / math.max(math.abs(hessian), 1e-10)
                table.insert(updates, {
                    player_id = id,
                    rating = o.rating + lr * delta,
                    rating_deviation = o.rating_deviation,
                    volatility = o.volatility,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(match_result.team_a, actual_a, 1.0)
    update_team(match_result.team_b, actual_b, -1.0)
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
