-- plugins/rating/flat.lua
-- Fixed points for a win, fixed points for a loss. A baseline that
-- demonstrates why adaptive systems are needed.
-- config: win_points, loss_points, initial_rating

information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    return {
        rating = config.initial_rating,
        rating_deviation = 350.0,
        volatility = 0.0,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local avg_a = team_average(team_a)
    local avg_b = team_average(team_b)
    return 1.0 / (1.0 + 10.0 ^ ((avg_b - avg_a) / 400.0))
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
    local updates = {}
    for _, id in ipairs(match_result.team_a) do
        local o = observations[id]
        if o then
            local delta = team_a_won and config.win_points or -config.loss_points
            table.insert(updates, {
                player_id = id,
                rating = o.rating + delta,
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    for _, id in ipairs(match_result.team_b) do
        local o = observations[id]
        if o then
            local delta = team_a_won and -config.loss_points or config.win_points
            table.insert(updates, {
                player_id = id,
                rating = o.rating + delta,
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    return updates, context
end