-- plugins/metrics/custom_metric.lua
-- Custom metric hook: records balance as absolute difference from 0.5 win prob.

function on_record(winner, team_a_avg, team_b_avg)
    local diff = math.abs(team_a_avg - team_b_avg)
    return diff / 400.0
end

function on_bucket_config()
    return {0.0, 1000.0, 2000.0, 3000.0, 4000.0, 5000.0}
end
