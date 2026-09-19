-- plugins/metrics/convergence.lua
-- Games until |rating - true_skill| drops below the threshold (fewer is better).

name = "convergence"

data_requirements = {
    observation_fields = { "player_id", "rating", "games_played", "skill_overall" },
    reality_fields = { "true_skill" },
}

function on_record(data, context)
    local threshold = 50.0
    context.converged = context.converged or {}
    context.games = context.games or {}
    local snapshot = data.snapshot
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

function compute(data, context)
    local games = context.games or {}
    if #games == 0 then
        return { kind = "scalar", value = math.huge }
    end
    return { kind = "summary", values = games }
end