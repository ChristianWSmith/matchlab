-- plugins/rating/bayesian_hierarchical.lua
-- Bayesian hierarchical rating: player ratings drawn from a population distribution.
-- r_i ~ N(mu_pop, sigma_pop^2), mu_pop ~ N(mu_0, tau^2)
-- Online variational approximation maintaining population statistics.
-- config: initial_rating, population_prior_mean, population_prior_variance,
--         learning_rate

information_budget = { "WinLoss" }

function sigmoid(x)
    if x > 20.0 then return 1.0 end
    if x < -20.0 then return 0.0 end
    return 1.0 / (1.0 + math.exp(-x))
end

function initialize(player_id, config, context)
    if not context.hierarchy then
        context.hierarchy = {
            pop_sum = 0.0,
            pop_sum_sq = 0.0,
            pop_count = 0,
            mu_pop = config.initial_rating,
            sigma_pop_sq = 62500.0,
        }
    end
    local h = context.hierarchy
    h.pop_sum = h.pop_sum + config.initial_rating
    h.pop_sum_sq = h.pop_sum_sq + config.initial_rating * config.initial_rating
    h.pop_count = h.pop_count + 1
    return {
        rating = config.initial_rating,
        rating_deviation = math.sqrt(h.sigma_pop_sq),
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
    local mu_0 = config.population_prior_mean or config.initial_rating
    local tau_sq = config.population_prior_variance or 62500.0
    local team_a_won = match_result.winner == "A"
    local avg_a = team_avg(observations, match_result.team_a)
    local avg_b = team_avg(observations, match_result.team_b)
    local diff = avg_a - avg_b
    local p_a = sigmoid(diff)
    local actual_a = team_a_won and 1.0 or 0.0
    local actual_b = 1.0 - actual_a

    if not context.hierarchy then
        context.hierarchy = {
            pop_sum = 0.0,
            pop_sum_sq = 0.0,
            pop_count = 0,
            mu_pop = config.initial_rating,
            sigma_pop_sq = 62500.0,
        }
    end
    local h = context.hierarchy

    local updates = {}
    local function update_team(ids, actual, sign)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local gradient = (actual - p_a) * sign
                local pop_pull = (h.mu_pop - o.rating) / math.max(h.sigma_pop_sq, 1.0)
                local total_gradient = gradient + pop_pull
                local hessian = -p_a * (1.0 - p_a) - 1.0 / math.max(h.sigma_pop_sq, 1.0)
                local delta = total_gradient / math.max(math.abs(hessian), 1e-10)
                local new_rating = o.rating + lr * delta
                table.insert(updates, {
                    player_id = id,
                    rating = new_rating,
                    rating_deviation = o.rating_deviation,
                    volatility = o.volatility,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(match_result.team_a, actual_a, 1.0)
    update_team(match_result.team_b, actual_b, -1.0)

    local all_ids = {}
    for _, id in ipairs(match_result.team_a) do table.insert(all_ids, id) end
    for _, id in ipairs(match_result.team_b) do table.insert(all_ids, id) end
    local pop_sum, pop_sum_sq, pop_count = 0.0, 0.0, 0
    for _, id in ipairs(all_ids) do
        for _, u in ipairs(updates) do
            if u.player_id == id then
                pop_sum = pop_sum + u.rating
                pop_sum_sq = pop_sum_sq + u.rating * u.rating
                pop_count = pop_count + 1
            end
        end
    end
    if pop_count > 0 then
        local sample_mean = pop_sum / pop_count
        local sample_var = pop_sum_sq / pop_count - sample_mean * sample_mean
        h.mu_pop = (mu_0 / tau_sq + pop_count * sample_mean / math.max(sample_var, 1.0)) /
                   (1.0 / tau_sq + pop_count / math.max(sample_var, 1.0))
        h.sigma_pop_sq = 1.0 / (1.0 / tau_sq + pop_count / math.max(sample_var, 1.0))
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
