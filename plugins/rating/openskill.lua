-- plugins/rating/openskill.lua
-- OpenSkill: Bayesian rating model inspired by TrueSkill.
-- Team performance: mu_T = sum(w_i * mu_i), sigma_T^2 = sum(w_i^2 * sigma_i^2)
-- P(A>B) = Phi((mu_A - mu_B) / sqrt(sigma_A^2 + sigma_B^2 + beta^2))
-- config: initial_mean (or initial_rating), initial_variance, beta, dynamics

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
    local sigma_sq = config.initial_variance or 62500.0
    return {
        rating = config.initial_mean or config.initial_rating,
        rating_deviation = math.sqrt(sigma_sq),
        volatility = 0.0,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local mu_a, var_a = team_stats(team_a)
    local mu_b, var_b = team_stats(team_b)
    local c = math.sqrt(var_a + var_b + config.beta * config.beta)
    if c == 0.0 then return 0.5 end
    return normal_cdf((mu_a - mu_b) / c)
end

function team_stats(team)
    local sum_mu, sum_var = 0.0, 0.0
    local n = #team
    for _, o in ipairs(team) do
        local weight = 1.0 / n
        sum_mu = sum_mu + weight * o.rating
        local rd = o.rating_deviation or 250.0
        sum_var = sum_var + weight * weight * rd * rd
    end
    return sum_mu, sum_var
end

function win_factors(t)
    local pdf = normal_pdf(t)
    local cdf = normal_cdf(t)
    local v = pdf / math.max(cdf, 1e-15)
    return v, v * (v + t)
end

function loss_factors(t)
    local pdf = normal_pdf(t)
    local cdf = normal_cdf(t)
    local m = pdf / math.max(1.0 - cdf, 1e-15)
    return -m, m * (m - t)
end

function update(match_result, observations, config, context)
    local team_a_won = match_result.winner == "A"
    local beta = config.beta
    local dynamics = config.dynamics or 0.0

    local function collect_stats(ids)
        local sum_mu, sum_var = 0.0, 0.0
        local n = #ids
        local weights = {}
        local sigmas = {}
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local w = 1.0 / n
                table.insert(weights, w)
                sum_mu = sum_mu + w * o.rating
                local var = o.rating_deviation * o.rating_deviation + dynamics * dynamics
                sum_var = sum_var + w * w * var
                table.insert(sigmas, o.rating_deviation)
            end
        end
        return sum_mu, sum_var, weights, sigmas
    end

    local mu_a, var_a, weights_a, sigmas_a = collect_stats(match_result.team_a)
    local mu_b, var_b, weights_b, sigmas_b = collect_stats(match_result.team_b)
    local c = math.sqrt(var_a + var_b + beta * beta)
    if c == 0.0 then return {}, context end

    local t = (mu_a - mu_b) / c
    local v_a, w_a, v_b, w_b
    if team_a_won then
        v_a, w_a = win_factors(t)
        v_b, w_b = loss_factors(t)
    else
        v_a, w_a = loss_factors(t)
        v_b, w_b = win_factors(t)
    end

    local updates = {}
    local function update_team(ids, weights, sigmas, v, w)
        for i, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local wi = weights[i] or (1.0 / #ids)
                local sigma = sigmas[i] or o.rating_deviation
                local var = sigma * sigma
                local mu_new = o.rating + (wi * var / c) * v
                local var_new = var * (1.0 - (wi * var / (c * c)) * w)
                table.insert(updates, {
                    player_id = id,
                    rating = mu_new,
                    rating_deviation = math.max(math.sqrt(math.max(var_new, 0.0)), 1e-6),
                    volatility = 0.0,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(match_result.team_a, weights_a, sigmas_a, v_a, w_a)
    update_team(match_result.team_b, weights_b, sigmas_b, v_b, w_b)
    return updates, context
end
