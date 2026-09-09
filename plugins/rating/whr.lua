-- plugins/rating/whr.lua
-- Whole-History Rating: Bayesian inference over the entire match history.
-- Online approximation: maintains per-player win/loss history and computes
-- a running MAP estimate using logistic likelihood with Gaussian prior.
-- config: initial_rating, prior_variance, learning_rate

information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    local key = tostring(player_id)
    context[key] = { history = {}, posterior_rating = config.initial_rating }
    return {
        rating = config.initial_rating,
        rating_deviation = config.prior_variance or 250.0,
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

function sigmoid(x)
    if x > 20.0 then return 1.0 end
    if x < -20.0 then return 0.0 end
    return 1.0 / (1.0 + math.exp(-x))
end

function update(match_result, observations, config, context)
    local lr = config.learning_rate or 0.1
    local prior_var = config.prior_variance or 250.0
    local team_a_won = match_result.winner == "A"
    local avg_a = team_avg(observations, match_result.team_a)
    local avg_b = team_avg(observations, match_result.team_b)

    local updates = {}
    local function update_team(ids, outcome)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local key = tostring(id)
                local state = context[key] or { history = {}, posterior_rating = o.rating }
                local r = state.posterior_rating
                local opp_rating = outcome == 1.0 and avg_b or avg_a
                local diff = r - opp_rating
                local p = sigmoid(diff)
                local gradient = outcome - p
                local hessian = -p * (1.0 - p) - 1.0 / prior_var
                local delta = gradient / math.max(math.abs(hessian), 1e-10)
                state.posterior_rating = r + lr * delta
                context[key] = state
                table.insert(updates, {
                    player_id = id,
                    rating = state.posterior_rating,
                    rating_deviation = o.rating_deviation,
                    volatility = o.volatility,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(match_result.team_a, team_a_won and 1.0 or 0.0)
    update_team(match_result.team_b, team_a_won and 0.0 or 1.0)
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
