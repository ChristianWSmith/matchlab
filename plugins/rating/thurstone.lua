-- plugins/rating/thurstone.lua
-- Thurstone-Mosteller model: performance X_i ~ N(mu_i, sigma_i^2).
-- P(i>j) = Phi((mu_i - mu_j) / sqrt(sigma_i^2 + sigma_j^2))
-- Updates use online gradient ascent on the log-likelihood.
-- config: initial_rating, initial_sigma, learning_rate

information_budget = { "WinLoss" }

local SQRT_2PI = 2.5066282746310002

function normal_pdf(x)
    return math.exp(-x * x / 2.0) / SQRT_2PI
end

function normal_cdf(x)
    local P = 0.2316419
    local B1 = 0.319381530
    local B2 = -0.356563782
    local B3 = 1.781477937
    local B4 = -1.821255978
    local B5 = 1.330274429
    if x >= 0.0 then
        local t = 1.0 / (1.0 + P * x)
        return 1.0 - normal_pdf(x)
                    * (B1 * t + B2 * t ^ 2 + B3 * t ^ 3 + B4 * t ^ 4 + B5 * t ^ 5)
    else
        return 1.0 - normal_cdf(-x)
    end
end

function initialize(player_id, config, context)
    return {
        rating = config.initial_rating,
        rating_deviation = config.initial_sigma or 250.0,
        volatility = 0.06,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local avg_a = team_average(team_a)
    local avg_b = team_average(team_b)
    local var_a = team_variance(team_a)
    local var_b = team_variance(team_b)
    local c = math.sqrt(var_a + var_b + 1.0)
    return normal_cdf((avg_a - avg_b) / c)
end

function team_average(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + o.rating
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function team_variance(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        local rd = o.rating_deviation or 250.0
        sum = sum + rd * rd
    end
    return sum
end

function update(match_result, observations, config, context)
    local lr = config.learning_rate or 0.1
    local team_a_won = match_result.winner == "A"
    local avg_a = team_avg(observations, match_result.team_a)
    local avg_b = team_avg(observations, match_result.team_b)
    local var_a = team_var(observations, match_result.team_a)
    local var_b = team_var(observations, match_result.team_b)
    local c = math.sqrt(var_a + var_b + 1.0)
    local t = (avg_a - avg_b) / c
    local p_a = normal_cdf(t)
    local pdf_a = normal_pdf(t)
    local actual_a = team_a_won and 1.0 or 0.0
    local actual_b = 1.0 - actual_a
    local gradient = pdf_a / math.max(p_a * (1.0 - p_a), 1e-10)

    local updates = {}
    local function update_team(ids, actual, sign)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local delta = lr * (actual - p_a) * sign / c
                table.insert(updates, {
                    player_id = id,
                    rating = o.rating + delta,
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

function team_var(observations, ids)
    local sum = 0.0
    for _, id in ipairs(ids) do
        local o = observations[id]
        if o then
            local rd = o.rating_deviation or 250.0
            sum = sum + rd * rd
        end
    end
    return sum
end
