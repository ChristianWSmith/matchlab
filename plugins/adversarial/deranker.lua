-- plugins/adversarial/deranker.lua
-- Intentionally loses matches while rating is above target_rating (throws by
-- raising quit_probability and tilt_level).
-- config: target_rating

data_requirements = {
    behavior_fields = { "quit_probability", "tilt_level" },
    observation_fields = { "player_id", "rating" },
}

function tick(data, context)
    local behavior = data.behavior
    local observation = data.observation
    local config = _matchlab_config
    local rating = observation and observation.rating or 0.0
    local target = config.target_rating or 500.0
    if rating > target then
        behavior.quit_probability = 0.9
        behavior.tilt_level = 1.0
    end
    return behavior, context
end

function objective(context)
    return { kind = "MaintainLowRating" }
end
