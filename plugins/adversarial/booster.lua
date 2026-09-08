-- plugins/adversarial/booster.lua
-- Links the boosting duo into a party and boosts the boostee's win rate to 1.0.
-- config: boost_target, boostee

function tick(player_id, behavior, observation, config, context)
    local party = config.boost_target ~ config.boostee
    behavior.party_id = party
    if player_id == config.boostee then
        behavior.win_rate = 1.0
    end
    return behavior, context
end

function objective(config, context)
    return { kind = "MaximizeRating" }
end