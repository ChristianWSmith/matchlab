-- plugins/adversarial/booster.lua
-- Links the boosting duo into a party and boosts the boostee's win rate to 1.0.
-- config: boost_target, boostee

function tick(player_id, behavior, observation, config, context)
    local target = config.boost_target
    local boostee = config.boostee
    if not target or not boostee then return behavior, context end
    local party = (math.floor(target) * 65537 + math.floor(boostee)) % 2147483647
    behavior.party_id = party
    if player_id == boostee then
        behavior.win_rate = 1.0
    end
    return behavior, context
end

function objective(config, context)
    return { kind = "MaximizeRating" }
end