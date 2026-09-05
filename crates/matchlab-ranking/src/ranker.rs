use serde::Deserialize;

pub trait RankMapper: Send + Sync {
    fn rating_to_rank(&self, rating: f64) -> Rank;
    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Rank {
    pub tier: String,
    pub division: u8,
}

pub struct BracketRankMapper {
    pub brackets: Vec<RankBracket>,
}

#[derive(Debug, Deserialize)]
pub struct RankBracket {
    pub rank: Rank,
    pub min: f64,
    pub max: f64,
}

impl BracketRankMapper {
    pub fn new(brackets: Vec<RankBracket>) -> Self {
        Self { brackets }
    }
}

impl RankMapper for BracketRankMapper {
    fn rating_to_rank(&self, rating: f64) -> Rank {
        for bracket in &self.brackets {
            if rating >= bracket.min && rating < bracket.max {
                return bracket.rank.clone();
            }
        }
        self.brackets.last().unwrap().rank.clone()
    }

    fn rank_to_rating_range(&self, rank: &Rank) -> (f64, f64) {
        for bracket in &self.brackets {
            if &bracket.rank == rank {
                return (bracket.min, bracket.max);
            }
        }
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapper() -> BracketRankMapper {
        BracketRankMapper::new(vec![
            RankBracket {
                rank: Rank { tier: "bronze".to_string(), division: 1 },
                min: 0.0,
                max: 800.0,
            },
            RankBracket {
                rank: Rank { tier: "silver".to_string(), division: 1 },
                min: 800.0,
                max: 1200.0,
            },
            RankBracket {
                rank: Rank { tier: "gold".to_string(), division: 1 },
                min: 1200.0,
                max: 1500.0,
            },
            RankBracket {
                rank: Rank { tier: "platinum".to_string(), division: 1 },
                min: 1500.0,
                max: 2000.0,
            },
        ])
    }

    #[test]
    fn rating_to_rank_maps_correctly() {
        let m = mapper();
        assert_eq!(
            m.rating_to_rank(500.0),
            Rank { tier: "bronze".to_string(), division: 1 }
        );
        assert_eq!(
            m.rating_to_rank(900.0),
            Rank { tier: "silver".to_string(), division: 1 }
        );
        assert_eq!(
            m.rating_to_rank(1300.0),
            Rank { tier: "gold".to_string(), division: 1 }
        );
        assert_eq!(
            m.rating_to_rank(1800.0),
            Rank { tier: "platinum".to_string(), division: 1 }
        );
    }

    #[test]
    fn rating_to_rank_clamps_to_last_bracket() {
        let m = mapper();
        assert_eq!(
            m.rating_to_rank(9999.0),
            Rank { tier: "platinum".to_string(), division: 1 }
        );
        assert_eq!(
            m.rating_to_rank(-100.0),
            Rank { tier: "platinum".to_string(), division: 1 }
        );
    }

    #[test]
    fn rank_to_rating_range_returns_bounds() {
        let m = mapper();
        assert_eq!(
            m.rank_to_rating_range(&Rank { tier: "bronze".to_string(), division: 1 }),
            (0.0, 800.0)
        );
        assert_eq!(
            m.rank_to_rating_range(&Rank { tier: "silver".to_string(), division: 1 }),
            (800.0, 1200.0)
        );
    }

    #[test]
    fn rank_to_rating_range_unknown_rank_returns_zero() {
        let m = mapper();
        assert_eq!(
            m.rank_to_rating_range(&Rank { tier: "radiant".to_string(), division: 1 }),
            (0.0, 0.0)
        );
    }

    #[test]
    fn rank_deserializes_from_yaml() {
        let yaml = serde_yaml::from_str::<Rank>("tier: gold\ndivision: 2\n").unwrap();
        assert_eq!(yaml.tier, "gold");
        assert_eq!(yaml.division, 2);
    }
}