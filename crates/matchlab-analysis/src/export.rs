//! Raw output export (spec §14.5). v0.1 is JSON-only; parquet is out of scope.
//!
//! `RawDataExporter` accumulates per-match and per-observation traces for
//! external analysis; `write_result_json` writes the full `ExperimentResult`
//! (metrics JSON) under `OutputSpec.directory`.

use std::io;
use std::path::Path;

use matchlab_core::match_::MatchResult;
use matchlab_core::world::World;
use matchlab_experiments::ExperimentResult;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedMatch {
    pub match_id: String,
    pub tick: u64,
    pub winner: String,
    pub team_a: Vec<String>,
    pub team_b: Vec<String>,
    pub team_a_score: f64,
    pub team_b_score: f64,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedObservation {
    pub player_id: String,
    pub tick: u64,
    pub rating: f64,
    pub rating_deviation: f64,
    pub games_played: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Json,
    Parquet,
}

impl std::str::FromStr for ExportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(ExportFormat::Json),
            "parquet" => Ok(ExportFormat::Parquet),
            other => Err(format!("unsupported export format: {other}")),
        }
    }
}

pub struct RawDataExporter {
    pub directory: String,
    pub format: ExportFormat,
    pub matches: Vec<ExportedMatch>,
    pub observations: Vec<ExportedObservation>,
}

impl RawDataExporter {
    pub fn new(directory: String, format: ExportFormat) -> Self {
        Self {
            directory,
            format,
            matches: Vec::new(),
            observations: Vec::new(),
        }
    }

    pub fn record_match(&mut self, mr: &MatchResult, world: &World) {
        self.matches.push(ExportedMatch {
            match_id: mr.match_id.0.to_string(),
            tick: world.time.ticks(),
            winner: format!("{:?}", mr.winner),
            team_a: mr.team_a.iter().map(|p| p.0.to_string()).collect(),
            team_b: mr.team_b.iter().map(|p| p.0.to_string()).collect(),
            team_a_score: mr.team_a_score,
            team_b_score: mr.team_b_score,
            duration_secs: mr.duration.as_secs_f64(),
        });
    }

    pub fn record_observations(&mut self, world: &World) {
        let mut pids: Vec<_> = world.observations.keys().collect();
        pids.sort_by_key(|pid| pid.0);
        for pid in pids {
            let Some(obs) = world.observations.get(pid) else {
                continue;
            };
            self.observations.push(ExportedObservation {
                player_id: pid.0.to_string(),
                tick: world.time.ticks(),
                rating: obs.rating,
                rating_deviation: obs.rating_deviation,
                games_played: obs.games_played,
            });
        }
    }

    pub fn write(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        match self.format {
            ExportFormat::Json => {
                let matches_path = Path::new(&self.directory).join("matches.json");
                let matches_data = serde_json::to_string_pretty(&self.matches).unwrap_or_default();
                std::fs::write(matches_path, matches_data)?;

                let obs_path = Path::new(&self.directory).join("observations.json");
                let obs_data = serde_json::to_string_pretty(&self.observations).unwrap_or_default();
                std::fs::write(obs_path, obs_data)?;
            }
            ExportFormat::Parquet => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "parquet export is out of scope for v0.1",
                ));
            }
        }
        Ok(())
    }
}

/// Write the full metrics result (`ExperimentResult`) as pretty JSON under
/// `OutputSpec.directory`, per the ticket 10/11 pipeline.
pub fn write_result_json(result: &ExperimentResult, directory: &str) -> io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let path = Path::new(directory).join(format!("{}.json", result.name));
    let data = serde_json::to_string_pretty(result).map_err(io::Error::other)?;
    std::fs::write(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: matchlab_core::player::VisibleRank {
                tier: "gold".to_string(),
                division: 1,
            },
            rating_deviation: 250.0,
            volatility: 0.06,
            games_played: 3,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: VecDeque::new(),
            quit_history: VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            skill_vector: matchlab_core::player::SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
        }
    }

    fn reality(id: u64, skill: f64) -> PlayerReality {
        PlayerReality {
            id: PlayerId(id),
            skill: matchlab_core::player::SkillVector::one_dimensional(skill),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: matchlab_core::player::Region::EU,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".to_string(),
        }
    }

    fn sample_match() -> MatchResult {
        MatchResult {
            match_id: matchlab_core::match_::MatchId(9),
            winner: matchlab_core::match_::Team::B,
            team_a: vec![PlayerId(1), PlayerId(2)],
            team_b: vec![PlayerId(3)],
            team_a_score: 2.0,
            team_b_score: 5.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(1200.0),
            disconnected: false,
            forfeited: false,
            variance: 0.2,
            unexpected_events: Vec::new(),
        }
    }

    #[test]
    fn export_format_maps_names() {
        assert_eq!("json".parse::<ExportFormat>().unwrap(), ExportFormat::Json);
        assert_eq!(
            "parquet".parse::<ExportFormat>().unwrap(),
            ExportFormat::Parquet
        );
        assert!("csv".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn json_export_writes_parseable_files() {
        let dir = std::env::temp_dir().join("matchlab-export-test");
        let _ = std::fs::remove_dir_all(&dir);

        let mut world = World::new(SimRng::from_seed(3));
        world.add_player(reality(1, 1000.0), obs(1, 950.0));
        world.add_player(reality(2, 1000.0), obs(2, 1050.0));

        let exporter = {
            let mut e =
                RawDataExporter::new(dir.to_string_lossy().into_owned(), ExportFormat::Json);
            e.record_match(&sample_match(), &world);
            e.record_observations(&world);
            e
        };
        exporter.write().expect("json export succeeds");

        let matches_json = std::fs::read_to_string(dir.join("matches.json")).unwrap();
        let matches_data: Vec<ExportedMatch> = serde_json::from_str(&matches_json).unwrap();
        assert_eq!(matches_data.len(), 1);
        assert_eq!(matches_data[0].match_id, "9");
        assert_eq!(matches_data[0].winner, "B");
        assert_eq!(
            matches_data[0].team_a,
            vec!["1".to_string(), "2".to_string()]
        );

        let obs_json = std::fs::read_to_string(dir.join("observations.json")).unwrap();
        let obs_data: Vec<ExportedObservation> = serde_json::from_str(&obs_json).unwrap();
        assert_eq!(obs_data.len(), 2);
        // Sorted by player id → deterministic order regardless of HashMap order.
        assert_eq!(obs_data[0].player_id, "1");
        assert_eq!(obs_data[1].player_id, "2");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parquet_export_is_rejected_for_v01() {
        let dir = std::env::temp_dir().join("matchlab-parquet-test");
        let exporter =
            RawDataExporter::new(dir.to_string_lossy().into_owned(), ExportFormat::Parquet);
        let err = exporter.write().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn observations_are_recorded_deterministically() {
        let mut world = World::new(SimRng::from_seed(4));
        world.add_player(reality(3, 1000.0), obs(3, 980.0));
        world.add_player(reality(1, 1000.0), obs(1, 990.0));
        world.add_player(reality(2, 1000.0), obs(2, 970.0));

        let mut a = RawDataExporter::new("x".to_string(), ExportFormat::Json);
        let mut b = RawDataExporter::new("x".to_string(), ExportFormat::Json);
        a.record_observations(&world);
        b.record_observations(&world);
        let ids_a: Vec<&str> = a
            .observations
            .iter()
            .map(|o| o.player_id.as_str())
            .collect();
        let ids_b: Vec<&str> = b
            .observations
            .iter()
            .map(|o| o.player_id.as_str())
            .collect();
        assert_eq!(ids_a, vec!["1", "2", "3"]);
        assert_eq!(ids_a, ids_b);
    }

    const MINI: &str = r#"
experiment:
  name: mini_out
  seed: 7
  population:
    size: 100
    seed: 7
    archetypes:
      - name: stable
        proportion: 1.0
        skill_distribution: { type: normal, mean: 1000, stddev: 150 }
        skill_volatility: 0.0
        improvement_rate: 0.0
        play_frequency: 0.8
        session_length: 1800.0
        quit_probability: 0.0
  game:
    team_size: 1
    outcome_model: logistic
    beta: 400.0
    noise: 0.05
  matchmaking:
    algorithm: batch
    batch_interval: 10
    max_queue_time: 60.0
  rating:
    systems:
      - name: elo
        k_factor: 32.0
        initial_rating: 1000.0
        beta: 400.0
  metrics:
    - match_quality
    - queue_time
    - rating_accuracy
  cohorts: []
  duration:
    matches: 40
    max_time: 200000.0
  output:
    directory: results/
    formats: [json]
    plots: false
    report: false
"#;

    fn mini_config() -> matchlab_experiments::ExperimentConfig {
        serde_yaml::from_str(MINI).unwrap()
    }

    #[test]
    fn write_result_json_lands_metrics_under_directory() {
        let dir = std::env::temp_dir().join("matchlab-result-write");
        let _ = std::fs::remove_dir_all(&dir);

        let result = matchlab_experiments::runner::ExperimentRunner::run(&mini_config()).unwrap();
        write_result_json(&result, dir.to_string_lossy().as_ref()).unwrap();

        let path = dir.join(format!("{}.json", result.name));
        let data = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&data).unwrap();
        for metric in ["match_quality", "queue_time", "rating_accuracy"] {
            assert!(
                value["metrics"].get(metric).is_some(),
                "missing metric {metric} in {path:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_seed_writes_identical_output_content() {
        let config = mini_config();
        let a = matchlab_experiments::runner::ExperimentRunner::run(&config).unwrap();
        let b = matchlab_experiments::runner::ExperimentRunner::run(&config).unwrap();

        let mut ja = serde_json::to_value(&a).unwrap();
        let mut jb = serde_json::to_value(&b).unwrap();
        // Wall-clock timestamp is run metadata, not experiment output.
        ja["timestamp"] = serde_json::Value::Null;
        jb["timestamp"] = serde_json::Value::Null;

        assert_eq!(ja, jb, "same seed must produce identical output content");
    }
}
