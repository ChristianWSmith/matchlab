-- plugins/matchmaking/random.lua
-- Uniformly-random matchmaker: forms 2*team_size-sized matches by drawing
-- players at random from the queue with no rating balancing — the third
-- policy in the rating x matchmaking feedback-loop comparison. Deterministic
-- given the seed (draws flow through matchlab.rng_range).

function find_matches(queue, team_size, now_secs, config, context)
    local pool = {}
    for _, e in ipairs(queue) do
        table.insert(pool, e)
    end

    local ratings = {}
    for _, e in ipairs(queue) do
        ratings[e.player_id] = e.rating
    end

    local needed = 2 * team_size
    local matches = {}
    while #pool >= needed do
        local team_a, team_b = {}, {}
        for i = 1, needed do
            local idx = math.floor(matchlab.rng_range(1.0, #pool + 1.0))
            if idx > #pool then idx = #pool end
            local e = table.remove(pool, idx)
            if i <= team_size then
                table.insert(team_a, e.player_id)
            else
                table.insert(team_b, e.player_id)
            end
        end
        table.insert(matches, {
            team_a = team_a,
            team_b = team_b,
            quality_score = match_quality(team_a, team_b, ratings),
        })
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