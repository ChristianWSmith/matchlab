-- plugins/matchmaking/custom_formation.lua
-- Custom match-quality formula: queue-time-aware tolerance.

function on_match_quality(team_a_avg, team_b_avg, queue_times)
    local diff = math.abs(team_a_avg - team_b_avg)
    local max_wait = 0
    for _, t in ipairs(queue_times) do
        if t > max_wait then max_wait = t end
    end
    local tolerance = 200.0 + max_wait * 5.0
    return 1.0 - math.min(diff / tolerance, 1.0)
end

function on_accept_match(team_a, team_b, quality, now)
    return quality > 0.8
end