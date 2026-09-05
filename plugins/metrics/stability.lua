-- plugins/metrics/stability.lua
-- Rating stddev for stable players (low improvement_rate); drifting players
-- are excluded.

name = "stability"

function on_record(match_result, snapshot, config, context)
    context.history = context.history or {}
    for _, p in ipairs(snapshot.players) do
        if p.improvement_rate ~= nil and math.abs(p.improvement_rate) < 0.1 then
            context.history[p.player_id] = context.history[p.player_id] or {}
            table.insert(context.history[p.player_id], p.rating)
        end
    end
    return context
end

function compute(config, context)
    local history = context.history or {}
    local variances = {}
    for _, ratings in pairs(history) do
        local mean = 0.0
        for _, r in ipairs(ratings) do mean = mean + r end
        mean = mean / #ratings
        local var = 0.0
        for _, r in ipairs(ratings) do
            var = var + (r - mean) ^ 2
        end
        table.insert(variances, var / #ratings)
    end
    if #variances == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local sum = 0.0
    for _, v in ipairs(variances) do sum = sum + v end
    return { kind = "scalar", value = math.sqrt(sum / #variances) }
end