-- plugins/adversarial/win_trader.lua
-- Links the pair into a party and alternates wins to farm games while
-- maintaining rating.
-- config: partner, alternating

function tick(player_id, behavior, observation, config, context)
    local partner = config.partner
    if not partner then return behavior, context end
    local party = (player_id * 65537 + math.floor(partner)) % 2147483647
    behavior.party_id = party
    return behavior, context
end

function objective(config, context)
    return { kind = "WinTrade" }
end