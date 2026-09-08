-- plugins/ranking/brackets.lua
-- Bracket rank mapper: first bracket where min <= rating < max; ratings outside
-- all brackets clamp to the last bracket. config.brackets = { {tier, division,
-- min, max}, ... }

function rating_to_rank(rating, config, context)
    local brackets = config.brackets
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

function rank_to_rating_range(rank, config, context)
    local brackets = config.brackets
    for _, b in ipairs(brackets) do
        if b.tier == rank.tier and b.division == rank.division then
            return { min = b.min, max = b.max }
        end
    end
    return { min = 0.0, max = 0.0 }
end