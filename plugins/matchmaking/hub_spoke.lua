-- plugins/matchmaking/hub_spoke.lua
-- Hub-and-spoke matchmaker: partition the queue by region; under-capacity
-- regions form matches regionally (greedy), overflow regions fall to the hub
-- path (longest-waiting first). No nested matchmakers — the regional greedy is
-- inlined.
-- config: spoke_capacity
-- When teams.a.role / teams.b.role are set, each team is filled exclusively
-- from entries whose role matches that side's role (regional greedy or hub
-- overflow alike); an entry matching neither waits. Roles unset ⇒ the legacy
-- counts-only path, byte-identical.

function find_matches(queue, teams, now_secs, config, context)
    local size_a = teams.a.size
    local size_b = teams.b.size
    local role_a = teams.a.role
    local role_b = teams.b.role
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
    local by_wait = function(a, b)
        if a.joined_at_secs ~= b.joined_at_secs then
            return a.joined_at_secs < b.joined_at_secs
        end
        return a.idx < b.idx
    end

    if role_a == nil and role_b == nil then
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
                table.sort(entries, by_wait)
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

    -- Role-aware formation.
    local function matches_role(entry, role)
        return entry.role == role
    end

    for region, entries in pairs(by_region) do
        if #entries <= capacity then
            -- Regional greedy, filled per side's role pool.
            local used = {}
            for _, anchor in ipairs(entries) do
                local seed_a = matches_role(anchor, role_a)
                local seed_b = matches_role(anchor, role_b)
                if (seed_a or seed_b) and not used[anchor.player_id] then
                    local team_a = seed_a and { anchor.player_id } or {}
                    local team_b = not seed_a and seed_b and { anchor.player_id } or {}
                    for _, other in ipairs(entries) do
                        if not used[other.player_id] and other.player_id ~= anchor.player_id then
                            local can_a = matches_role(other, role_a) and #team_a < size_a
                                and (#team_a <= #team_b or not matches_role(other, role_b))
                            if can_a then
                                table.insert(team_a, other.player_id)
                            elseif matches_role(other, role_b) and #team_b < size_b then
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
            -- Overflow: hub path, longest-waiting first, per role.
            if role_a == role_b then
                -- Same role on both sides: one shared stream, A then B.
                local pool = {}
                for _, e in ipairs(entries) do
                    if matches_role(e, role_a) then table.insert(pool, e) end
                end
                table.sort(pool, by_wait)
                local team_a, team_b = {}, {}
                local i = 1
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
                while true do
                    if #team_a == size_a and #team_b == size_b then
                        emit()
                    end
                    if #team_a < size_a and i <= #pool then
                        table.insert(team_a, pool[i].player_id)
                        i = i + 1
                    elseif #team_b < size_b and i <= #pool then
                        table.insert(team_b, pool[i].player_id)
                        i = i + 1
                    else
                        emit()
                        break
                    end
                end
                emit()
            else
                -- Disjoint pools, A priority like the counts-only hub path.
                local pool_a, pool_b = {}, {}
                for _, e in ipairs(entries) do
                    if matches_role(e, role_a) then table.insert(pool_a, e) end
                    if matches_role(e, role_b) then table.insert(pool_b, e) end
                end
                table.sort(pool_a, by_wait)
                table.sort(pool_b, by_wait)
                local team_a, team_b = {}, {}
                local i_a, i_b = 1, 1
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
                while true do
                    if #team_a == size_a and #team_b == size_b then
                        emit()
                    end
                    if #team_a < size_a and i_a <= #pool_a then
                        table.insert(team_a, pool_a[i_a].player_id)
                        i_a = i_a + 1
                    elseif #team_b < size_b and i_b <= #pool_b then
                        table.insert(team_b, pool_b[i_b].player_id)
                        i_b = i_b + 1
                    else
                        emit()
                        break
                    end
                end
                emit()
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