-- plugins/_test/spy_collector.lua
-- TEST-ONLY (never referenced by a manifest in experiments/): a metric
-- collector that errors loudly if its snapshot does NOT carry the ground-truth
-- skill fields — metrics are the legitimate reality reader, so the metric
-- snapshot must include true_skill/skill_overall/skill_vector. `compute`
-- returns the number of participants asserted; a non-zero result proves every
-- record carried reality data.

name = "spy_collector"

function on_record(match_result, snapshot, config, context)
    for _, p in ipairs(snapshot.players) do
        if p.true_skill == nil then
            error("spy_collector: snapshot missing true_skill")
        end
        if p.skill_overall == nil then
            error("spy_collector: snapshot missing skill_overall")
        end
        if p.skill_vector == nil then
            error("spy_collector: snapshot missing skill_vector")
        end
    end
    context.count = (context.count or 0) + #snapshot.players
    return context
end

function compute(config, context)
    return { kind = "scalar", value = context.count or 0 }
end