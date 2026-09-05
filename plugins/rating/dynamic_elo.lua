-- plugins/rating/dynamic_elo.lua
-- Dynamic K factor based on player experience.
-- New players (few games) get higher K for faster convergence.
-- Established players with high win rates get elevated K for responsiveness.
-- Veterans get lower K for stability.

function on_k_factor(player_id, rating, games_played, recent_win_rate)
    if games_played < 10 then
        return 64.0
    elseif recent_win_rate > 0.7 then
        return 48.0
    elseif games_played > 100 then
        return 16.0
    end
    return 32.0
end

-- Rating bounds prevent runaway inflation/deflation.
function on_rating_bounds()
    return { floor = 100.0, ceiling = 3000.0 }
end
