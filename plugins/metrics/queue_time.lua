-- plugins/metrics/queue_time.lua
-- Actual queue wait per participant: formation time minus queue join time.

name = "queue_time"

data_requirements = {
    observation_fields = { "player_id", "queue_joined_at_ticks" },
    snapshot_fields = { "tick" },
}

function on_record(data, context)
    context.samples = context.samples or {}
    local snapshot = data.snapshot
    for _, p in ipairs(snapshot.players) do
        if p.queue_joined_at_ticks ~= nil then
            local wait = (snapshot.tick - p.queue_joined_at_ticks) / 1e9
            table.insert(context.samples, wait)
        end
    end
    return context
end

function compute(data, context)
    return { kind = "summary", values = context.samples or {} }
end