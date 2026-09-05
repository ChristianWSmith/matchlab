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
