-- plugins/adversarial/rating_farmer.lua
-- Queues then quits/disconnects: keeps games_played minimal (smurf-like
-- account after a reset).
-- config: quit_probability, quit_after_minutes

data_requirements = {
    behavior_fields = { "quit_probability", "is_online" },
}

function tick(data, context)
    local behavior = data.behavior
    local config = _matchlab_config
    local prob = config.quit_probability or 0.0
    if matchlab.rng_bool(prob) then
        behavior.quit_probability = 1.0
        behavior.is_online = false
    end
    return behavior, context
end

function objective(context)
    return { kind = "MaximizeWinRate" }
end
