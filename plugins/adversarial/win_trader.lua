-- plugins/adversarial/win_trader.lua
-- Links the pair into a party and alternates wins to farm games while
-- maintaining rating.
-- config: partner, alternating

function tick(player_id, behavior, observation, config, context)
    local partner = config.partner
    local party = player_id ~ partner
    behavior.party_id = party
    return behavior, context
end

function objective(config, context)
    return { kind = "WinTrade" }
end