-- plugins/detection/smurf.lua
-- Smurf detector: a player is suspicious when their observed performance is
-- far above what their visible rating implies, for several consecutive games.
-- Per-player state lives in `context` (keyed by player id) so evidence
-- accumulates across matches. Never a boolean flag — inferred from behavior.
-- config: sigma_threshold (3.0), min_anomalous_games (5),
--         min_games_before_action (5), escalation_factor (0.9),
--         ladder = { {probability, action}, ... }

local DEFAULT_LADDER = {
    { 0.3, "None" },
    { 0.5, "AccelerateRating" },
    { 0.7, "FlagForReview" },
    { 0.8, "RestrictQueue" },
    { 0.9, "TempBan" },
    { 0.95, "Probation" },
    { 0.99, "Ban" },
}

function observe(match_result, observations, config, context)
    local sigma_threshold = config.sigma_threshold or 3.0
    for _, pid in ipairs(match_result.team_a) do
        record_player(pid, match_result, observations, context, sigma_threshold)
    end
    for _, pid in ipairs(match_result.team_b) do
        record_player(pid, match_result, observations, context, sigma_threshold)
    end
    return context
end

function record_player(pid, match_result, observations, context, sigma_threshold)
    local o = observations[pid]
    local perf = find_perf(match_result.performances, pid)
    if not o or not perf then
        return
    end

    local expected = o.rating / 100.0
    local actual = perf.impact + perf.kills / 10.0

    local key = tostring(pid)
    local state = context[key]
    if not state then
        state = { recent = {}, consecutive = 0, games = 0, interventions = 0 }
        context[key] = state
    end

    table.insert(state.recent, actual)
    if #state.recent > 20 then
        table.remove(state.recent, 1)
    end
    state.games = state.games + 1

    local dev = math.abs(actual - expected)
    local spread = 0.0
    for _, p in ipairs(state.recent) do
        spread = math.max(spread, math.abs(p - expected))
    end
    local sigmas = spread > 0.0 and dev / spread or 0.0

    if sigmas >= sigma_threshold then
        state.consecutive = state.consecutive + 1
    else
        state.consecutive = 0
    end
end

function find_perf(performances, pid)
    for _, p in ipairs(performances) do
        if p.player_id == pid then
            return p
        end
    end
    return nil
end

function evaluate(player_id, observations, config, context)
    local key = tostring(player_id)
    local state = context[key]
    if not state then
        return {
            player_id = player_id,
            probability_of_anomaly = 0.0,
            confidence = 0.0,
            evidence = {},
        }, context
    end

    local min_games = config.min_anomalous_games or 5
    local flagged = state.consecutive >= min_games
    local prob
    if flagged then
        local extra = state.consecutive - min_games
        prob = math.min(0.7 + 0.25 * math.min(extra, 1.2), 0.99)
    else
        prob = state.consecutive / min_games * 0.3
    end
    local confidence = math.min(state.consecutive / min_games, 1.0)

    return {
        player_id = player_id,
        probability_of_anomaly = prob,
        confidence = confidence,
        evidence = {
            "consecutive_anomalous=" .. state.consecutive,
            "min_required=" .. min_games,
        },
    }, context
end

function recommend_action(result, config, context)
    local key = tostring(result.player_id)
    local state = context[key]
    local games = state and state.games or 0
    local prior = state and state.interventions or 0
    local min_games_before_action = config.min_games_before_action or 5

    if games < min_games_before_action then
        return "None", context
    end

    local ladder = config.ladder or DEFAULT_LADDER
    local factor = config.escalation_factor or 0.9
    local prob = result.probability_of_anomaly
    local chosen = "None"

    for _, tier in ipairs(ladder) do
        local thresh = math.min(tier[1] * (factor ^ prior), tier[1])
        if prob >= thresh then
            chosen = tier[2]
        end
    end

    if state and chosen ~= "None" then
        state.interventions = state.interventions + 1
    end
    return chosen, context
end