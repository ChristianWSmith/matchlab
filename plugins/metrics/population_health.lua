-- plugins/metrics/population_health.lua
-- Rating inflation/deflation and compression across the run. Population-level
-- metric; only the first and latest rating snapshots are kept.

name = "population_health"
needs_population = true

function on_record(match_result, snapshot, config, context)
    if snapshot.population == nil then
        return context
    end
    if not context.snapshots then
        context.snapshots = {}
    end
    local ratings = {}
    for _, p in ipairs(snapshot.population) do
        table.insert(ratings, p.rating)
    end
    if #context.snapshots == 0 then
        table.insert(context.snapshots, ratings)
    end
    context.latest = ratings
    return context
end

function compute(config, context)
    local snapshots = context.snapshots or {}
    if #snapshots == 0 or context.latest == nil then
        return { kind = "scalar", value = 0.0 }
    end
    local first = snapshots[1]
    local last = context.latest
    local inflation = mean(last) - mean(first)
    local compression = stddev(first) - stddev(last)
    return {
        kind = "distribution",
        values = { inflation, compression, mean(first), mean(last) },
    }
end

function mean(values)
    local sum = 0.0
    for _, v in ipairs(values) do sum = sum + v end
    if #values == 0 then return 0.0 end
    return sum / #values
end

function stddev(values)
    local m = mean(values)
    local var = 0.0
    for _, v in ipairs(values) do
        var = var + (v - m) ^ 2
    end
    if #values == 0 then return 0.0 end
    return math.sqrt(var / #values)
end