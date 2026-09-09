-- plugins/rating/colley.lua
-- Colley rating: regularized win/loss system. r_i = 1 + (w_i - l_i) / (2 + n_i).
-- Online approximation: maintains win/loss counts per player.
-- config: initial_rating

information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    local key = tostring(player_id)
    context[key] = { wins = 0, losses = 0 }
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
    local function update_team(ids, won)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local key = tostring(id)
                local state = context[key] or { wins = 0, losses = 0 }
                if won then
                    state.wins = state.wins + 1
                else
                    state.losses = state.losses + 1
                end
                context[key] = state
                local n = state.wins + state.losses
                local colley_rating = 1.0 + (state.wins - state.losses) / (2.0 + n)
                local scale = 400.0
                table.insert(updates, {
                    player_id = id,
                    rating = config.initial_rating + colley_rating * scale,
                    rating_deviation = o.rating_deviation,
                    volatility = o.volatility,
                    games_played = o.games_played + 1,
                })
            end
        end
    end

    update_team(match_result.team_a, team_a_won)
    update_team(match_result.team_b, not team_a_won)
    return updates, context
end
