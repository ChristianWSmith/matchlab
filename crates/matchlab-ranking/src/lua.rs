//! Lua-native rank mapper.
//!
//! `LuaRankMapper` implements the `RankMapper` trait by delegating to a
//! script's `rating_to_rank` / `rank_to_rating_range` functions. The bracket
//! table lives in `config.brackets`.
use crate::ranker::{Rank, RankMapper};
use matchlab_lua::DataRequirements;
use matchlab_lua::vm::LuaVm;
use mlua::{Table, Value};
/// A rank mapper whose algorithm lives entirely in a Lua script.
pub struct LuaRankMapper {
    vm: LuaVm,
    data_requirements: DataRequirements,
}
impl LuaRankMapper {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load_with_plugin_dir(
            path,
            params,
            &["rating_to_rank", "rank_to_rating_range"],
            "plugins/ranking",
        )?;
        let data_requirements = vm.read_data_requirements()?;
        Ok(Self {
            vm,
            data_requirements,
        })
    }
    pub fn script_path(&self) -> &str {
        self.vm.script_path()
    }
}
fn rank_from_table(t: &Table) -> Rank {
    Rank {
        tier: t.get::<String>("tier").unwrap_or_default(),
        division: t.get::<u8>("division").unwrap_or(1),
    }
}
impl RankMapper for LuaRankMapper {
    fn rating_to_rank(&self, rating: f64) -> Rank {
        let data_val: Value = self
            .vm
            .with_lua(|lua| {
                let data = lua.create_table().map_err(|e| e.to_string())?;
                if self.data_requirements.has_request_field("rating") {
                    data.set("rating", rating).map_err(|e| e.to_string())?;
                }
                Ok(mlua::Value::Table(data))
            })
            .expect("build data");
        let rank_tbl: Table = self
            .vm
            .call_with_context("rating_to_rank", &[data_val])
            .expect("rating_to_rank failed");
        rank_from_table(&rank_tbl)
    }
    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64) {
        let data_val: Value = self
            .vm
            .with_lua(|lua| {
                let data = lua.create_table().map_err(|e| e.to_string())?;
                if self.data_requirements.has_request_field("rank") {
                    let t = lua.create_table().map_err(|e| e.to_string())?;
                    t.set("tier", rank.tier.as_str())
                        .map_err(|e| e.to_string())?;
                    t.set("division", rank.division)
                        .map_err(|e| e.to_string())?;
                    data.set("rank", t).map_err(|e| e.to_string())?;
                }
                Ok(mlua::Value::Table(data))
            })
            .expect("build data");
        let range: Table = self
            .vm
            .call_with_context("rank_to_rating_range", &[data_val])
            .expect("rank_to_rating_range failed");
        (
            range.get::<f64>("min").unwrap_or(0.0),
            range.get::<f64>("max").unwrap_or(0.0),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mapper() -> LuaRankMapper {
        LuaRankMapper::load(
            "plugins/ranking/brackets.lua",
            &serde_yaml::from_str(
                "brackets:\n  - { tier: bronze, division: 1, min: 0.0, max: 1200.0 }\n  - { tier: silver, division: 1, min: 1200.0, max: 2000.0 }",
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn rating_maps_to_bracket() {
        let m = mapper();
        assert_eq!(m.rating_to_rank(500.0).tier, "bronze");
        assert_eq!(m.rating_to_rank(1500.0).tier, "silver");
    }
    #[test]
    fn above_max_clamps_to_last() {
        let m = mapper();
        assert_eq!(m.rating_to_rank(5000.0).tier, "silver");
    }
    #[test]
    fn range_roundtrip() {
        let m = mapper();
        let rank = Rank {
            tier: "silver".into(),
            division: 1,
        };
        let (min, max) = m.rank_to_rating_range(&rank);
        assert_eq!((min, max), (1200.0, 2000.0));
    }
}
