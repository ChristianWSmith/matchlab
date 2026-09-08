-- plugins/metrics/streaks.lua
-- Probability of reaching 3/5/8/10-game win/loss streaks.

name = "streaks"

function on_record(match_result, snapshot, config, context)
    context.streaks = context.streaks or {}
    context.max_streaks = context.max_streaks or {}
    local winner_is_a = match_result.winner == "A"
    for _, p in ipairs(snapshot.players) do
        local won = (team_has(match_result.team_a, p.player_id) and winner_is_a)
            or (team_has(match_result.team_b, p.player_id) and not winner_is_a)
        local entry = context.streaks[p.player_id]
        if not entry then
            entry = { won, 0 }
            context.streaks[p.player_id] = entry
        end
        if (entry[1] and won) or (not entry[1] and not won) then
            entry[2] = entry[2] + 1
        else
            table.insert(context.max_streaks, entry[2])
            entry[1] = won
            entry[2] = 1
        end
    end
    return context
end

function compute(config, context)
    local max_streaks = context.max_streaks or {}
    local total = #max_streaks
    if total == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local function p_at(threshold)
        local count = 0
        for _, s in ipairs(max_streaks) do
            if s >= threshold then count = count + 1 end
        end
        return count / total
    end
    return { kind = "distribution", values = { p_at(3), p_at(5), p_at(8), p_at(10) } }
end

function team_has(team, pid)
    for _, id in ipairs(team) do
        if id == pid then return true end
    end
    return false
end