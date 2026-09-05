-- plugins/rating/trueskill.lua
-- TrueSkill (Herbrich, Minka, Graepel): each player is N(mu, sigma^2); team
-- performance is the sum of member performances; the comparison d = P_A - P_B
-- is modeled as N(mu_A - mu_B, c^2) and updated via truncated-Gaussian
-- conditioning (inverse-Mills-ratio factors v, w).
-- config: initial_mean (or initial_rating), initial_variance, beta, dynamics,
--         draw_probability

information_budget = { "WinLoss" }

local SQRT_2PI = 2.5066282746310002

function normal_pdf(x)
    return math.exp(-x * x / 2.0) / SQRT_2PI
end

-- Abramowitz-Stegun 7.1.26 approximation of the standard normal CDF.
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

-- Standard normal quantile (probit) via Newton iteration on the CDF.
function normal_quantile(p)
    if p <= 0.0 then return -math.huge end
    if p >= 1.0 then return math.huge end
    local x = 0.0
    for _ = 1, 50 do
        local err = normal_cdf(x) - p
        local pdf = normal_pdf(x)
        if math.abs(pdf) < 1e-15 then break end
        local dx = err / pdf
        x = x - dx
        if math.abs(dx) < 1e-12 then break end
    end
    return x
end

-- Truncated-Gaussian update factors: mu' = mu + (sigma^2/c) v,
-- sigma'^2 = sigma^2 (1 - (sigma^2/c^2) w).
function win_factors(t, u)
    local alpha = u - t
    local v = normal_pdf(alpha) / (1.0 - normal_cdf(alpha))
    return v, v * (v + t - u)
end

function loss_factors(t, u)
    local beta = -u - t
    local m = normal_pdf(beta) / math.max(normal_cdf(beta), 1e-15)
    return -m, m * (m + beta)
end

function initialize(player_id, config, context)
    return {
        rating = config.initial_mean or config.initial_rating,
        rating_deviation = math.sqrt(config.initial_variance),
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

function update(match_result, observations, config, context)
    local team_a_won = match_result.winner == "A"
    local dynamics = config.dynamics or 0.0
    local beta = config.beta
    local draw_probability = config.draw_probability or 0.0

    local u = 0.0
    if draw_probability > 0.0 then
        u = normal_quantile((1.0 + draw_probability) / 2.0)
    end

    local function collect(ids)
        local sum_mu, sum_var = 0.0, 0.0
        local sigmas = {}
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local mu = o.rating
                local sigma = o.rating_deviation
                local var = sigma * sigma + dynamics * dynamics
                sum_mu = sum_mu + mu
                sum_var = sum_var + var
                table.insert(sigmas, sigma)
            end
        end
        return sum_mu, sum_var, sigmas
    end

    local sum_mu_a, sum_var_a, sigmas_a = collect(match_result.team_a)
    local sum_mu_b, sum_var_b, sigmas_b = collect(match_result.team_b)
    local n = #match_result.team_a + #match_result.team_b
    local c = math.sqrt(sum_var_a + sum_var_b + n * beta * beta)
    if c == 0.0 then
        return {}, context
    end

    local t = (sum_mu_a - sum_mu_b) / c
    local v_a, w_a, v_b, w_b
    if team_a_won then
        v_a, w_a = win_factors(t, u)
        v_b, w_b = loss_factors(t, u)
    else
        v_a, w_a = loss_factors(t, u)
        v_b, w_b = win_factors(t, u)
    end

    local updates = {}
    local function update_team(ids, sigmas, v, w)
        for i, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local sigma = sigmas[i] or o.rating_deviation
                local var = sigma * sigma
                local mu_new = o.rating + (var / c) * v
                local var_new = var * (1.0 - (var / (c * c)) * w)
                table.insert(updates, {
                    player_id = id,
                    rating = mu_new,
                    rating_deviation = math.max(math.sqrt(var_new), 1e-6),
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