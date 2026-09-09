-- plugins/rating/massey.lua
-- Massey rating: uses score differentials to build a linear system Mr = p.
-- Online approximation: maintains running sums and solves via regularization.
-- config: initial_rating, learning_rate, regularization

information_budget = { "ScoreDifference" }

function initialize(player_id, config, context)
    local key = tostring(player_id)
    context[key] = { games = 0, score_diff_sum = 0.0 }
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
    local lr = config.learning_rate or 1.0
    local reg = config.regularization or 1.0
    local team_a_won = match_result.winner == "A"
    local score_diff = match_result.score_a - match_result.score_b
    local sign = team_a_won and 1.0 or -1.0
    local abs_diff = math.abs(score_diff)
    if abs_diff < 1.0 then abs_diff = 1.0 end

    local updates = {}
    local function update_team(ids, team_sign)
        for _, id in ipairs(ids) do
            local o = observations[id]
            if o then
                local key = tostring(id)
                local state = context[key] or { games = 0, score_diff_sum = 0.0 }
                state.games = state.games + 1
                state.score_diff_sum = state.score_diff_sum + sign * team_sign * abs_diff
                context[key] = state
                local normalized_diff = state.score_diff_sum / math.max(state.games, 1)
                local delta = lr * normalized_diff / (state.games + reg)
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

    update_team(match_result.team_a, 1.0)
    update_team(match_result.team_b, -1.0)
    return updates, context
end
