-- plugins/metrics/avg_rating_gap.lua
-- A novel metric: the mean absolute deviation of each participant's rating
-- from the cohort average rating across all match participants. Add a metric
-- by dropping one Lua file and listing its name in the manifest.

name = "avg_rating_gap"

function on_record(match_result, snapshot, config, context)
    context.gaps = context.gaps or {}
    local ratings = {}
    for _, p in ipairs(snapshot.players) do
        table.insert(ratings, p.rating)
    end
    local avg = mean(ratings)
    local sum_dev = 0.0
    for _, r in ipairs(ratings) do
        sum_dev = sum_dev + math.abs(r - avg)
    end
    if #ratings > 0 then
        table.insert(context.gaps, sum_dev / #ratings)
    end
    return context
end

function compute(config, context)
    return { kind = "scalar", value = mean(context.gaps or {}) }
end

function mean(values)
    if #values == 0 then return 0.0 end
    local sum = 0.0
    for _, v in ipairs(values) do sum = sum + v end
    return sum / #values
end