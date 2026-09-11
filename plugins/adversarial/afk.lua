-- plugins/adversarial/afk.lua
-- Goes AFK/disconnects with probability go_afk_probability.
-- config: go_afk_probability

function tick(player_id, behavior, observation, config, context)
    local prob = config.go_afk_probability or 0.0
    if matchlab.rng_bool(prob) then
        behavior.quit_probability = 1.0
    end
    return behavior, context
end

function objective(config, context)
    return { kind = "MinimizeGamesPlayed" }
end