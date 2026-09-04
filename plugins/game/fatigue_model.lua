-- plugins/game/fatigue_model.lua
-- Session-length-based skill decay hook.
-- Players who have played many games see reduced effective skill.

function on_effective_skill(rating, rd, games_played)
    local decay = 1.0 - (games_played * 0.001)
    return rating * math.max(decay, 0.5)
end

function on_noise(duration, team_size)
    -- Longer matches have slightly more noise
    return 0.05 + duration / 10000.0
end
