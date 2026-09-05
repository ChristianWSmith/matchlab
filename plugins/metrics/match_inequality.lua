-- plugins/metrics/match_inequality.lua
-- Distribution of expected win probabilities: a well-matched system clusters
-- near 0.5, a poorly-matched system has a fat tail.

name = "match_inequality"

function on_record(match_result, snapshot, config, context)
    context.values = context.values or {}
    local ratings = index_ratings(snapshot.players)
    local avg_a = team_average(match_result.team_a, ratings)
    local avg_b = team_average(match_result.team_b, ratings)
    local p = 1.0 / (1.0 + 10.0 ^ ((avg_b - avg_a) / 400.0))
    table.insert(context.values, p)
    return context
end

function compute(config, context)
    return { kind = "summary", values = context.values or {} }
end

function index_ratings(players)
    local ratings = {}
    for _, p in ipairs(players) do
        ratings[p.player_id] = p.rating
    end
    return ratings
end

function team_average(ids, ratings)
    local sum, n = 0.0, 0
    for _, id in ipairs(ids) do
        if ratings[id] ~= nil then
            sum = sum + ratings[id]
            n = n + 1
        end
    end
    if n == 0 then return 0.0 end
    return sum / n
end