-- plugins/rating/glicko2.lua
-- Full Glicko-2 (Glickman 2012): scale to (mu, phi, sigma), per-opponent g/E,
-- v and Delta, Newton-Raphson volatility iteration, then scale back.
-- config: initial_rating, initial_rd, initial_volatility, tau, epsilon
-- Verified against the paper worked example (r'=1464.06, RD'=151.52, sigma'=0.05999).

information_budget = { "WinLoss" }

local SCALE = 173.7178
local RATING_CENTER = 1500.0

function initialize(player_id, config, context)
    return {
        rating = config.initial_rating,
        rating_deviation = config.initial_rd,
        volatility = config.initial_volatility,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local avg_a = team_average(team_a)
    local avg_b = team_average(team_b)
    return 1.0 / (1.0 + math.exp(-(avg_a - avg_b) / 400.0))
end

function team_average(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + o.rating
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function g(phi)
    return 1.0 / math.sqrt(1.0 + 3.0 * phi * phi / (math.pi * math.pi))
end

function e(mu, mu_j, phi_j)
    local gj = g(phi_j)
    return 1.0 / (1.0 + math.exp(-gj * (mu - mu_j)))
end

-- Newton-Raphson on Glicko-2's f(x) to find the new volatility (steps 5.2-5.6).
function new_volatility(sigma, delta, phi, v, tau, epsilon)
    local a = math.log(sigma * sigma)
    local function big_f(x)
        local ex = math.exp(x)
        return (ex * (delta * delta - phi * phi - v - ex))
                / (2.0 * (phi * phi + v + ex) * (phi * phi + v + ex))
            - (x - a) / (tau * tau)
    end

    local b
    if delta * delta > phi * phi + v then
        b = math.log(delta * delta - phi * phi - v)
    else
        local k = 1
        while big_f(a - k * tau) < 0.0 do
            k = k + 1
        end
        b = a - k * tau
    end

    local fa = big_f(a)
    local fb = big_f(b)
    local a_val = a
    local b_val = b
    while math.abs(b_val - a_val) > epsilon do
        local c = a_val + (a_val - b_val) * fa / (fb - fa)
        local fc = big_f(c)
        if fc * fb <= 0.0 then
            a_val = b_val
            fa = fb
        else
            fa = fa / 2.0
        end
        b_val = c
        fb = fc
    end

    -- Converged x = ln(sigma^2); sigma' = exp(x/2) = exp((a+b)/4).
    return math.exp((a_val + b_val) / 4.0)
end

function scale(rating, rd)
    return (rating - RATING_CENTER) / SCALE, rd / SCALE
end

function unscale(mu, phi)
    return RATING_CENTER + SCALE * mu, SCALE * phi
end

function update_player(mu, phi, sigma, opponents, epsilon, tau)
    local v_inv = 0.0
    local delta_numer = 0.0
    for _, opp in ipairs(opponents) do
        local mu_j, phi_j, outcome = opp[1], opp[2], opp[3]
        local gj = g(phi_j)
        local e_val = e(mu, mu_j, phi_j)
        v_inv = v_inv + gj * gj * e_val * (1.0 - e_val)
        delta_numer = delta_numer + gj * (outcome - e_val)
    end
    if v_inv == 0.0 then
        return mu, phi, sigma
    end
    local v = 1.0 / v_inv
    local delta = v * delta_numer
    local sigma_prime = new_volatility(sigma, delta, phi, v, tau, epsilon)
    local phi_star = math.sqrt(phi * phi + sigma_prime * sigma_prime)
    local phi_prime = 1.0 / math.sqrt(1.0 / (phi_star * phi_star) + 1.0 / v)
    local mu_prime = mu + phi_prime * phi_prime * delta_numer
    return mu_prime, phi_prime, sigma_prime
end

function update(match_result, observations, config, context)
    local epsilon = config.epsilon or 0.000001
    local tau = config.tau or 0.5
    local team_a = match_result.team_a
    local team_b = match_result.team_b
    local team_a_won = match_result.winner == "A"
    local outcome_a = team_a_won and 1.0 or 0.0
    local outcome_b = 1.0 - outcome_a

    local function collect_opponents(ids, outcome)
        local opponents = {}
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local mu, phi = scale(o.rating, o.rating_deviation)
                table.insert(opponents, { mu, phi, outcome })
            end
        end
        return opponents
    end

    local opp_b = collect_opponents(team_b, outcome_a)
    local opp_a = collect_opponents(team_a, outcome_b)

    local updates = {}
    local function update_team(ids, opponents)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local mu, phi = scale(o.rating, o.rating_deviation)
                local mu_p, phi_p, sigma_p = update_player(mu, phi, o.volatility,
                                                           opponents, epsilon, tau)
                local rating, rd = unscale(mu_p, phi_p)
                if config.floor and config.ceiling then
                    rating = math.max(config.floor, math.min(config.ceiling, rating))
                end
                table.insert(updates, {
                    player_id = id,
                    rating = rating,
                    rating_deviation = rd,
                    volatility = sigma_p,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(team_a, opp_b)
    update_team(team_b, opp_a)
    return updates, context
end