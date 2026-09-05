-- plugins/metrics/dimensionality_fidelity.lua
-- Correlation of 1D ratings and skill-vector predictions vs true overall skill;
-- fidelity = how much multiD improves over 1D. Population-level metric.

name = "dimensionality_fidelity"
needs_population = true

function on_record(match_result, snapshot, config, context)
    context.samples = context.samples or {}
    if snapshot.population == nil then
        return context
    end
    for _, p in ipairs(snapshot.population) do
        if p.true_skill ~= nil then
            table.insert(context.samples, { p.rating, p.skill_overall, p.true_skill })
        end
    end
    return context
end

function compute(config, context)
    local samples = context.samples or {}
    if #samples == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local oned = {}
    local multid = {}
    for _, s in ipairs(samples) do
        table.insert(oned, { s[1], s[3] })
        table.insert(multid, { s[2], s[3] })
    end
    local oned_corr = pearson(oned)
    local multid_corr = pearson(multid)
    local fidelity = 0.0
    if oned_corr > 0.0 then
        fidelity = math.max(0.0, math.min(1.0, (multid_corr - oned_corr) / (1.0 - oned_corr)))
    end
    return {
        kind = "summary",
        mean = oned_corr,
        median = multid_corr,
        p75 = fidelity,
        p90 = 0.0,
        p95 = 0.0,
        p99 = 0.0,
        stddev = 0.0,
    }
end

function pearson(pairs)
    if #pairs < 2 then return 0.0 end
    local n = #pairs
    local sum_x, sum_y, sum_xy, sum_x2, sum_y2 = 0.0, 0.0, 0.0, 0.0, 0.0
    for _, pair in ipairs(pairs) do
        sum_x = sum_x + pair[1]
        sum_y = sum_y + pair[2]
        sum_xy = sum_xy + pair[1] * pair[2]
        sum_x2 = sum_x2 + pair[1] * pair[1]
        sum_y2 = sum_y2 + pair[2] * pair[2]
    end
    local num = n * sum_xy - sum_x * sum_y
    local den = math.sqrt((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y))
    if den == 0.0 then return 0.0 end
    return num / den
end