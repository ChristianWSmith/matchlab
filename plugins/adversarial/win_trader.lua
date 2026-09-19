-- plugins/adversarial/win_trader.lua
-- Links the pair into a party and alternates wins to farm games while
-- maintaining rating.
-- config: partner, alternating

data_requirements = {
    request_fields = { "player_id" },
    behavior_fields = { "party_id" },
}

function tick(data, context)
    local behavior = data.behavior
    local player_id = data.player_id
    local config = _matchlab_config
    local partner = config.partner
    if not partner then return behavior, context end
    local party = (player_id * 65537 + math.floor(partner)) % 2147483647
    behavior.party_id = party
    return behavior, context
end

function objective(context)
    return { kind = "WinTrade" }
end
