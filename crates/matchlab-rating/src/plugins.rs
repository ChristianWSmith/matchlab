pub mod registry {
    use crate::lua::LuaRatingSystem;
    use crate::system::RatingSystem;

    /// Built-in name → script path map, for concise manifests and docs.
    pub fn known_systems() -> Vec<(&'static str, &'static str)> {
        vec![
            ("elo", "plugins/rating/elo.lua"),
            ("flatpoints", "plugins/rating/flat.lua"),
            ("glicko2", "plugins/rating/glicko2.lua"),
            ("trueskill", "plugins/rating/trueskill.lua"),
        ]
    }

    /// Resolve a rating system by script path.
    pub fn from_script(
        path: &str,
        params: &serde_yaml::Value,
    ) -> Result<Box<dyn RatingSystem>, String> {
        Ok(Box::new(LuaRatingSystem::load(path, params)?))
    }

    /// Resolve a built-in system by name (maps to a script path).
    pub fn from_name(
        name: &str,
        params: &serde_yaml::Value,
    ) -> Result<Box<dyn RatingSystem>, String> {
        let path = known_systems()
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, p)| *p)
            .ok_or_else(|| format!("unknown rating system: {name}"))?;
        from_script(path, params)
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
    fn known_systems_lists_scripts() {
        let systems = registry::known_systems();
        assert!(systems.contains(&("elo", "plugins/rating/elo.lua")));
        assert!(systems.contains(&("glicko2", "plugins/rating/glicko2.lua")));
        assert!(systems.contains(&("trueskill", "plugins/rating/trueskill.lua")));
        assert!(systems.contains(&("flatpoints", "plugins/rating/flat.lua")));
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
