-- plugins/matchmaking/hub_spoke.lua
-- Hub-and-spoke matchmaker: partition the queue by region; under-capacity
-- regions form matches regionally (greedy), overflow regions fall to the hub
-- path (longest-waiting first). No nested matchmakers — the regional greedy is
-- inlined.
-- config: spoke_capacity

function find_matches(queue, teams, now_secs, config, context)
    local size_a = teams.a.size
    local size_b = teams.b.size
    local capacity = config.spoke_capacity or 100

    local by_region = {}
    for _, e in ipairs(queue) do
        if not by_region[e.region] then
            by_region[e.region] = {}
        end
        table.insert(by_region[e.region], e)
    end

    local ratings = {}
    for _, e in ipairs(queue) do
        ratings[e.player_id] = e.rating
    end

    local matches = {}
    for region, entries in pairs(by_region) do
        if #entries <= capacity then
            -- Regional greedy (same logic as strict with no diff bound).
            local used = {}
            for _, entry in ipairs(entries) do
                if not used[entry.player_id] then
                    local team_a = { entry.player_id }
                    local team_b = {}
                    for _, other in ipairs(entries) do
                        if not used[other.player_id] and other.player_id ~= entry.player_id then
                            if #team_a < size_a and #team_a <= #team_b then
                                table.insert(team_a, other.player_id)
                            elseif #team_b < size_b then
                                table.insert(team_b, other.player_id)
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
        else
            -- Overflow: hub path, longest-waiting first, batch greedy.
            table.sort(entries, function(a, b)
                if a.joined_at_secs ~= b.joined_at_secs then
                    return a.joined_at_secs < b.joined_at_secs
                end
                return a.idx < b.idx
            end)
            local team_a, team_b = {}, {}
            local function emit()
                if #team_a == size_a and #team_b == size_b then
                    table.insert(matches, {
                        team_a = team_a,
                        team_b = team_b,
                        quality_score = match_quality(team_a, team_b, ratings),
                    })
                    team_a, team_b = {}, {}
                end
            end
            for _, e in ipairs(entries) do
                if #team_a < size_a then
                    table.insert(team_a, e.player_id)
                elseif #team_b < size_b then
                    table.insert(team_b, e.player_id)
                else
                    emit()
                    table.insert(team_a, e.player_id)
                end
            end
            emit()
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