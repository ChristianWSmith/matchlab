//! Party effects on match quality: parties affect the game
//! through coordination bonuses, communication advantages, and premade
//! cohesion — integrating with the v0.5 team-model architecture.
use matchlab_core::player::PlayerId;
/// Context for party effect computation.
#[derive(Debug, Clone)]
pub struct PartyEffectContext {
    pub party_members: Vec<PlayerId>,
    pub coordination_level: f64,
}
/// A model for how parties affect team performance.
pub trait PartyEffect: Send + Sync {
    /// Apply the party effect to team strength.
    /// Returns the adjusted team strength.
    fn apply(&self, team_strength: f64, party_sizes: &[usize], context: &PartyEffectContext)
    -> f64;
}
/// Coordination bonus: linear bonus per party member.
#[derive(Debug, Clone)]
pub struct CoordinationBonus {
    /// Bonus added per party member beyond solo.
    pub bonus_per_member: f64,
}
impl PartyEffect for CoordinationBonus {
    fn apply(
        &self,
        team_strength: f64,
        party_sizes: &[usize],
        _context: &PartyEffectContext,
    ) -> f64 {
        let total_bonus: f64 = party_sizes
            .iter()
            .map(|&size| {
                if size > 1 {
                    self.bonus_per_member * (size - 1) as f64
                } else {
                    0.0
                }
            })
            .sum();
        team_strength + total_bonus
    }
}
/// Communication bonus: fixed bonus for any party (vs solo).
#[derive(Debug, Clone)]
pub struct CommunicationBonus {
    /// Bonus for being in a party (not solo).
    pub bonus: f64,
}
impl PartyEffect for CommunicationBonus {
    fn apply(
        &self,
        team_strength: f64,
        party_sizes: &[usize],
        _context: &PartyEffectContext,
    ) -> f64 {
        let has_party = party_sizes.iter().any(|&s| s > 1);
        if has_party {
            team_strength + self.bonus
        } else {
            team_strength
        }
    }
}
/// Party size effect: configurable bonus curve based on party size.
#[derive(Debug, Clone)]
pub struct PartySizeEffect {
    /// Bonus multiplier for each party size level.
    pub curve: Vec<(usize, f64)>,
}
impl PartyEffect for PartySizeEffect {
    fn apply(
        &self,
        team_strength: f64,
        party_sizes: &[usize],
        _context: &PartyEffectContext,
    ) -> f64 {
        let total_bonus: f64 = party_sizes
            .iter()
            .map(|&size| {
                self.curve
                    .iter()
                    .find(|&&(s, _)| s == size)
                    .map_or(0.0, |&(_, bonus)| bonus)
            })
            .sum();
        team_strength + total_bonus
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordination_bonus_scales_with_party_size() {
        let effect = CoordinationBonus {
            bonus_per_member: 5.0,
        };
        let ctx = PartyEffectContext {
            party_members: vec![],
            coordination_level: 1.0,
        };
        let result = effect.apply(100.0, &[1, 3], &ctx);
        assert!((result - 110.0).abs() < 1e-9);
    }
    #[test]
    fn communication_bonus_only_for_parties() {
        let effect = CommunicationBonus { bonus: 15.0 };
        let ctx = PartyEffectContext {
            party_members: vec![],
            coordination_level: 1.0,
        };
        let result = effect.apply(100.0, &[1, 1], &ctx);
        assert!((result - 100.0).abs() < 1e-9);
        let result = effect.apply(100.0, &[1, 3], &ctx);
        assert!((result - 115.0).abs() < 1e-9);
    }
    #[test]
    fn party_size_effect_follows_curve() {
        let effect = PartySizeEffect {
            curve: vec![(2, 3.0), (3, 8.0), (5, 20.0)],
        };
        let ctx = PartyEffectContext {
            party_members: vec![],
            coordination_level: 1.0,
        };
        let result = effect.apply(100.0, &[3, 7], &ctx);
        assert!((result - 108.0).abs() < 1e-9);
    }
    #[test]
    fn effects_are_deterministic() {
        let effect = CoordinationBonus {
            bonus_per_member: 5.0,
        };
        let ctx = PartyEffectContext {
            party_members: vec![],
            coordination_level: 1.0,
        };
        let r1 = effect.apply(100.0, &[1, 3], &ctx);
        let r2 = effect.apply(100.0, &[1, 3], &ctx);
        assert_eq!(r1, r2);
    }
}
