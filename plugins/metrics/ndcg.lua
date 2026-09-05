-- plugins/metrics/ndcg.lua
-- Normalized discounted cumulative gain over match qualities: do high-quality
-- matches appear early?

name = "ndcg"

function on_record(match_result, snapshot, config, context)
    context.qualities = context.qualities or {}
    local ratings = index_ratings(snapshot.players)
    local avg_a = team_average(match_result.team_a, ratings)
    local avg_b = team_average(match_result.team_b, ratings)
    local p = 1.0 / (1.0 + math.exp(-(avg_a - avg_b) / 400.0))
    local quality = 1.0 - math.abs(p - 0.5) * 2.0
    table.insert(context.qualities, quality)
    return context
end

function compute(config, context)
    local qualities = context.qualities or {}
    if #qualities == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local ideal = {}
    for _, q in ipairs(qualities) do table.insert(ideal, q) end
    table.sort(ideal, function(a, b) return a > b end)
    local dcg, idcg = 0.0, 0.0
    for i, actual in ipairs(qualities) do
        local discount = math.log(i + 1) / math.log(2)
        dcg = dcg + actual / discount
        idcg = idcg + ideal[i] / discount
    end
    local ndcg = idcg > 0.0 and dcg / idcg or 0.0
    return { kind = "scalar", value = ndcg }
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