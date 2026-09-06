-- plugins/matchmaking/batch.lua
-- Rating-balanced batch matchmaker: sort candidates by visible rating (ties by
-- join order) and assign alternately to team A / team B in consecutive
-- blocks of teams.a.size + teams.b.size. Adjacent-by-rating players land on
-- opposite teams, so the teams stay balanced and match quality stays high.
-- For equal sizes this is exactly the pre-XvY 2*team_size alternation; for
-- XvY a full team simply skips its turns until the other fills.
-- When teams.a.role / teams.b.role are set, team A is filled exclusively from
-- entries whose role matches teams.a.role and team B from entries matching
-- teams.b.role; an entry matching neither waits. Roles unset ⇒ the legacy
-- counts-only single-queue alternation (byte-identical regression path).

function find_matches(queue, teams, now_secs, config, context)
    local size_a = teams.a.size
    local size_b = teams.b.size
    local role_a = teams.a.role
    local role_b = teams.b.role

    if role_a == nil and role_b == nil then
        local candidates = {}
        for _, e in ipairs(queue) do
            table.insert(candidates, e)
        end
        table.sort(candidates, function(a, b)
            if a.rating ~= b.rating then
                return a.rating < b.rating
            end
            if a.joined_at_secs ~= b.joined_at_secs then
                return a.joined_at_secs < b.joined_at_secs
            end
            return a.idx < b.idx
        end)

        local ratings = {}
        for _, e in ipairs(candidates) do
            ratings[e.player_id] = e.rating
        end

        local matches = {}
        local team_a, team_b = {}, {}
        local alternate = false

        local function emit()
            if #team_a == size_a and #team_b == size_b then
                local quality = match_quality(team_a, team_b, ratings)
                table.insert(matches, {
                    team_a = team_a,
                    team_b = team_b,
                    quality_score = quality,
                })
                team_a, team_b = {}, {}
                alternate = false
            end
        end

        for _, e in ipairs(candidates) do
            if #team_a == size_a and #team_b == size_b then
                emit()
            end
            if alternate then
                if #team_b < size_b then
                    table.insert(team_b, e.player_id)
                else
                    table.insert(team_a, e.player_id)
                end
            else
                if #team_a < size_a then
                    table.insert(team_a, e.player_id)
                else
                    table.insert(team_b, e.player_id)
                end
            end
            alternate = not alternate
        end
        emit()

        return matches, context
    end

    -- Role-aware: split into per-side pools, sorting each by rating (ties by
    -- join order, then idx). Alternate consumption so each side's picks stay
    -- rating-ordered; never borrow across pools. When one pool runs short, the
    -- pending teams stall and those players wait (no partial proposals).
    local function matches_role(entry, role)
        return entry.role == role
    end

    local function by_rating(a, b)
        if a.rating ~= b.rating then
            return a.rating < b.rating
        end
        if a.joined_at_secs ~= b.joined_at_secs then
            return a.joined_at_secs < b.joined_at_secs
        end
        return a.idx < b.idx
    end

    local pool_a, pool_b = {}, {}
    for _, e in ipairs(queue) do
        if matches_role(e, role_a) then table.insert(pool_a, e) end
        if matches_role(e, role_b) then table.insert(pool_b, e) end
    end
    table.sort(pool_a, by_rating)
    table.sort(pool_b, by_rating)

    local ratings = {}
    for _, e in ipairs(queue) do
        ratings[e.player_id] = e.rating
    end

    local matches = {}
    local team_a, team_b = {}, {}
    local i_a, i_b = 1, 1
    local alternate = false

    local function emit()
        if #team_a == size_a and #team_b == size_b then
            table.insert(matches, {
                team_a = team_a,
                team_b = team_b,
                quality_score = match_quality(team_a, team_b, ratings),
            })
            team_a, team_b = {}, {}
            alternate = false
        end
    end

    while i_a <= #pool_a or i_b <= #pool_b do
        emit()
        if alternate then
            if #team_b < size_b and i_b <= #pool_b then
                table.insert(team_b, pool_b[i_b].player_id)
                i_b = i_b + 1
            elseif #team_a < size_a and i_a <= #pool_a then
                table.insert(team_a, pool_a[i_a].player_id)
                i_a = i_a + 1
            else
                break
            end
        else
            if #team_a < size_a and i_a <= #pool_a then
                table.insert(team_a, pool_a[i_a].player_id)
                i_a = i_a + 1
            elseif #team_b < size_b and i_b <= #pool_b then
                table.insert(team_b, pool_b[i_b].player_id)
                i_b = i_b + 1
            else
                break
            end
        end
        alternate = not alternate
    end
    emit()

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