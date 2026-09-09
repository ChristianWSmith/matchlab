-- plugins/rating/trueskill_through_time.lua
-- TrueSkill Through Time: TrueSkill with skill drift modeling.
-- Between observations: sigma_{t+1}^2 = sigma_t^2 + gamma^2
-- Match probability: P(i>j) = Phi((mu_i - mu_j) / sqrt(2*beta^2 + sigma_i^2 + sigma_j^2))
-- config: initial_mean (or initial_rating), initial_variance, beta, dynamics, gamma

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
    local key = tostring(player_id)
    local sigma_sq = config.initial_variance or 62500.0
    context[key] = { last_update = 0, sigma_sq = sigma_sq }
    return {
        rating = config.initial_mean or config.initial_rating,
        rating_deviation = math.sqrt(sigma_sq),
        volatility = 0.0,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local avg_a = team_average(team_a)
    local avg_b = team_average(team_b)
    return 1.0 / (1.0 + math.exp(-(avg_a - avg_b) / config.beta))
end

function team_average(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + o.rating
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function win_factors(t, u)
    local alpha = u - t
    local v = normal_pdf(alpha) / math.max(1.0 - normal_cdf(alpha), 1e-15)
    return v, v * (v + t - u)
end

function loss_factors(t, u)
    local beta_val = -u - t
    local m = normal_pdf(beta_val) / math.max(normal_cdf(beta_val), 1e-15)
    return -m, m * (m + beta_val)
end

function update(match_result, observations, config, context)
    local team_a_won = match_result.winner == "A"
    local beta = config.beta
    local gamma = config.gamma or 2.0
    local dynamics = config.dynamics or 0.0

    local function collect(ids)
        local sum_mu, sum_var = 0.0, 0.0
        local sigmas = {}
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local key = tostring(id)
                local state = context[key] or { sigma_sq = config.initial_variance or 62500.0 }
                local sigma_sq = state.sigma_sq + gamma * gamma
                local var = sigma_sq + dynamics * dynamics
                sum_mu = sum_mu + o.rating
                sum_var = sum_var + var
                table.insert(sigmas, math.sqrt(sigma_sq))
            end
        end
        return sum_mu, sum_var, sigmas
    end

    local sum_mu_a, sum_var_a, sigmas_a = collect(match_result.team_a)
    local sum_mu_b, sum_var_b, sigmas_b = collect(match_result.team_b)
    local n = #match_result.team_a + #match_result.team_b
    local c = math.sqrt(sum_var_a + sum_var_b + n * beta * beta)
    if c == 0.0 then return {}, context end

    local t = (sum_mu_a - sum_mu_b) / c
    local v_a, w_a, v_b, w_b
    if team_a_won then
        v_a, w_a = win_factors(t, 0.0)
        v_b, w_b = loss_factors(t, 0.0)
    else
        v_a, w_a = loss_factors(t, 0.0)
        v_b, w_b = win_factors(t, 0.0)
    end

    local updates = {}
    local function update_team(ids, sigmas, v, w)
        for i, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local key = tostring(id)
                local state = context[key] or { sigma_sq = config.initial_variance or 62500.0 }
                local sigma_sq = state.sigma_sq + gamma * gamma
                local sigma = sigmas[i] or math.sqrt(sigma_sq)
                local var = sigma * sigma
                local mu_new = o.rating + (var / c) * v
                local var_new = var * (1.0 - (var / (c * c)) * w)
                state.sigma_sq = math.max(var_new, 1e-6)
                context[key] = state
                table.insert(updates, {
                    player_id = id,
                    rating = mu_new,
                    rating_deviation = math.sqrt(state.sigma_sq),
                    volatility = 0.0,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(match_result.team_a, sigmas_a, v_a, w_a)
    update_team(match_result.team_b, sigmas_b, v_b, w_b)
    return updates, context
end
