-- plugins/detection/smurf_thresholds.lua
-- Per-player sigma threshold hook.
-- New players get a higher threshold (more lenient), veterans get lower.

function on_anomaly_threshold(player_id, games_played)
    if games_played < 5 then
        return 5.0
    elseif games_played < 20 then
        return 3.5
    end
    return 2.5
end

function on_confidence(consecutive_anomalies, evidence_count)
    return math.min(consecutive_anomalies / 5.0, 1.0)
end
