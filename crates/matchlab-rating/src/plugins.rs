pub mod registry {
    use crate::system::RatingSystem;

    pub fn all_systems() -> Vec<&'static str> {
        vec!["elo", "flatpoints", "glicko2", "trueskill"]
    }

    pub fn from_name(name: &str, config: &serde_yaml::Value) -> Option<Box<dyn RatingSystem>> {
        match name {
            "elo" => Some(Box::new(crate::elo::EloRatingSystem::from_yaml(config)?)),
            "flatpoints" => Some(Box::new(crate::flat::FlatPointsRatingSystem::from_yaml(
                config,
            )?)),
            "glicko2" => Some(Box::new(crate::glicko::Glicko2RatingSystem::from_yaml(
                config,
            )?)),
            "trueskill" => Some(Box::new(
                crate::trueskill::TrueSkillRatingSystem::from_yaml(config)?,
            )),
            "lua:elo" => {
                let path = config.get("script")?.as_str()?;
                let hooks = crate::hooks::LuaHooks::load(path).ok()?;
                let sys = crate::elo::EloRatingSystem::from_yaml(config)?;
                Some(Box::new(crate::elo::EloRatingSystem::with_hooks(
                    sys.config, hooks,
                )))
            }
            "lua:glicko2" => {
                let path = config.get("script")?.as_str()?;
                let hooks = crate::hooks::LuaHooks::load(path).ok()?;
                let sys = crate::glicko::Glicko2RatingSystem::from_yaml(config)?;
                Some(Box::new(crate::glicko::Glicko2RatingSystem::with_hooks(
                    sys.config, hooks,
                )))
            }
            "lua:trueskill" => {
                let path = config.get("script")?.as_str()?;
                let hooks = crate::hooks::LuaHooks::load(path).ok()?;
                let sys = crate::trueskill::TrueSkillRatingSystem::from_yaml(config)?;
                Some(Box::new(
                    crate::trueskill::TrueSkillRatingSystem::with_hooks(sys.config, hooks),
                ))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::registry;
    use crate::system::ObservationType;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(prefix: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("{}_{}_{}.lua", prefix, std::process::id(), n))
    }

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
    fn glicko2_registers_from_name() {
        let yaml = serde_yaml::from_str("initial_rating: 1500.0\n").unwrap();
        let sys = registry::from_name("glicko2", &yaml).expect("glicko2 should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1500.0);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn trueskill_registers_from_name() {
        let yaml = serde_yaml::from_str("initial_mean: 1200.0\n").unwrap();
        let sys = registry::from_name("trueskill", &yaml).expect("trueskill should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1200.0);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);
    }

    #[test]
    fn lua_trueskill_registers_from_name() {
        let script = "function on_rating_bounds() return { floor = 100.0, ceiling = 3000.0 } end";
        let path = temp_path("test_plugin_lua_trueskill");
        std::fs::write(&path, script).unwrap();

        let yaml = serde_yaml::from_str(&format!(
            "script: {}\ninitial_mean: 1500.0\n",
            path.to_str().unwrap()
        ))
        .unwrap();
        let sys =
            registry::from_name("lua:trueskill", &yaml).expect("lua:trueskill should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1500.0);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn lua_glicko2_registers_from_name() {
        let script = "function on_rating_bounds() return { floor = 100.0, ceiling = 3000.0 } end";
        let path = temp_path("test_plugin_lua_glicko2");
        std::fs::write(&path, script).unwrap();

        let yaml = serde_yaml::from_str(&format!(
            "script: {}\ninitial_rating: 1500.0\ninitial_rd: 350.0\n",
            path.to_str().unwrap()
        ))
        .unwrap();
        let sys = registry::from_name("lua:glicko2", &yaml).expect("lua:glicko2 should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1500.0);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn lua_glicko2_missing_script_returns_none() {
        let yaml = serde_yaml::from_str("initial_rating: 1500.0\n").unwrap();
        assert!(registry::from_name("lua:glicko2", &yaml).is_none());
    }

    #[test]
    fn unknown_name_returns_none() {
        let yaml = serde_yaml::from_str("{}").unwrap();
        assert!(registry::from_name("bogus", &yaml).is_none());
    }

    #[test]
    fn all_systems_lists_implemented_systems() {
        let systems = registry::all_systems();
        assert!(systems.contains(&"elo"));
        assert!(systems.contains(&"flatpoints"));
        assert!(systems.contains(&"glicko2"));
        assert!(systems.contains(&"trueskill"));
    }

    #[test]
    fn from_name_requires_valid_config() {
        // Missing required fields → None.
        let yaml = serde_yaml::from_str("{}").unwrap();
        assert!(registry::from_name("elo", &yaml).is_none());
    }

    #[test]
    fn lua_elo_registers_from_name() {
        let script = "function on_k_factor() return 48.0 end";
        let path = temp_path("test_plugin_lua_elo");
        std::fs::write(&path, script).unwrap();

        let yaml = serde_yaml::from_str(&format!(
            "script: {}\nk_factor: 32.0\ninitial_rating: 1200.0\nbeta: 400.0\n",
            path.to_str().unwrap()
        ))
        .unwrap();
        let sys = registry::from_name("lua:elo", &yaml).expect("lua:elo should register");
        let state = sys.initialize(matchlab_core::player::PlayerId(1));
        assert_eq!(sys.rating(&state), 1200.0);
        assert_eq!(sys.information_budget(), vec![ObservationType::WinLoss]);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn lua_elo_missing_script_returns_none() {
        let yaml = serde_yaml::from_str("k_factor: 32.0\ninitial_rating: 1000.0\n").unwrap();
        assert!(registry::from_name("lua:elo", &yaml).is_none());
    }

    #[test]
    fn lua_elo_invalid_script_returns_none() {
        let path = temp_path("test_plugin_bad");
        std::fs::write(&path, "bad lua syntax {{{").unwrap();

        let yaml = serde_yaml::from_str(&format!(
            "script: {}\nk_factor: 32.0\ninitial_rating: 1000.0\n",
            path.to_str().unwrap()
        ))
        .unwrap();
        assert!(registry::from_name("lua:elo", &yaml).is_none());

        let _ = std::fs::remove_file(&path);
    }
}
