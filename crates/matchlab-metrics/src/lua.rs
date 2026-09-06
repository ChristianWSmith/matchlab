//! Lua-native metric collectors.
//!
//! `LuaMetricCollector` implements the `MetricCollector` trait by delegating to
//! a script's `on_record` / `compute` functions. The script declares its `name`
//! global and may declare a `time_buckets` function (for the `{name}_by_time`
//! series) and a `needs_population = true` global (to receive the full
//! population snapshot, not just match participants).

use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;
use matchlab_lua::convert;
use matchlab_lua::vm::LuaVm;
use mlua::{Function, Table, Value};

use crate::collector::{MetricCollector, MetricResult};
use crate::stats::summary_to_result;

/// A metric collector whose algorithm lives entirely in a Lua script.
pub struct LuaMetricCollector {
    vm: LuaVm,
    metric_name: String,
    needs_population: bool,
    /// Population snapshots are expensive to marshal; attach one every N
    /// matches (configurable via `sample_every`, default 50).
    sample_every: u64,
    match_count: u64,
}

impl LuaMetricCollector {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["on_record", "compute"])?;
        let metric_name = vm
            .get_global::<String>("name")?
            .ok_or_else(|| format!("metric script {} must declare `name`", vm.script_path()))?;
        let needs_population = vm.get_global::<bool>("needs_population")?.unwrap_or(false);
        let sample_every = vm
            .config()
            .get("sample_every")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .max(1);
        Ok(Self {
            vm,
            metric_name,
            needs_population,
            sample_every,
            match_count: 0,
        })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

fn parse_result(t: &Table) -> MetricResult {
    let kind: String = t.get("kind").unwrap_or_else(|_| "summary".to_string());
    match kind.as_str() {
        "scalar" => {
            let value = t.get::<f64>("value").unwrap_or(0.0);
            MetricResult::Scalar(value)
        }
        "distribution" => {
            let values = t.get::<Vec<f64>>("values").unwrap_or_default();
            MetricResult::Distribution(values)
        }
        "summary" => {
            if let Ok(values) = t.get::<Vec<f64>>("values") {
                summary_to_result(&values)
            } else {
                MetricResult::Summary {
                    mean: t.get::<f64>("mean").unwrap_or(0.0),
                    median: t.get::<f64>("median").unwrap_or(0.0),
                    p75: t.get::<f64>("p75").unwrap_or(0.0),
                    p90: t.get::<f64>("p90").unwrap_or(0.0),
                    p95: t.get::<f64>("p95").unwrap_or(0.0),
                    p99: t.get::<f64>("p99").unwrap_or(0.0),
                    stddev: t.get::<f64>("stddev").unwrap_or(0.0),
                }
            }
        }
        _other => MetricResult::Scalar(f64::NAN), // unsupported kind; surface as NaN
    }
}

impl MetricCollector for LuaMetricCollector {
    fn name(&self) -> &str {
        &self.metric_name
    }

    fn record_match(&mut self, match_result: &MatchResult, world: &World) {
        self.match_count += 1;
        let sample_population = self.needs_population
            && (self.match_count % self.sample_every == 0 || self.match_count == 1);
        let (mr_val, snapshot) = self
            .vm
            .with_lua(|lua| {
                let mr_val =
                    convert::match_result_to_table(lua, match_result).map(mlua::Value::Table)?;
                let snap = convert::metric_snapshot(lua, match_result, world)?;
                if sample_population {
                    let population = convert::population_snapshot(lua, world)?;
                    snap.as_table()
                        .expect("metric snapshot is a table")
                        .set("population", population)
                        .map_err(|e| e.to_string())?;
                }
                Ok((mr_val, snap))
            })
            .expect("build metric snapshot");
        let _: Value = self
            .vm
            .call_with_context("on_record", &[mr_val, snapshot])
            .expect("metric on_record failed");
    }

    fn compute(&self) -> MetricResult {
        let result_tbl: Table = self
            .vm
            .call_with_context("compute", &[])
            .expect("metric compute failed");
        parse_result(&result_tbl)
    }

    fn time_buckets(&self) -> Option<Vec<f64>> {
        let present = self
            .vm
            .get_global::<Function>("time_buckets")
            .expect("read time_buckets global");
        present.as_ref()?;
        self.vm
            .call_with_context("time_buckets", &[])
            .expect("metric time_buckets failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{PlayerId, PlayerReality, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> matchlab_core::player::PlayerObservation {
        matchlab_core::player::PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "gold".into(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: Some(SimTime::from_secs(1.0)),
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
            role: None,
        }
    }

    fn reality(id: u64, skill: f64) -> PlayerReality {
        PlayerReality {
            id: PlayerId(id),
            skill: SkillVector::one_dimensional(skill),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: matchlab_core::player::Region::NA,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".into(),
            role: None,
        }
    }

    fn mr() -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: vec![PlayerId(1), PlayerId(2)],
            team_b: vec![PlayerId(3), PlayerId(4)],
            team_a_score: 13.0,
            team_b_score: 5.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.1,
            unexpected_events: Vec::new(),
        }
    }

    fn load(name: &str) -> LuaMetricCollector {
        LuaMetricCollector::load(
            &format!("plugins/metrics/{name}.lua"),
            &serde_yaml::Value::Null,
        )
        .unwrap()
    }

    #[test]
    fn match_quality_summary_for_equal_teams() {
        let mut world = World::new(SimRng::from_seed(1));
        for id in 1..=4u64 {
            world.add_player(reality(id, 1000.0), obs(id, 1000.0));
        }
        let mut c = load("match_quality");
        assert_eq!(c.name(), "match_quality");
        c.record_match(&mr(), &world);
        c.record_match(&mr(), &world);
        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 1.0).abs() < 1e-9),
            other => panic!("expected summary, got {other:?}"),
        }
    }

    #[test]
    fn queue_time_measures_join_to_formation() {
        let mut world = World::new(SimRng::from_seed(1));
        for id in 1..=4u64 {
            world.add_player(reality(id, 1000.0), obs(id, 1000.0));
        }
        world.time = SimTime::from_secs(31.0);
        let mut c = load("queue_time");
        c.record_match(&mr(), &world);
        match c.compute() {
            MetricResult::Summary { mean, .. } => {
                assert!((mean - 30.0).abs() < 1e-6, "mean={mean}")
            }
            other => panic!("expected summary, got {other:?}"),
        }
    }

    #[test]
    fn rating_accuracy_emits_time_buckets() {
        let mut world = World::new(SimRng::from_seed(1));
        for id in 1..=4u64 {
            world.add_player(reality(id, 1500.0), obs(id, 1000.0));
        }
        world.time = SimTime::from_secs(300.0);
        let mut c = load("rating_accuracy");
        c.record_match(&mr(), &world);
        match c.compute() {
            MetricResult::Summary { mean, .. } => assert!((mean - 500.0).abs() < 1e-6),
            other => panic!("expected summary, got {other:?}"),
        }
        let buckets = c.time_buckets().expect("rating_accuracy has time buckets");
        assert_eq!(buckets.len(), 20);
    }
}
