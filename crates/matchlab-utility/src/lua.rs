//! Lua-native satisfaction model.
//!
//! `LuaSatisfactionModel` implements the `SatisfactionModel` trait by
//! delegating to a script's `satisfaction` / `retention_probability` /
//! `rematch_probability` functions. `PlayerExperience` (loop-maintained data)
//! stays in Rust; the weights live in the script's config.

use matchlab_lua::vm::LuaVm;
use mlua::{Table, Value};

use crate::satisfaction::{PlayerExperience, SatisfactionModel};

/// A satisfaction model whose algorithm lives entirely in a Lua script.
pub struct LuaSatisfactionModel {
    vm: LuaVm,
}

impl LuaSatisfactionModel {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(
            path,
            params,
            &[
                "satisfaction",
                "retention_probability",
                "rematch_probability",
            ],
        )?;
        Ok(Self { vm })
    }

    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}

fn experience_to_table(exp: &PlayerExperience, lua: &mlua::Lua) -> Result<Table, String> {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let qualities = lua.create_table().map_err(|e| e.to_string())?;
    for (i, v) in exp.recent_match_qualities.iter().enumerate() {
        qualities.set(i + 1, *v).map_err(|e| e.to_string())?;
    }
    t.set("recent_match_qualities", qualities)
        .map_err(|e| e.to_string())?;
    let queues = lua.create_table().map_err(|e| e.to_string())?;
    for (i, v) in exp.recent_queue_times.iter().enumerate() {
        queues.set(i + 1, *v).map_err(|e| e.to_string())?;
    }
    t.set("recent_queue_times", queues)
        .map_err(|e| e.to_string())?;
    let outcomes = lua.create_table().map_err(|e| e.to_string())?;
    for (i, v) in exp.recent_outcomes.iter().enumerate() {
        outcomes.set(i + 1, *v).map_err(|e| e.to_string())?;
    }
    t.set("recent_outcomes", outcomes)
        .map_err(|e| e.to_string())?;
    t.set("current_streak", exp.current_streak)
        .map_err(|e| e.to_string())?;
    t.set("rank_change", exp.rank_change)
        .map_err(|e| e.to_string())?;
    t.set("perceived_fairness", exp.perceived_fairness)
        .map_err(|e| e.to_string())?;
    t.set("rematch_rate", exp.rematch_rate)
        .map_err(|e| e.to_string())?;
    Ok(t)
}

impl SatisfactionModel for LuaSatisfactionModel {
    fn satisfaction(&self, exp: &PlayerExperience) -> f64 {
        let exp_val = self
            .vm
            .with_lua(|lua| experience_to_table(exp, lua).map(Value::Table))
            .expect("build experience table");
        self.vm
            .call_with_context("satisfaction", &[exp_val])
            .expect("satisfaction failed")
    }

    fn retention_probability(&self, satisfaction: f64) -> f64 {
        self.vm
            .call_with_context("retention_probability", &[Value::Number(satisfaction)])
            .expect("retention_probability failed")
    }

    fn rematch_probability(&self, satisfaction: f64) -> f64 {
        self.vm
            .call_with_context("rematch_probability", &[Value::Number(satisfaction)])
            .expect("rematch_probability failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> LuaSatisfactionModel {
        LuaSatisfactionModel::load("plugins/utility/satisfaction.lua", &serde_yaml::Value::Null)
            .unwrap()
    }

    #[test]
    fn default_satisfaction_uses_defaults() {
        let m = model();
        let exp = PlayerExperience::new();
        // avg_quality 0.5, avg_queue 30.0, win_rate 0.0, streak 0, fairness 0.5
        let expected = 1.0 * 0.5 + (-0.01) * 30.0 + 0.5 * 0.0 + 0.0 + (-0.8) * 0.5;
        let score = m.satisfaction(&exp);
        assert!((score - expected).abs() < 1e-9, "score={score}");
    }

    #[test]
    fn high_quality_and_wins_increase_score() {
        let m = model();
        let mut exp = PlayerExperience::new();
        exp.recent_match_qualities = vec![1.0, 1.0];
        exp.recent_outcomes = vec![true, true];
        let baseline = m.satisfaction(&PlayerExperience::new());
        let good = m.satisfaction(&exp);
        assert!(good > baseline);
    }

    #[test]
    fn loss_streak_penalty_applies_below_minus_three() {
        let m = model();
        let mut exp = PlayerExperience::new();
        exp.current_streak = -5;
        let score = m.satisfaction(&exp);
        let expected = 1.0 * 0.5 + (-0.01) * 30.0 + 0.5 * 0.0 + (-0.3) * 2.0 + (-0.8) * 0.5;
        assert!((score - expected).abs() < 1e-9, "score={score}");
    }

    #[test]
    fn retention_and_rematch_logistics() {
        let m = model();
        assert!((m.retention_probability(0.0) - 0.5).abs() < 1e-9);
        assert!((m.rematch_probability(2.0) - 0.5).abs() < 1e-9);
        for s in [0.0, 1.0, 2.0, 4.0, 6.0] {
            assert!(
                m.rematch_probability(s) < m.retention_probability(s),
                "rematch < retention at s={s}"
            );
        }
    }
}
