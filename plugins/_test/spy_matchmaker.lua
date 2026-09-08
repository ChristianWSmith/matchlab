-- plugins/_test/spy_matchmaker.lua
-- TEST-ONLY (never referenced by a manifest in experiments/): a matchmaker
-- whose `find_matches` errors loudly if any queue entry it receives carries
-- ground-truth skill data (skill_vector/skill_overall/hidden_mmr/true_skill).
-- Queue snapshots must be observations-only; a passing loop run proves the
-- convert layer leaks nothing to matchmaking scripts. It forms the simplest
-- possible teams (first `size_a` entries to A, next `size_b` to B).

function find_matches(queue, teams, now_secs, config, context)
    local forbidden = {
        skill_vector = true,
        skill_overall = true,
        hidden_mmr = true,
        true_skill = true,
    }
    for _, e in ipairs(queue) do
        for key in pairs(forbidden) do
            if e[key] ~= nil then
                error("budget leak: queue entry carries " .. key)
            end
        end
    end

    local size_a = teams.a.size
    local size_b = teams.b.size
    local matches = {}
    local team_a, team_b = {}, {}
    for i = 1, math.min(size_a, #queue) do
        table.insert(team_a, queue[i].player_id)
    end
    for i = size_a + 1, math.min(size_a + size_b, #queue) do
        table.insert(team_b, queue[i].player_id)
    end
    if #team_a == size_a and #team_b == size_b then
        table.insert(matches, { team_a = team_a, team_b = team_b })
    end
    return matches, context
end