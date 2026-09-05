use matchlab_core::player::{PlayerId, PlayerReality, Region};
use matchlab_core::world::World;

/// Map a true-skill value to a coarse tier label, used to align a player's
/// reality with the `RankTier` cohort filter and visible-rank brackets.
pub fn tier_for_skill(skill: f64) -> String {
    match skill {
        s if s < 400.0 => "iron".to_string(),
        s if s < 700.0 => "bronze".to_string(),
        s if s < 1000.0 => "silver".to_string(),
        s if s < 1300.0 => "gold".to_string(),
        s if s < 1600.0 => "platinum".to_string(),
        s if s < 1900.0 => "diamond".to_string(),
        _ => "radiant".to_string(),
    }
}

#[derive(Debug, Clone)]
pub enum CohortFilter {
    All,
    SkillRange(f64, f64),
    Archetype(String),
    GamesPlayedRange(u64, u64),
    Region(Region),
    PartySize(usize),
    SessionLength(f64, f64),
    RankTier(String),
    IsSmurfByProperties,
}

impl CohortFilter {
    pub fn matches(&self, reality: &PlayerReality) -> bool {
        match self {
            CohortFilter::All => true,
            CohortFilter::SkillRange(low, high) => {
                reality.skill.overall() >= *low && reality.skill.overall() <= *high
            }
            CohortFilter::Archetype(name) => reality.archetype == *name,
            CohortFilter::GamesPlayedRange(low, high) => {
                reality.games_played >= *low && reality.games_played <= *high
            }
            CohortFilter::Region(region) => reality.region == *region,
            CohortFilter::PartySize(size) => reality
                .party_id
                .map(|_| *size > 1)
                .or(Some(*size == 1))
                .unwrap_or_default(),
            CohortFilter::SessionLength(min, max) => {
                let s = reality.session_length;
                s >= *min && s <= *max
            }
            CohortFilter::RankTier(tier) => {
                let t = tier_for_skill(reality.skill.overall());
                t == *tier
            }
            CohortFilter::IsSmurfByProperties => {
                reality.skill.overall() > 1300.0 && reality.games_played < 20
            }
        }
    }

    pub fn filter_player_ids(&self, world: &World) -> Vec<PlayerId> {
        world
            .players
            .iter()
            .filter(|(_, reality)| self.matches(reality))
            .map(|(pid, _)| *pid)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::SkillVector;

    fn reality(skill: f64, games: u64, archetype: &str) -> PlayerReality {
        PlayerReality {
            id: PlayerId(1),
            skill: SkillVector::one_dimensional(skill),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::NA,
            account_age: 0,
            games_played: games,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: archetype.to_string(),
        }
    }

    #[test]
    fn skill_range_filters_correctly() {
        let f = CohortFilter::SkillRange(900.0, 1100.0);
        assert!(f.matches(&reality(1000.0, 10, "stable")));
        assert!(!f.matches(&reality(800.0, 10, "stable")));
        assert!(!f.matches(&reality(1200.0, 10, "stable")));
    }

    #[test]
    fn archetype_filters_correctly() {
        let f = CohortFilter::Archetype("smurf".to_string());
        assert!(f.matches(&reality(1500.0, 5, "smurf")));
        assert!(!f.matches(&reality(1000.0, 10, "stable")));
    }

    #[test]
    fn smurf_by_properties_filters_correctly() {
        let f = CohortFilter::IsSmurfByProperties;
        assert!(f.matches(&reality(1500.0, 5, "stable")));
        assert!(!f.matches(&reality(1500.0, 50, "stable")));
        assert!(!f.matches(&reality(1000.0, 5, "stable")));
    }

    #[test]
    fn tier_for_skill_maps_boundaries() {
        assert_eq!(tier_for_skill(300.0), "iron");
        assert_eq!(tier_for_skill(500.0), "bronze");
        assert_eq!(tier_for_skill(900.0), "silver");
        assert_eq!(tier_for_skill(1200.0), "gold");
        assert_eq!(tier_for_skill(1500.0), "platinum");
        assert_eq!(tier_for_skill(1800.0), "diamond");
        assert_eq!(tier_for_skill(2200.0), "radiant");
    }

    #[test]
    fn all_matches_everything() {
        assert!(CohortFilter::All.matches(&reality(500.0, 0, "x")));
    }
}
