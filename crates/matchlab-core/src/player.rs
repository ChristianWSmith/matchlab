use crate::time::SimTime;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    NA,
    EU,
    Asia,
    Other,
}

/// Multidimensional skill represented as a named-dimension map.
///
/// v0.1 uses a single `overall` dimension, but the type supports N dimensions so
/// later work can ask whether a 1D rating represents multidimensional skill.
#[derive(Debug, Clone)]
pub struct SkillVector {
    /// Map of skill dimension name → value.
    /// For 1D: {"overall": 1200.0}
    /// For multidimensional: {"aim": 1500, "movement": 1100, ...}
    pub dimensions: HashMap<String, f64>,
}

impl SkillVector {
    pub fn one_dimensional(value: f64) -> Self {
        let mut dimensions = HashMap::new();
        dimensions.insert("overall".to_string(), value);
        Self { dimensions }
    }

    pub fn overall(&self) -> f64 {
        if self.dimensions.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.dimensions.values().sum();
        sum / self.dimensions.len() as f64
    }

    pub fn weighted_overall(&self, weights: &HashMap<String, f64>) -> f64 {
        if self.dimensions.is_empty() {
            return 0.0;
        }
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        for (dim, &val) in &self.dimensions {
            let w = weights.get(dim).unwrap_or(&1.0);
            weighted_sum += val * w;
            weight_sum += w;
        }
        weighted_sum / weight_sum
    }
}

/// Lightweight visible rank, kept inside `player.rs` so core stays free of a
/// ranking dependency (spec §5.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VisibleRank {
    pub tier: String,
    pub division: u8,
}

impl VisibleRank {
    /// Approximate numeric midpoint of the visible rank bracket, for comparing
    /// the communicated rank to true skill.
    pub fn midpoint(&self) -> f64 {
        let tier_base: f64 = match self.tier.as_str() {
            "iron" => 300.0,
            "bronze" => 600.0,
            "silver" => 900.0,
            "gold" => 1200.0,
            "platinum" => 1500.0,
            "diamond" => 1800.0,
            "radiant" => 2100.0,
            _ => 1200.0,
        };
        let div = (self.division.clamp(1, 4) as f64 - 1.0) * 50.0;
        tier_base + div
    }
}

#[derive(Debug, Clone)]
pub enum DetectionFlag {
    PerformanceAnomaly { confidence: f64 },
    AcceleratedRating,
    UnderReview,
}

/// Ground truth known only to the simulation. Algorithms must never see this.
/// See `World.observations` / `World.observe` for what algorithms are allowed.
#[derive(Debug, Clone)]
pub struct PlayerReality {
    pub id: PlayerId,
    pub skill: SkillVector,
    pub skill_volatility: f64,
    pub improvement_rate: f64,
    pub consistency: f64,
    pub play_frequency: f64,
    pub session_length: f64,
    pub quit_probability: f64,
    pub party_id: Option<u64>,
    pub region: Region,
    pub account_age: u64,
    pub games_played: u64,
    pub fatigue: f64,
    pub tilt: f64,
    pub experience: u64,
    pub is_online: bool,
    pub archetype: String,
}

/// What rating, matchmaking, and detection systems are allowed to see.
///
/// There is deliberate overlap with `PlayerReality` (e.g. `skill_vector`) but
/// every system must operate on this type alone — never `PlayerReality`.
#[derive(Debug, Clone)]
pub struct PlayerObservation {
    pub id: PlayerId,
    pub rating: f64,
    pub hidden_mmr: f64,
    pub visible_rank: VisibleRank,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: u64,
    pub win_rate: f64,
    pub recent_performances: Vec<f64>,
    pub queue_joined_at: Option<SimTime>,
    pub is_online: bool,
    pub party_id: Option<u64>,
    pub session_history: VecDeque<u64>,
    pub quit_history: VecDeque<f64>,
    pub tilt_level: f64,
    pub game_mode: String,
    pub skill_vector: SkillVector,
    pub detection_flags: Vec<DetectionFlag>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_id_is_copy_and_eq() {
        let a = PlayerId(1);
        let b = PlayerId(1);
        let c = PlayerId(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.0, 1);
    }

    #[test]
    fn skillvector_onedimensional_overall() {
        let sv = SkillVector::one_dimensional(1200.0);
        assert_eq!(sv.overall(), 1200.0);
        assert_eq!(sv.dimensions["overall"], 1200.0);
    }

    #[test]
    fn skillvector_multidimensional_averages() {
        let mut dims = HashMap::new();
        dims.insert("aim".to_string(), 1500.0);
        dims.insert("movement".to_string(), 1100.0);
        let sv = SkillVector { dimensions: dims };
        assert_eq!(sv.overall(), 1300.0);
    }

    #[test]
    fn skillvector_weighted_overall_respects_weights() {
        let mut dims = HashMap::new();
        dims.insert("aim".to_string(), 100.0);
        dims.insert("movement".to_string(), 200.0);
        let sv = SkillVector { dimensions: dims };

        let mut weights = HashMap::new();
        weights.insert("aim".to_string(), 3.0);
        // unweighted movement defaults to 1.0
        let weighted = sv.weighted_overall(&weights);
        // (100*3 + 200*1) / (3+1) = 500 / 4 = 125
        assert!((weighted - 125.0).abs() < 1e-9);
    }

    #[test]
    fn skillvector_uniform_weights_match_overall() {
        let mut dims = HashMap::new();
        dims.insert("a".to_string(), 800.0);
        dims.insert("b".to_string(), 1200.0);
        let sv = SkillVector { dimensions: dims };
        let equal = sv.weighted_overall(&HashMap::new());
        assert!((equal - sv.overall()).abs() < 1e-9);
    }

    #[test]
    fn empty_skillvector_overall_is_zero() {
        let sv = SkillVector {
            dimensions: HashMap::new(),
        };
        assert_eq!(sv.overall(), 0.0);
        assert_eq!(sv.weighted_overall(&HashMap::new()), 0.0);
    }

    #[test]
    fn visible_rank_midpoint_bases_and_divisions() {
        let iron = VisibleRank {
            tier: "iron".to_string(),
            division: 1,
        };
        assert_eq!(iron.midpoint(), 300.0);
        let diamond3 = VisibleRank {
            tier: "diamond".to_string(),
            division: 3,
        };
        assert_eq!(diamond3.midpoint(), 1800.0 + 100.0);
    }

    #[test]
    fn visible_rank_midpoint_clamps_division() {
        let high = VisibleRank {
            tier: "silver".to_string(),
            division: 9,
        };
        assert_eq!(high.midpoint(), 900.0 + (4 - 1) as f64 * 50.0);
        let low = VisibleRank {
            tier: "silver".to_string(),
            division: 0,
        };
        assert_eq!(low.midpoint(), 900.0);
    }
}
