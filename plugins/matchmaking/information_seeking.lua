-- plugins/matchmaking/information_seeking.lua
-- Information-Seeking Matchmaker (ISM): evaluates candidate matches along two
-- dimensions — match quality and expected information value — and trades off
-- between them via a tunable parameter λ. Supports six information-value
-- strategies: rating discrimination, trajectory validation, boundary testing,
-- uncertainty prioritization, population validation, and upset tracking.
-- When teams.a.role / teams.b.role are set, each team is filled exclusively
-- from entries whose role matches that side's role; an entry matching neither waits.

data_requirements = {
    queue_fields = { "player_id", "rating", "rating_deviation", "wait_secs", "role", "idx" },
    completed_match_fields = { "id", "winner", "team_a", "team_b", "time" },
}

function find_closest_rating(target_rating, candidates, used, count, role)
    local pool = {}
    for _, e in ipairs(candidates) do
        if not used[e.player_id]
            and (role == nil or e.role == role)
        then
            table.insert(pool, {
                entry = e,
                dist = math.abs(e.rating - target_rating),
            })
        end
    end
    table.sort(pool, function(a, b)
        if a.dist ~= b.dist then
            return a.dist < b.dist
        end
        return a.entry.idx < b.entry.idx
    end)
    local result = {}
    for i = 1, math.min(count, #pool) do
        table.insert(result, pool[i].entry)
    end
    return result
end

function batch_fallback(queue, teams, context)
    local size_a = teams.a.size
    local size_b = teams.b.size

    local function match_quality(team_a_ids, team_b_ids, ratings_map)
        local sum_a, sum_b = 0, 0
        for _, pid in ipairs(team_a_ids) do
            sum_a = sum_a + (ratings_map[pid] or 0)
        end
        for _, pid in ipairs(team_b_ids) do
            sum_b = sum_b + (ratings_map[pid] or 0)
        end
        local avg_a = #team_a_ids > 0 and sum_a / #team_a_ids or 0
        local avg_b = #team_b_ids > 0 and sum_b / #team_b_ids or 0
        return 1.0 - math.min(math.abs(avg_a - avg_b) / 400.0, 1.0)
    end

    local ratings = {}
    for _, e in ipairs(queue) do
        ratings[e.player_id] = e.rating
    end

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

    local matches = {}
    local team_a, team_b = {}, {}
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

function find_matches(queue, teams, now_secs, config, context, completed)
    local size_a = teams.a.size
    local size_b = teams.b.size
    local role_a = teams.a.role
    local role_b = teams.b.role

    local info = config.information or {}
    local quality_cfg = config.quality or {}
    local constraints = config.constraints or {}

    local enabled = info.enabled
    if enabled == false then
        return batch_fallback(queue, teams, context)
    end

    local lambda = info.weight or 0.10
    local exploration_prob = info.exploration_probability or 0.05
    local max_rd = info.max_rating_deviation or 150
    local history_len = info.rating_history_length or 5

    local strategies = info.strategies or {}
    local s_rd = strategies.rating_discrimination or 1.0
    local s_traj = strategies.trajectory_validation or 0.5
    local s_boundary = strategies.boundary_testing or 0.5
    local s_uncert = strategies.uncertainty_prioritization or 0.0
    local s_pop = strategies.population_validation or 0.0
    local s_upset = strategies.upset_tracking or 0.0

    local upset_cfg = info.upset_tracking or {}
    local upset_decay_rate = upset_cfg.decay_rate or 0.95
    local upset_max_recent = upset_cfg.max_recent or 20
    local upset_normalization_rd = upset_cfg.normalization_rd or 350.0
    local upset_decay_interval = upset_cfg.decay_interval or 1000

    local quality_weight = quality_cfg.rating_weight or 1.0
    local max_skill_diff = constraints.max_skill_difference or 200
    local max_queue = constraints.max_queue_time or 60
    local repeat_penalty = constraints.repeat_opponent_penalty or 0.5

    if #queue < 2 then
        return {}, context
    end

    context.players = context.players or {}
    context.matches_formed = context.matches_formed or 0
    context.skill_band_counts = context.skill_band_counts or {}
    context.upsets = context.upsets or { matches_observed = 0, last_decay = 0 }

    local function process_completed(completed)
        if completed == nil then return end
        for _, m in ipairs(completed) do
            context.upsets.matches_observed = context.upsets.matches_observed + 1
            local winners, losers
            if m.winner == "a" then
                winners, losers = m.team_a, m.team_b
            else
                winners, losers = m.team_b, m.team_a
            end
            local sum_win, n_win = 0, 0
            for _, entry in ipairs(winners) do
                sum_win = sum_win + entry.rating
                n_win = n_win + 1
            end
            local sum_lose, n_lose = 0, 0
            for _, entry in ipairs(losers) do
                sum_lose = sum_lose + entry.rating
                n_lose = n_lose + 1
            end
            local avg_win = n_win > 0 and sum_win / n_win or 0
            local avg_lose = n_lose > 0 and sum_lose / n_lose or 0
            if avg_win < avg_lose then
                local gap = avg_lose - avg_win
                local avg_rd = 0
                local rd_count = 0
                for _, entry in ipairs(m.team_a) do
                    if entry.rd and entry.rd > 0 then
                        avg_rd = avg_rd + entry.rd
                        rd_count = rd_count + 1
                    end
                end
                for _, entry in ipairs(m.team_b) do
                    if entry.rd and entry.rd > 0 then
                        avg_rd = avg_rd + entry.rd
                        rd_count = rd_count + 1
                    end
                end
                if rd_count > 0 then
                    avg_rd = avg_rd / rd_count
                else
                    avg_rd = upset_normalization_rd
                end
                local normalized_gap = gap / math.max(avg_rd, 1.0)
                for _, entry in ipairs(winners) do
                    local pid = entry.id
                    local u = context.upsets[pid]
                    if u == nil then
                        u = { involved = 0, upsets = 0, total_magnitude = 0, recent = {} }
                        context.upsets[pid] = u
                    end
                    u.involved = u.involved + 1
                    u.upsets = u.upsets + 1
                    u.total_magnitude = u.total_magnitude + normalized_gap
                    table.insert(u.recent, { gap = normalized_gap, time = m.time })
                    while #u.recent > upset_max_recent do
                        table.remove(u.recent, 1)
                    end
                end
                for _, entry in ipairs(losers) do
                    local pid = entry.id
                    local u = context.upsets[pid]
                    if u == nil then
                        u = { involved = 0, upsets = 0, total_magnitude = 0, recent = {} }
                        context.upsets[pid] = u
                    end
                    u.involved = u.involved + 1
                    u.total_magnitude = u.total_magnitude + normalized_gap
                    table.insert(u.recent, { gap = normalized_gap, time = m.time })
                    while #u.recent > upset_max_recent do
                        table.remove(u.recent, 1)
                    end
                end
            else
                for _, entry in ipairs(m.team_a) do
                    local pid = entry.id
                    local u = context.upsets[pid]
                    if u == nil then
                        u = { involved = 0, upsets = 0, total_magnitude = 0, recent = {} }
                        context.upsets[pid] = u
                    end
                    u.involved = u.involved + 1
                end
                for _, entry in ipairs(m.team_b) do
                    local pid = entry.id
                    local u = context.upsets[pid]
                    if u == nil then
                        u = { involved = 0, upsets = 0, total_magnitude = 0, recent = {} }
                        context.upsets[pid] = u
                    end
                    u.involved = u.involved + 1
                end
            end
        end
    end

    local function apply_decay()
        local observed = context.upsets.matches_observed or 0
        local last = context.upsets.last_decay or 0
        if observed - last >= upset_decay_interval then
            for _, u in pairs(context.upsets) do
                if type(u) == "table" and u.total_magnitude then
                    u.total_magnitude = u.total_magnitude * upset_decay_rate
                end
            end
            context.upsets.last_decay = observed
        end
    end

    process_completed(completed)
    apply_decay()

    local function matches_role(entry, role)
        return role == nil or entry.role == role
    end

    local function average_rating(ids, ratings_map)
        local sum = 0.0
        for _, pid in ipairs(ids) do
            sum = sum + (ratings_map[pid] or 0.0)
        end
        if #ids == 0 then return 0.0 end
        return sum / #ids
    end

    local function quality_score(team_a_ids, team_b_ids, ratings_map)
        local diff = math.abs(
            average_rating(team_a_ids, ratings_map)
            - average_rating(team_b_ids, ratings_map)
        )
        return 1.0 - math.min(diff / 400.0, 1.0)
    end

    local function clamp(v, lo, hi)
        if v < lo then return lo end
        if v > hi then return hi end
        return v
    end

    local function rating_discrimination_value(seed_r, seed_u, opp_r)
        if seed_u <= 0.001 then return 0.0 end
        local diff = math.abs(seed_r - opp_r)
        local z = (diff - seed_u) / seed_u
        return math.exp(-0.5 * z * z)
    end

    local function linear_slope(values)
        local n = #values
        if n < 2 then return 0.0 end
        local sum_x, sum_y, sum_xy, sum_x2 = 0, 0, 0, 0
        for i, v in ipairs(values) do
            sum_x = sum_x + i
            sum_y = sum_y + v
            sum_xy = sum_xy + i * v
            sum_x2 = sum_x2 + i * i
        end
        local denom = n * sum_x2 - sum_x * sum_x
        if math.abs(denom) < 1e-12 then return 0.0 end
        return (n * sum_xy - sum_x * sum_y) / denom
    end

    local function trajectory_validation_value(history, opp_r)
        if #history < 2 then return 0.5 end
        local slope = linear_slope(history)
        if math.abs(slope) < 0.5 then return 0.5 end
        if slope > 0 then
            return opp_r > history[#history] and 1.0 or 0.2
        else
            return opp_r < history[#history] and 1.0 or 0.2
        end
    end

    local function boundary_testing_value(seed_r, seed_u, opp_r)
        if seed_u <= 0.001 then return 0.0 end
        local ratio = math.abs(seed_r - opp_r) / seed_u
        if ratio >= 0.8 and ratio <= 1.5 then return 1.0 end
        local dist = ratio < 0.8 and (0.8 - ratio) or (ratio - 1.5)
        return math.max(0.0, 1.0 - dist)
    end

    local function uncertainty_prioritization_value(seed_u, max_u)
        if max_u <= 0.001 then return 0.0 end
        return seed_u / max_u
    end

    local function skill_band(rating)
        return math.floor(rating / 100)
    end

    local function population_validation_value(band)
        local count = context.skill_band_counts[band] or 0
        return 1.0 / (1.0 + count)
    end

    local function upset_rate(pid)
        local u = context.upsets[pid]
        if not u or u.involved == 0 then return 0.5 end
        return u.upsets / u.involved
    end

    local function upset_uncertainty_value(seed_id, opp_id)
        local seed_u = upset_rate(seed_id)
        local opp_u = upset_rate(opp_id)
        return math.max(seed_u, opp_u)
    end

    local function info_value(seed, opp)
        local seed_u = clamp(seed.rating_deviation or 350.0, 0.1, max_rd)
        local seed_r = seed.rating
        local opp_r = opp.rating
        local total = 0.0

        if s_rd > 0 then
            total = total + s_rd * rating_discrimination_value(seed_r, seed_u, opp_r)
        end
        if s_traj > 0 then
            local hist = context.players[seed.player_id]
            local history = hist and hist.rating_history or {}
            total = total + s_traj * trajectory_validation_value(history, opp_r)
        end
        if s_boundary > 0 then
            total = total + s_boundary * boundary_testing_value(seed_r, seed_u, opp_r)
        end
        if s_uncert > 0 then
            total = total + s_uncert * uncertainty_prioritization_value(seed_u, max_rd)
        end
        if s_pop > 0 then
            local band = skill_band(seed_r)
            total = total + s_pop * population_validation_value(band)
        end
        if s_upset > 0 then
            total = total + s_upset * upset_uncertainty_value(seed.player_id, opp.player_id)
        end
        return total
    end

    local function matches_role_filter(entry, role)
        return role == nil or entry.role == role
    end

    local function entry_usable(entry)
        if entry.wait_secs > max_queue then return false end
        return matches_role_filter(entry, role_a) or matches_role_filter(entry, role_b)
    end

    local candidates = {}
    for _, e in ipairs(queue) do
        if entry_usable(e) then
            table.insert(candidates, e)
        end
    end
    table.sort(candidates, function(a, b)
        return a.wait_secs > b.wait_secs
    end)

    if #candidates < 2 then
        return {}, context
    end

    local ratings = {}
    for _, e in ipairs(candidates) do
        ratings[e.player_id] = e.rating
    end

    local used = {}
    local matches = {}
    local any_info = s_rd + s_traj + s_boundary + s_uncert + s_pop + s_upset > 0.001

    local function record_match(team_a_ids, team_b_ids)
        for _, pid in ipairs(team_a_ids) do
            local p = context.players[pid]
            if p == nil then
                p = { rating_history = {}, prev_opponents = {}, times_matched = 0 }
                context.players[pid] = p
            end
            for _, oid in ipairs(team_b_ids) do
                p.prev_opponents[oid] = true
            end
            p.times_matched = p.times_matched + 1
            local r = ratings[pid] or 0
            table.insert(p.rating_history, r)
            while #p.rating_history > history_len do
                table.remove(p.rating_history, 1)
            end
            local band = skill_band(r)
            context.skill_band_counts[band] = (context.skill_band_counts[band] or 0) + 1
        end
        for _, pid in ipairs(team_b_ids) do
            local p = context.players[pid]
            if p == nil then
                p = { rating_history = {}, prev_opponents = {}, times_matched = 0 }
                context.players[pid] = p
            end
            for _, oid in ipairs(team_a_ids) do
                p.prev_opponents[oid] = true
            end
            p.times_matched = p.times_matched + 1
            local r = ratings[pid] or 0
            table.insert(p.rating_history, r)
            while #p.rating_history > history_len do
                table.remove(p.rating_history, 1)
            end
            local band = skill_band(r)
            context.skill_band_counts[band] = (context.skill_band_counts[band] or 0) + 1
        end
    end

    for _, seed in ipairs(candidates) do
        if not used[seed.player_id] and size_a > 0 and size_b > 0 then
            local seed_u = clamp(seed.rating_deviation or 350.0, 0.1, max_rd)

            local effective_lambda = lambda
            if any_info and matchlab.rng_bool(exploration_prob) then
                effective_lambda = 1.0
            end

            local best_opp = nil
            local best_score = -1

            for _, cand in ipairs(candidates) do
                if not used[cand.player_id]
                    and cand.player_id ~= seed.player_id
                    and matches_role_filter(seed, role_a)
                    and matches_role_filter(cand, role_b)
                then
                    local diff = math.abs(seed.rating - cand.rating)
                    if diff <= max_skill_diff then
                        local q = 1.0 - math.min(diff / 400.0, 1.0)
                        local iv = info_value(seed, cand)

                        local p = context.players[seed.player_id]
                        if p and p.prev_opponents[cand.player_id] then
                            iv = iv * (1.0 - repeat_penalty)
                        end

                        local score = (1.0 - effective_lambda) * (quality_weight * q)
                            + effective_lambda * iv
                        if score > best_score then
                            best_score = score
                            best_opp = cand
                        end
                    end
                end
            end

            if best_opp ~= nil then
                used[seed.player_id] = true
                used[best_opp.player_id] = true

                local team_a, team_b
                if matches_role_filter(seed, role_a) then
                    team_a = { seed.player_id }
                    team_b = { best_opp.player_id }
                else
                    team_a = { best_opp.player_id }
                    team_b = { seed.player_id }
                end

                local remaining_a = size_a - #team_a
                if remaining_a > 0 then
                    local extras = find_closest_rating(
                        seed.rating, candidates, used, remaining_a, role_a
                    )
                    for _, e in ipairs(extras) do
                        table.insert(team_a, e.player_id)
                        used[e.player_id] = true
                    end
                end

                local remaining_b = size_b - #team_b
                if remaining_b > 0 then
                    local extras = find_closest_rating(
                        best_opp.rating, candidates, used, remaining_b, role_b
                    )
                    for _, e in ipairs(extras) do
                        table.insert(team_b, e.player_id)
                        used[e.player_id] = true
                    end
                end

                if #team_a == size_a and #team_b == size_b then
                    table.insert(matches, {
                        team_a = team_a,
                        team_b = team_b,
                        quality_score = quality_score(team_a, team_b, ratings),
                    })
                    context.matches_formed = context.matches_formed + 1
                    record_match(team_a, team_b)
                end
            end
        end
    end

    if repeat_penalty > 0 then
        for _, p in pairs(context.players) do
            if p.prev_opponents then
                local count = 0
                for _ in pairs(p.prev_opponents) do
                    count = count + 1
                end
                if count > history_len * 2 then
                    local trimmed = {}
                    local i = 0
                    for opp_id, _ in pairs(p.prev_opponents) do
                        i = i + 1
                        if i <= history_len then
                            trimmed[opp_id] = true
                        end
                    end
                    p.prev_opponents = trimmed
                end
            end
        end
    end

    return matches, context
end
