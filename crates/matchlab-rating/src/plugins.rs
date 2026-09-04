pub mod registry {
    use crate::system::RatingSystem;

    pub fn all_systems() -> Vec<&'static str> {
        vec!["elo", "flatpoints"]
    }

    pub fn from_name(name: &str, config: &serde_yaml::Value) -> Option<Box<dyn RatingSystem>> {
        match name {
            "elo" => Some(Box::new(crate::elo::EloRatingSystem::from_yaml(config)?)),
            "flatpoints" => Some(Box::new(crate::flat::FlatPointsRatingSystem::from_yaml(
                config,
            )?)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::registry;
    use crate::system::ObservationType;

    #[test]
    fn elo_registers_from_name() {
        let yaml =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1200.0\nbeta: 400.0\n").unwrap();
        let sys = registry::from_name("elo", &yaml).expect("elo should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1200.0);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn flatpoints_registers_from_name() {
        let yaml = serde_yaml::from_str("initial_rating: 750.0\n").unwrap();
        let sys = registry::from_name("flatpoints", &yaml).expect("flatpoints should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 750.0);
    }

    #[test]
    fn glicko2_is_not_implemented() {
        let yaml = serde_yaml::from_str("rating: 1000.0\n").unwrap();
        assert!(registry::from_name("glicko2", &yaml).is_none());
        assert!(registry::from_name("trueskill", &yaml).is_none());
    }

    #[test]
    fn unknown_name_returns_none() {
        let yaml = serde_yaml::from_str("{}").unwrap();
        assert!(registry::from_name("bogus", &yaml).is_none());
    }

    #[test]
    fn all_systems_lists_v01_systems() {
        let systems = registry::all_systems();
        assert!(systems.contains(&"elo"));
        assert!(systems.contains(&"flatpoints"));
        assert!(!systems.contains(&"glicko2"));
        assert!(!systems.contains(&"trueskill"));
    }

    #[test]
    fn from_name_requires_valid_config() {
        // Missing required fields → None.
        let yaml = serde_yaml::from_str("{}").unwrap();
        assert!(registry::from_name("elo", &yaml).is_none());
    }
}
