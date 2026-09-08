//! Team composition and interaction: first-class mechanism for
//! individual skills to interact at the team level, supporting synergy, role
//! compatibility, diminishing returns, and composition effects.
use matchlab_core::player::SkillVector;
/// Contextual information for team performance calculation.
#[derive(Debug, Clone)]
pub struct TeamContext {
    pub team_size: usize,
    pub role_requirements: Option<Vec<String>>,
}
/// A model that aggregates individual player performances into team strength.
pub trait TeamModel: Send + Sync {
    /// Compute team strength from individual realized performances.
    fn team_strength(&self, performances: &[SkillVector], context: &TeamContext) -> f64;
}
/// Additive team model: sum of individual skill across all dimensions.
/// This is the current `composition.lua` behavior, formalized.
pub struct AdditiveTeamModel;
impl TeamModel for AdditiveTeamModel {
    fn team_strength(&self, performances: &[SkillVector], _context: &TeamContext) -> f64 {
        performances.iter().map(|p| p.overall()).sum()
    }
}
/// Weighted team model: sum of per-dimension weighted skill values.
pub struct WeightedTeamModel {
    pub weights: std::collections::HashMap<String, f64>,
}
impl TeamModel for WeightedTeamModel {
    fn team_strength(&self, performances: &[SkillVector], _context: &TeamContext) -> f64 {
        performances
            .iter()
            .map(|p| p.weighted_overall(&self.weights))
            .sum()
    }
}
/// Complementary team model: gives a bonus when team members have diverse
/// skill profiles (high variance in per-player overall skill).
pub struct ComplementaryTeamModel {
    /// Bonus multiplier applied to team strength when diversity exceeds
    /// the threshold.
    pub synergy_bonus: f64,
    /// Minimum coefficient of variation in player skill to trigger bonus.
    pub complementarity_threshold: f64,
}
impl TeamModel for ComplementaryTeamModel {
    fn team_strength(&self, performances: &[SkillVector], _context: &TeamContext) -> f64 {
        let base: f64 = performances.iter().map(|p| p.overall()).sum();
        if performances.len() < 2 {
            return base;
        }
        let means: Vec<f64> = performances.iter().map(|p| p.overall()).collect();
        let mean = means.iter().sum::<f64>() / means.len() as f64;
        let variance = means.iter().map(|m| (m - mean).powi(2)).sum::<f64>() / means.len() as f64;
        let stddev = variance.sqrt();
        let cv = if mean.abs() > 1e-12 {
            stddev / mean
        } else {
            0.0
        };
        if cv >= self.complementarity_threshold {
            base * (1.0 + self.synergy_bonus)
        } else {
            base
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn make_skill(overall: f64) -> SkillVector {
        SkillVector::one_dimensional(overall)
    }
    #[test]
    fn additive_team_model_sums_players() {
        let model = AdditiveTeamModel;
        let ctx = TeamContext {
            team_size: 3,
            role_requirements: None,
        };
        let players = vec![make_skill(100.0), make_skill(200.0), make_skill(300.0)];
        assert!((model.team_strength(&players, &ctx) - 600.0).abs() < 1e-9);
    }
    #[test]
    fn additive_team_model_single_player() {
        let model = AdditiveTeamModel;
        let ctx = TeamContext {
            team_size: 1,
            role_requirements: None,
        };
        let players = vec![make_skill(1500.0)];
        assert!((model.team_strength(&players, &ctx) - 1500.0).abs() < 1e-9);
    }
    #[test]
    fn weighted_team_model_applies_weights() {
        let mut weights = std::collections::HashMap::new();
        weights.insert("a".to_string(), 3.0);
        weights.insert("b".to_string(), 1.0);
        let model = WeightedTeamModel { weights };
        let ctx = TeamContext {
            team_size: 2,
            role_requirements: None,
        };
        let p1 = SkillVector {
            dimensions: [("a".to_string(), 100.0), ("b".to_string(), 100.0)]
                .into_iter()
                .collect(),
        };
        let p2 = SkillVector {
            dimensions: [("a".to_string(), 100.0), ("b".to_string(), 300.0)]
                .into_iter()
                .collect(),
        };
        let players = vec![p1, p2];
        assert!((model.team_strength(&players, &ctx) - 250.0).abs() < 1e-9);
    }
    #[test]
    fn complementary_team_model_bonus_for_diverse_profiles() {
        let model = ComplementaryTeamModel {
            synergy_bonus: 0.2,
            complementarity_threshold: 0.3,
        };
        let ctx = TeamContext {
            team_size: 2,
            role_requirements: None,
        };
        let diverse = vec![make_skill(100.0), make_skill(300.0)];
        let base_diverse = model.team_strength(&diverse, &ctx);
        assert!(
            (base_diverse - 480.0).abs() < 1e-9,
            "diverse team gets 20% bonus: {base_diverse}"
        );
    }
    #[test]
    fn complementary_team_model_no_bonus_for_similar_profiles() {
        let model = ComplementaryTeamModel {
            synergy_bonus: 0.2,
            complementarity_threshold: 0.3,
        };
        let ctx = TeamContext {
            team_size: 2,
            role_requirements: None,
        };
        let similar = vec![make_skill(200.0), make_skill(210.0)];
        let base_similar = model.team_strength(&similar, &ctx);
        assert!(
            (base_similar - 410.0).abs() < 1e-9,
            "similar team gets no bonus: {base_similar}"
        );
    }
    #[test]
    fn complementary_team_model_single_player_no_bonus() {
        let model = ComplementaryTeamModel {
            synergy_bonus: 0.2,
            complementarity_threshold: 0.3,
        };
        let ctx = TeamContext {
            team_size: 1,
            role_requirements: None,
        };
        let players = vec![make_skill(1500.0)];
        let strength = model.team_strength(&players, &ctx);
        assert!((strength - 1500.0).abs() < 1e-9);
    }
    #[test]
    fn all_models_deterministic_for_fixed_inputs() {
        let additive = AdditiveTeamModel;
        let weighted = WeightedTeamModel {
            weights: std::collections::HashMap::new(),
        };
        let complementary = ComplementaryTeamModel {
            synergy_bonus: 0.1,
            complementarity_threshold: 0.5,
        };
        let ctx = TeamContext {
            team_size: 2,
            role_requirements: None,
        };
        let players = vec![make_skill(100.0), make_skill(200.0)];
        for _ in 0..10 {
            assert!((additive.team_strength(&players, &ctx) - 300.0).abs() < 1e-9);
            assert!((weighted.team_strength(&players, &ctx) - 300.0).abs() < 1e-9);
            let s = complementary.team_strength(&players, &ctx);
            assert!((s - 300.0).abs() < 1e-9 || (s - 330.0).abs() < 1e-9);
        }
    }
}
