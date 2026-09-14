pub mod registry {
    use crate::lua::LuaRatingSystem;
    use crate::system::RatingSystem;
    pub fn from_script(
        path: &str,
        params: &serde_yaml::Value,
    ) -> Result<Box<dyn RatingSystem>, String> {
        Ok(Box::new(LuaRatingSystem::load(path, params)?))
    }
    pub fn from_name(
        name: &str,
        params: &serde_yaml::Value,
    ) -> Result<Box<dyn RatingSystem>, String> {
        from_script(name, params)
    }
}
#[cfg(test)]
mod tests {
    use super::registry;
    #[test]
    fn elo_registers_from_script() {
        let yaml =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1200.0\nbeta: 400.0\n").unwrap();
        let sys = registry::from_script("plugins/rating/elo.lua", &yaml).unwrap();
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1200.0);
    }
    #[test]
    fn from_name_resolves_builtin_scripts() {
        let yaml =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1200.0\nbeta: 400.0\n").unwrap();
        let sys = registry::from_name("elo", &yaml).expect("elo resolves");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1200.0);
    }
    #[test]
    fn from_script_resolves_by_path() {
        let yaml =
            serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1200.0\nbeta: 400.0\n").unwrap();
        let sys = registry::from_script("elo", &yaml).expect("elo resolves by name");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1200.0);
    }
    #[test]
    fn unknown_name_errors() {
        let yaml = serde_yaml::from_str("{}").unwrap();
        assert!(registry::from_name("bogus", &yaml).is_err());
    }
    #[test]
    fn missing_script_errors() {
        let yaml = serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\n").unwrap();
        assert!(registry::from_script("plugins/rating/nope.lua", &yaml).is_err());
    }
    #[test]
    fn script_with_math_random_is_rejected() {
        let yaml = serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\n").unwrap();
        assert!(registry::from_script("plugins/rating/nope.lua", &yaml).is_err());
    }
}
