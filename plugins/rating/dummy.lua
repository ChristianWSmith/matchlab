-- plugins/rating/dummy.lua
-- A baseline rating system that assigns every player a fixed rating
-- and never updates. Used to verify that the simulation environment
-- produces stable ground truth — if MAE drifts with this system,
-- the skill dynamics are too aggressive for any rating system to track.
-- config: fixed_rating

data_requirements = {
    observation_fields = { "rating", "rating_deviation", "volatility", "games_played" },
    match_result_fields = { "winner", "team_a", "team_b" },
}

function initialize(player_id, config, context)
    return {
        rating = config.fixed_rating or 1000.0,
        rating_deviation = 350.0,
        volatility = 0.0,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    return 0.5
end

function update(match_result, observations, config, context)
    local updates = {}
    for _, id in ipairs(match_result.team_a) do
        local o = observations[id]
        if o then
            table.insert(updates, {
                player_id = id,
                rating = o.rating,
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    for _, id in ipairs(match_result.team_b) do
        local o = observations[id]
        if o then
            table.insert(updates, {
                player_id = id,
                rating = o.rating,
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    return updates, context
end
