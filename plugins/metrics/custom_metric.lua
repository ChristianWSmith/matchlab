-- plugins/metrics/custom_metric.lua
-- Custom metric hook: records balance as absolute difference from 0.5 win prob.
-- This is a template; modify on_record/compute for your custom metric.

name = "custom_metric"

function on_record(match_result, snapshot, config, context)
    context.values = context.values or {}
    local diff = math.abs(match_result.team_a_score - match_result.team_b_score)
    table.insert(context.values, diff)
    return context
end

function compute(config, context)
    local values = context.values or {}
    if #values == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local sum = 0.0
    for _, v in ipairs(values) do
        sum = sum + v
    end
    return { kind = "scalar", value = sum / #values }
end
