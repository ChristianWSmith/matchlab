-- plugins/utility/satisfaction.lua
-- Player satisfaction / retention model (spec §16.1).
-- config: match_quality, queue_time_penalty, win_bonus, loss_streak_penalty,
--         rank_progression_bonus, fairness_sensitivity, rematch_bonus

function satisfaction(experience, config, context)
    local mq = config.match_quality or 1.0
    local qtp = config.queue_time_penalty or -0.01
    local wb = config.win_bonus or 0.5
    local lsp = config.loss_streak_penalty or -0.3
    local rpb = config.rank_progression_bonus or 0.2
    local fs = config.fairness_sensitivity or -0.8
    local rb = config.rematch_bonus or 0.1

    local avg_quality = mean_or(experience.recent_match_qualities, 0.5)
    local avg_queue = mean_or(experience.recent_queue_times, 30.0)
    local outcomes = experience.recent_outcomes
    local wins = 0
    for _, won in ipairs(outcomes) do
        if won then wins = wins + 1 end
    end
    local win_rate = wins / math.max(#outcomes, 1)

    local streak_penalty = 0.0
    if experience.current_streak < -3 then
        streak_penalty = lsp * (math.abs(experience.current_streak) - 3.0)
    end

    return mq * avg_quality
        + qtp * avg_queue
        + wb * win_rate
        + streak_penalty
        + rpb * experience.rank_change
        + fs * (1.0 - experience.perceived_fairness)
        + rb * experience.rematch_rate
end

function retention_probability(satisfaction, config, context)
    return 1.0 / (1.0 + math.exp(-satisfaction))
end

function rematch_probability(satisfaction, config, context)
    return 1.0 / (1.0 + math.exp(-0.5 * (satisfaction - 2.0)))
end

function mean_or(values, default)
    if #values == 0 then return default end
    local sum = 0.0
    for _, v in ipairs(values) do sum = sum + v end
    return sum / #values
end