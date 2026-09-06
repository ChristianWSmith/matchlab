//! Readable experiment reports (spec §14.4).
//!
//! v0.1 scope is Markdown + JSON; HTML is out of scope. The report captures
//! name, config hash, git commit, and a metrics table so an experiment can be
//! reproduced and audited from the report alone.

use matchlab_experiments::ExperimentResult;

#[derive(Debug, Clone, PartialEq)]
pub struct ReportConfig {
    pub include_plots: bool,
    pub include_raw_data: bool,
    pub format: ReportFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportFormat {
    Json,
    Markdown,
}

/// Markdown report for a single experiment.
pub fn generate_report(result: &ExperimentResult) -> String {
    generate_markdown(std::slice::from_ref(result))
}

/// Generate a report for one or more experiments (spec §14.4).
pub fn generate_comparison_report(results: &[ExperimentResult], config: &ReportConfig) -> String {
    match config.format {
        ReportFormat::Markdown => generate_markdown(results),
        ReportFormat::Json => serde_json::to_string_pretty(results).unwrap_or_default(),
    }
}

fn generate_markdown(results: &[ExperimentResult]) -> String {
    let mut out = String::from("# matchlab Experiment Results\n\n");
    for result in results {
        out.push_str(&format!("## {}\n\n", result.name));
        out.push_str(&format!("Config: `{}`\n\n", result.config_hash));
        out.push_str(&format!("Git commit: `{}`\n\n", result.git_commit));
        out.push_str(&format!(
            "Matches completed: {}\n\n",
            result.matches_completed
        ));
        out.push_str(&format!(
            "Simulated time: {:.1}s\n\n",
            result.simulated_time_secs
        ));
        out.push_str("| Metric | Value |\n|--------|-------|\n");
        for (name, value) in &result.metrics {
            out.push_str(&format!("| {} | {:?} |\n", name, value));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use matchlab_core::world::World;
    use matchlab_metrics::MetricsEngine;
    use matchlab_metrics::lua::LuaMetricCollector;
    use std::collections::VecDeque;

    fn sample_result(name: &str, config_hash: &str) -> ExperimentResult {
        let mut engine = MetricsEngine::new();
        engine.register(Box::new(
            LuaMetricCollector::load(
                "plugins/metrics/match_quality.lua",
                &serde_yaml::Value::Null,
            )
            .unwrap(),
        ));

        let mut world = World::new(SimRng::from_seed(1));
        let pid = PlayerId(1);
        let obs = PlayerObservation {
            id: pid,
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: matchlab_core::player::VisibleRank {
                tier: "gold".to_string(),
                division: 2,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 10,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            skill_vector: matchlab_core::player::SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
            role: None,
        };
        let reality = PlayerReality {
            id: pid,
            skill: matchlab_core::player::SkillVector::one_dimensional(1000.0),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: matchlab_core::player::Region::NA,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".to_string(),
            role: None,
        };
        world.add_player(reality, obs);

        let mr = matchlab_core::match_::MatchResult {
            match_id: matchlab_core::match_::MatchId(1),
            winner: matchlab_core::match_::Team::A,
            team_a: vec![pid],
            team_b: vec![pid],
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(30.0),
            disconnected: false,
            forfeited: false,
            variance: 0.1,
            unexpected_events: Vec::new(),
        };
        engine.record_match(&mr, &world);
        engine.finalize();

        let metrics: std::collections::BTreeMap<_, _> = engine
            .results()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        ExperimentResult {
            experiment_id: format!("{name}-{config_hash}"),
            name: name.to_string(),
            config_hash: config_hash.to_string(),
            git_commit: "0f2c9a7".to_string(),
            timestamp: "2026-09-04T00:00:00Z".to_string(),
            matches_completed: 7,
            matches_formed: 7,
            simulated_time_secs: 210.0,
            metrics,
            utility_score: None,
        }
    }

    #[test]
    fn markdown_includes_reproducibility_and_metrics_table() {
        let r = sample_result("probe", "abcd1234ef567890");
        let md = generate_report(&r);
        assert!(md.contains("## probe"));
        assert!(md.contains("Config: `abcd1234ef567890`"));
        assert!(md.contains("Git commit: `0f2c9a7`"));
        assert!(md.contains("| match_quality |"));
    }

    #[test]
    fn json_report_roundtrips() {
        let r = sample_result("probe", "abcd1234ef567890");
        let config = ReportConfig {
            include_plots: false,
            include_raw_data: false,
            format: ReportFormat::Json,
        };
        let json = generate_comparison_report(std::slice::from_ref(&r), &config);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["name"], "probe");
        assert_eq!(parsed[0]["config_hash"], "abcd1234ef567890");
        assert!(parsed[0]["metrics"].is_object());
    }
}
