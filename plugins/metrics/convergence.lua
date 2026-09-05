-- plugins/metrics/convergence.lua
-- Games until |rating - true_skill| drops below the threshold (fewer is better).

name = "convergence"

function on_record(match_result, snapshot, config, context)
    local threshold = config.threshold or 50.0
    context.converged = context.converged or {}
    context.games = context.games or {}
    for _, p in ipairs(snapshot.players) do
        if p.true_skill ~= nil and not context.converged[p.player_id] then
            local error = math.abs(p.rating - p.true_skill)
            if error < threshold then
                context.converged[p.player_id] = true
                table.insert(context.games, p.games_played)
            end
        end
    end
    return context
end

function compute(config, context)
    local games = context.games or {}
    if #games == 0 then
        return { kind = "scalar", value = math.huge }
    end
    return { kind = "summary", values = games }
end