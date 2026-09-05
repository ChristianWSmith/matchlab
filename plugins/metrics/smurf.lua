-- plugins/metrics/smurf.lua
-- Per-smurf damage: unfairness of matches containing a smurf. Smurfs are
-- identified by properties (high skill + few games), never a boolean flag.
-- Result fields follow the original collector's packed Summary layout.

name = "smurf"

function on_record(match_result, snapshot, config, context)
    context.events = context.events or {}
    context.smurf_ids = context.smurf_ids or {}
    local ratings = index_ratings(snapshot.players)
    for _, p in ipairs(snapshot.players) do
        local is_smurf = p.true_skill ~= nil
            and p.true_skill > 1300.0
            and (p.reality_games_played or 0) < 20
        if is_smurf then
            local avg_a = team_average(match_result.team_a, ratings)
            local avg_b = team_average(match_result.team_b, ratings)
            local prob = 1.0 / (1.0 + 10.0 ^ ((avg_b - avg_a) / 400.0))
            local unfairness = math.abs(prob - 0.5) * 2.0
            table.insert(context.events, {
                damage = unfairness,
                games = p.games_played,
            })
            local already = false
            for _, id in ipairs(context.smurf_ids) do
                if id == p.player_id then already = true break end
            end
            if not already then
                table.insert(context.smurf_ids, p.player_id)
            end
        end
    end
    return context
end

function compute(config, context)
    local events = context.events or {}
    local total = #events
    if total == 0 then
        return { kind = "scalar", value = 0.0 }
    end
    local sum_damage = 0.0
    local sum_games = 0.0
    for _, e in ipairs(events) do
        sum_damage = sum_damage + e.damage
        sum_games = sum_games + e.games
    end
    return {
        kind = "summary",
        mean = 0.0,
        median = 0.0,
        p75 = sum_damage / total,
        p90 = sum_games / total,
        p95 = #(context.smurf_ids or {}),
        p99 = 0.0,
        stddev = 0.0,
    }
end

function index_ratings(players)
    local ratings = {}
    for _, p in ipairs(players) do
        ratings[p.player_id] = p.rating
    end
    return ratings
end

function team_average(ids, ratings)
    local sum, n = 0.0, 0
    for _, id in ipairs(ids) do
        if ratings[id] ~= nil then
            sum = sum + ratings[id]
            n = n + 1
        end
    end
    if n == 0 then return 0.0 end
    return sum / n
end