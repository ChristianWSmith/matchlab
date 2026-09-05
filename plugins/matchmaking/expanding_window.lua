-- plugins/matchmaking/expanding_window.lua
-- Expanding-window matchmaker: skills are matched within a window that widens
-- with queue wait, via stepped tiers.
-- config: tiers = { {max_secs, allowed_diff}, ... }, max_window

function find_matches(queue, team_size, now_secs, config, context)
    local tiers = config.tiers or {
        { 5.0, 25.0 },
        { 10.0, 50.0 },
        { 20.0, 100.0 },
        { 30.0, 200.0 },
    }
    local max_window = config.max_window or 400.0

    local function skill_window(wait)
        for _, tier in ipairs(tiers) do
            if wait <= tier[1] then
                return tier[2]
            end
        end
        return max_window
    end

    local ratings = {}
    for _, e in ipairs(queue) do
        ratings[e.player_id] = e.rating
    end

    local matches = {}
    local used = {}
    for _, entry in ipairs(queue) do
        if not used[entry.player_id] then
            local window = skill_window(entry.wait_secs)
            local team_a = { entry.player_id }
            local team_b = {}
            for _, other in ipairs(queue) do
                if not used[other.player_id] and other.player_id ~= entry.player_id then
                    local diff = math.abs(entry.rating - other.rating)
                    if diff <= window then
                        if #team_a <= #team_b then
                            table.insert(team_a, other.player_id)
                        else
                            table.insert(team_b, other.player_id)
                        end
                    end
                    if #team_a == team_size and #team_b == team_size then
                        break
                    end
                end
            end
            if #team_a == team_size and #team_b == team_size then
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