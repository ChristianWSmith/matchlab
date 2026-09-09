-- plugins/rating/rank_centrality.lua
-- Rank Centrality: spectral inference over the match graph.
-- Maintains a transition matrix and computes stationary distribution.
-- Online approximation: power iteration on the running graph.
-- config: initial_rating, damping_factor, iterations

information_budget = { "WinLoss" }

function initialize(player_id, config, context)
    if not context.graph then
        context.graph = { nodes = {}, edges = {} }
    end
    context.graph.nodes[tostring(player_id)] = true
    return {
        rating = config.initial_rating,
        rating_deviation = 350.0,
        volatility = 0.06,
        games_played = 0,
    }, context
end

function predict(team_a, team_b, config, context)
    local avg_a = team_average(team_a)
    local avg_b = team_average(team_b)
    return 1.0 / (1.0 + 10.0 ^ ((avg_b - avg_a) / 400.0))
end

function team_average(team)
    local sum = 0.0
    for _, o in ipairs(team) do
        sum = sum + o.rating
    end
    if #team == 0 then return 0.0 end
    return sum / #team
end

function update(match_result, observations, config, context)
    local damping = config.damping_factor or 0.85
    local iters = config.iterations or 10
    local team_a_won = match_result.winner == "A"

    if not context.graph then
        context.graph = { nodes = {}, edges = {} }
    end
    local graph = context.graph

    local function ensure_node(id)
        local key = tostring(id)
        if not graph.nodes[key] then
            graph.nodes[key] = { wins = 0, losses = 0, opponents = {} }
        end
    end

    for _, id in ipairs(match_result.team_a) do ensure_node(id) end
    for _, id in ipairs(match_result.team_b) do ensure_node(id) end

    local function add_edge(winner_ids, loser_ids)
        for _, wid in ipairs(winner_ids) do
            for _, lid in ipairs(loser_ids) do
                local wk, lk = tostring(wid), tostring(lid)
                local w_node = graph.nodes[wk]
                local l_node = graph.nodes[lk]
                w_node.wins = w_node.wins + 1
                l_node.losses = l_node.losses + 1
                w_node.opponents[lk] = (w_node.opponents[lk] or 0) + 1
                l_node.opponents[wk] = (l_node.opponents[wk] or 0) + 1
            end
        end
    end

    if team_a_won then
        add_edge(match_result.team_a, match_result.team_b)
    else
        add_edge(match_result.team_b, match_result.team_a)
    end

    local node_ids = {}
    for id, _ in pairs(graph.nodes) do
        table.insert(node_ids, id)
    end
    local n = #node_ids
    if n == 0 then return {}, context end

    local id_to_idx = {}
    for i, id in ipairs(node_ids) do
        id_to_idx[id] = i
    end

    local pi = {}
    for i = 1, n do
        pi[i] = 1.0 / n
    end

    for _ = 1, iters do
        local new_pi = {}
        for i = 1, n do
            new_pi[i] = (1.0 - damping) / n
        end
        for i, id in ipairs(node_ids) do
            local node = graph.nodes[id]
            local total_games = node.wins + node.losses
            if total_games > 0 then
                for opp_id, opp_wins in pairs(node.opponents) do
                    local j = id_to_idx[opp_id]
                    if j then
                        local opp_node = graph.nodes[opp_id]
                        local opp_total = opp_node.wins + opp_node.losses
                        if opp_total > 0 then
                            local p_ij = opp_wins / (opp_wins + (graph.nodes[opp_id].opponents[id] or 0))
                            new_pi[i] = new_pi[i] + damping * pi[j] * p_ij / n
                        end
                    end
                end
            end
        end
        pi = new_pi
    end

    local updates = {}
    local min_pi, max_pi = math.huge, -math.huge
    for i = 1, n do
        if pi[i] < min_pi then min_pi = pi[i] end
        if pi[i] > max_pi then max_pi = pi[i] end
    end
    local range = max_pi - min_pi
    if range < 1e-10 then range = 1.0 end

    for _, id in ipairs(match_result.team_a) do
        local o = observations[id]
        if o then
            local idx = id_to_idx[tostring(id)]
            local normalized = idx and ((pi[idx] - min_pi) / range) or 0.5
            table.insert(updates, {
                player_id = id,
                rating = config.initial_rating + normalized * 1000.0,
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    for _, id in ipairs(match_result.team_b) do
        local o = observations[id]
        if o then
            local idx = id_to_idx[tostring(id)]
            local normalized = idx and ((pi[idx] - min_pi) / range) or 0.5
            table.insert(updates, {
                player_id = id,
                rating = config.initial_rating + normalized * 1000.0,
                rating_deviation = o.rating_deviation,
                volatility = o.volatility,
                games_played = o.games_played + 1,
            })
        end
    end
    return updates, context
end
