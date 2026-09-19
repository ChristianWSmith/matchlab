-- plugins/ranking/brackets.lua
-- Bracket rank mapper: first bracket where min <= rating < max; ratings outside
-- all brackets clamp to the last bracket. config.brackets = { {tier, division,
-- min, max}, ... }

data_requirements = {
    request_fields = { "rating", "rank" },
}

function rating_to_rank(data, context)
    local rating = data.rating
    local config = _matchlab_config
    local brackets = config.brackets
    if not brackets then
        return { tier = "unranked", division = 1 }
    end
    local last = nil
    for _, b in ipairs(brackets) do
        last = b
        if rating >= b.min and rating < b.max then
            return { tier = b.tier, division = b.division }
        end
    end
    if last then
        return { tier = last.tier, division = last.division }
    end
    return { tier = "unranked", division = 1 }
end

function rank_to_rating_range(data, context)
    local rank = data.rank
    local config = _matchlab_config
    local brackets = config.brackets
    if not brackets then
        return { min = 0.0, max = 0.0 }
    end
    for _, b in ipairs(brackets) do
        if b.tier == rank.tier and b.division == rank.division then
            return { min = b.min, max = b.max }
        end
    end
    return { min = 0.0, max = 0.0 }
end
