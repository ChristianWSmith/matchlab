-- plugins/metrics/rating_accuracy.lua
-- MAE of visible rating vs true skill, sampled per match participant. Declares
-- `time_buckets` so the engine emits the rating_accuracy_by_time convergence
-- series (20 equal-width buckets over the run).

name = "rating_accuracy"

function on_record(match_result, snapshot, config, context)
    context.samples = context.samples or {}
    context.ticks = context.ticks or {}
    for _, p in ipairs(snapshot.players) do
        if p.true_skill ~= nil then
            table.insert(context.samples, math.abs(p.rating - p.true_skill))
            table.insert(context.ticks, snapshot.tick)
        end
    end
    return context
end

function compute(config, context)
    return { kind = "summary", values = context.samples or {} }
end

function time_buckets(config, context)
    local samples = context.samples or {}
    local ticks = context.ticks or {}
    if #samples == 0 then
        return nil
    end
    local n = 20
    local end_tick = 0
    for _, t in ipairs(ticks) do
        if t > end_tick then end_tick = t end
    end
    local width = end_tick == 0 and 1 or math.floor((end_tick + n - 1) / n)
    local sums = {}
    local counts = {}
    for i = 1, n do sums[i] = 0.0; counts[i] = 0 end
    for i, err in ipairs(samples) do
        local idx = math.min(ticks[i] // width, n - 1) + 1
        sums[idx] = sums[idx] + err
        counts[idx] = counts[idx] + 1
    end
    local out = {}
    for i = 1, n do
        out[i] = counts[i] > 0 and sums[i] / counts[i] or 0.0
    end
    return out
end