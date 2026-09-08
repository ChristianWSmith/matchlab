-- plugins/_test/spy_rating.lua
-- TEST-ONLY (never referenced by a manifest in experiments/): a mirror of
-- elo.lua whose `update` errors loudly if it can see data outside its WinLoss
-- information budget. The simulation must hand this script the sanitized
-- result produced by filter_match_result -> into_match_result: scores zeroed,
-- duration zeroed, performances emptied, and no ground-truth skill keys in the
-- observation map. A passing loop run proves the budget sanitization is wired.

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
    return expected_score(team_average(team_a), team_average(team_b), config.beta)
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
    if match_result.team_a_score ~= 0.0 then
        error("budget leak: team_a_score " .. match_result.team_a_score)
    end
    if match_result.team_b_score ~= 0.0 then
        error("budget leak: team_b_score " .. match_result.team_b_score)
    end
    if match_result.duration_secs ~= 0.0 then
        error("budget leak: duration_secs " .. match_result.duration_secs)
    end
    if #match_result.performances ~= 0 then
        error("budget leak: performances " .. #match_result.performances)
    end
    for _, o in pairs(observations) do
        if o.skill_overall ~= nil then
            error("budget leak: observation carries skill_overall")
        end
        if o.skill_vector ~= nil then
            error("budget leak: observation carries skill_vector")
        end
        if o.true_skill ~= nil then
            error("budget leak: observation carries true_skill")
        end
    end

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