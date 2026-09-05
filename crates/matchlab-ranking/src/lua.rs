//! Lua-native rank mapper.
//!
//! `LuaRankMapper` implements the `RankMapper` trait by delegating to a
//! script's `rating_to_rank` / `rank_to_rating_range` functions. The bracket
//! table lives in `config.brackets`.

use matchlab_lua::vm::LuaVm;
use mlua::{Table, Value};

use crate::ranker::{Rank, RankMapper};

/// A rank mapper whose algorithm lives entirely in a Lua script.
pub struct LuaRankMapper {
    vm: LuaVm,
}

impl LuaRankMapper {
    pub fn load(path: &str, params: &serde_yaml::Value) -> Result<Self, String> {
        let vm = LuaVm::load(path, params, &["rating_to_rank", "rank_to_rating_range"])?;
        Ok(Self { vm })
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
        let rank_tbl: Table = self
            .vm
            .call_with_context("rating_to_rank", &[Value::Number(rating)])
            .expect("rating_to_rank failed");
        rank_from_table(&rank_tbl)
    }

    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64) {
        let rank_val = self
            .vm
            .with_lua(|lua| {
                let t = lua.create_table().map_err(|e| e.to_string())?;
                t.set("tier", rank.tier.as_str())
                    .map_err(|e| e.to_string())?;
                t.set("division", rank.division)
                    .map_err(|e| e.to_string())?;
                Ok(Value::Table(t))
            })
            .expect("build rank table");
        let range: Table = self
            .vm
            .call_with_context("rank_to_rating_range", &[rank_val])
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
