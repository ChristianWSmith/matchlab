-- plugins/rating/adaptive_glicko.lua
-- Adaptive rating bounds: wider bounds for new players, tighter for veterans.
-- Missing hooks fall back to Rust defaults; on_rating_bounds is consumed by
-- Glicko2RatingSystem to clamp post-update ratings.

function on_rating_bounds()
    return { floor = 100.0, ceiling = 3000.0 }
end

function on_k_factor(player_id, rating, games_played, recent_win_rate)
    -- Dynamic K proxy for systems that expose a K factor.
    if games_played < 10 then
        return 64.0
    end
    return 32.0
end