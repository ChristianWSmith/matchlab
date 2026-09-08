-- plugins/metrics/match_quality.lua
-- Balance quality: 1 - (|avg_a - avg_b| / 400), clamped to [0, 1].

name = "match_quality"

function on_record(match_result, snapshot, config, context)
    context.samples = context.samples or {}
    local ratings = index_ratings(snapshot.players)
    local avg_a = team_average(match_result.team_a, ratings)
    local avg_b = team_average(match_result.team_b, ratings)
    local diff = math.abs(avg_a - avg_b)
    table.insert(context.samples, 1.0 - math.min(diff / 400.0, 1.0))
    return context
end

function compute(config, context)
    return { kind = "summary", values = context.samples or {} }
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