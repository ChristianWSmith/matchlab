-- plugins/matchmaking/strict.lua
-- Strict matchmaker: only matches players within a fixed skill difference, so
-- outliers may wait indefinitely (that is the intended strict behavior).
-- config: max_skill_diff

function find_matches(queue, teams, now_secs, config, context)
    local size_a = teams.a.size
    local size_b = teams.b.size
    local max_diff = config.max_skill_diff

    local ratings = {}
    for _, e in ipairs(queue) do
        ratings[e.player_id] = e.rating
    end

    local matches = {}
    local used = {}
    for _, entry in ipairs(queue) do
        if not used[entry.player_id] then
            local team_a = { entry.player_id }
            local team_b = {}
            for _, other in ipairs(queue) do
                if not used[other.player_id] and other.player_id ~= entry.player_id then
                    local diff = math.abs(entry.rating - other.rating)
                    if diff <= max_diff then
                        if #team_a < size_a and #team_a <= #team_b then
                            table.insert(team_a, other.player_id)
                        elseif #team_b < size_b then
                            table.insert(team_b, other.player_id)
                        end
                    end
                    if #team_a == size_a and #team_b == size_b then
                        break
                    end
                end
            end
            if #team_a == size_a and #team_b == size_b then
                for _, pid in ipairs(team_a) do used[pid] = true end
                for _, pid in ipairs(team_b) do used[pid] = true end
                table.insert(matches, {
                    team_a = team_a,
                    team_b = team_b,
                    quality_score = match_quality(team_a, team_b, ratings),
                })
            end
        end
    end

    return matches, context
end

function match_quality(team_a, team_b, ratings)
    local diff = math.abs(average_rating(team_a, ratings) - average_rating(team_b, ratings))
    return 1.0 - math.min(diff / 400.0, 1.0)
end

function average_rating(team, ratings)
    local sum = 0.0
    for _, pid in ipairs(team) do
        sum = sum + (ratings[pid] or 0.0)
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end