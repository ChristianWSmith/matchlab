-- plugins/metrics/responsiveness.lua
-- Fraction of rating updates that move in the direction the outcome predicts
-- (winner gains, loser loses).

name = "responsiveness"

function on_record(match_result, snapshot, config, context)
    context.prev = context.prev or {}
    context.responses = context.responses or {}
    local winner_is_a = match_result.winner == "A"
    for _, p in ipairs(snapshot.players) do
        local prev = context.prev[p.player_id]
        context.prev[p.player_id] = p.rating
        if prev ~= nil then
            local delta = p.rating - prev
            if delta ~= 0.0 then
                local won = (team_has(match_result.team_a, p.player_id) and winner_is_a)
                    or (team_has(match_result.team_b, p.player_id) and not winner_is_a)
                local responsive = (delta > 0.0) == won
                table.insert(context.responses, responsive and 1 or 0)
            end
        end
    end
    return context
end

function compute(config, context)
    local responses = context.responses or {}
    if #responses == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local correct = 0
    for _, r in ipairs(responses) do
        correct = correct + r
    end
    return { kind = "scalar", value = correct / #responses }
end

function team_has(team, pid)
    for _, id in ipairs(team) do
        if id == pid then return true end
    end
    return false
end