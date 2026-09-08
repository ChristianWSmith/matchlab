-- plugins/adversarial/deranker.lua
-- Intentionally loses matches while rating is above target_rating (throws by
-- raising quit_probability and tilt_level).
-- config: target_rating

function tick(player_id, behavior, observation, config, context)
    local rating = observation and observation.rating or 0.0
    if rating > config.target_rating then
        behavior.quit_probability = 0.9
        behavior.tilt_level = 1.0
    end
    return behavior, context
end

function objective(config, context)
    return { kind = "MaintainLowRating" }
end