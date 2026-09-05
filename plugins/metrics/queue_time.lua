-- plugins/metrics/queue_time.lua
-- Actual queue wait per participant: formation time minus queue join time.

name = "queue_time"

function on_record(match_result, snapshot, config, context)
    context.samples = context.samples or {}
    for _, p in ipairs(snapshot.players) do
        if p.queue_joined_at_ticks ~= nil then
            local wait = (snapshot.tick - p.queue_joined_at_ticks) / 1e9
            table.insert(context.samples, wait)
        end
    end
    return context
end

function compute(config, context)
    return { kind = "summary", values = context.samples or {} }
end