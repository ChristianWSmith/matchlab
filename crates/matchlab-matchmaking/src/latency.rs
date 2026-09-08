//! Latency model: deterministic statistical latency estimation
//! from player locations, with team-level statistics for matchmaking.
use matchlab_core::player::{GeoLocation, PlayerId, Region};
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;
use std::collections::HashMap;
/// Configurable latency matrix between region pairs.
#[derive(Debug, Clone, Default)]
pub struct LatencyMatrix {
    latencies: HashMap<(Region, Region), f64>,
}
impl LatencyMatrix {
    /// Create a latency matrix from a map of (region_a, region_b) → base latency.
    pub fn new(latencies: HashMap<(Region, Region), f64>) -> Self {
        Self { latencies }
    }
    /// Estimate base latency between two regions (symmetric).
    pub fn base_latency(&self, a: Region, b: Region) -> f64 {
        if a == b {
            self.latencies.get(&(a, b)).copied().unwrap_or(20.0)
        } else {
            self.latencies
                .get(&(a, b))
                .or_else(|| self.latencies.get(&(b, a)))
                .copied()
                .unwrap_or(80.0)
        }
    }
    /// Estimate latency between two geographic locations, including distance
    /// factor and jitter.
    pub fn estimate_latency(
        &self,
        a: &GeoLocation,
        b: &GeoLocation,
        jitter_stddev: f64,
        rng: &mut SimRng,
    ) -> f64 {
        let base = self.base_latency(a.region, b.region);
        let dx = a.latitude - b.latitude;
        let dy = a.longitude - b.longitude;
        let distance_factor = (dx * dx + dy * dy).sqrt() / 100.0;
        let base_with_distance = base + distance_factor * 5.0;
        let jitter = if jitter_stddev > 0.0 {
            rng.sample_normal(0.0, jitter_stddev)
        } else {
            0.0
        };
        (base_with_distance + jitter).max(0.0)
    }
}
/// Latency statistics for a team.
#[derive(Debug, Clone)]
pub struct TeamLatencyStats {
    pub mean_latency: f64,
    pub max_latency: f64,
    pub latency_variance: f64,
    pub fraction_above_threshold: f64,
}
/// The latency model: computes latency from locations and provides team stats.
#[derive(Debug, Clone)]
pub struct LatencyModel {
    pub latency_matrix: LatencyMatrix,
    pub jitter_stddev: f64,
}
impl LatencyModel {
    pub fn new(latency_matrix: LatencyMatrix, jitter_stddev: f64) -> Self {
        Self {
            latency_matrix,
            jitter_stddev,
        }
    }
    /// Estimate latency between a player and a reference location.
    pub fn estimate_latency(
        &self,
        player_location: &GeoLocation,
        reference: &GeoLocation,
        rng: &mut SimRng,
    ) -> f64 {
        self.latency_matrix
            .estimate_latency(player_location, reference, self.jitter_stddev, rng)
    }
    /// Compute team latency statistics relative to a reference location.
    pub fn team_latency(
        &self,
        team: &[PlayerId],
        reference: &GeoLocation,
        world: &World,
    ) -> TeamLatencyStats {
        let latencies: Vec<f64> = team
            .iter()
            .filter_map(|pid| {
                let reality = world.players.get(pid)?;
                let mut rng = SimRng::from_seed(pid.0.wrapping_add(42));
                Some(self.estimate_latency(&reality.location, reference, &mut rng))
            })
            .collect();
        if latencies.is_empty() {
            return TeamLatencyStats {
                mean_latency: 0.0,
                max_latency: 0.0,
                latency_variance: 0.0,
                fraction_above_threshold: 0.0,
            };
        }
        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let max = latencies.iter().cloned().fold(0.0f64, f64::max);
        let variance =
            latencies.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / latencies.len() as f64;
        let threshold = 100.0;
        let above = latencies.iter().filter(|&&l| l > threshold).count();
        let fraction = above as f64 / latencies.len() as f64;
        TeamLatencyStats {
            mean_latency: mean,
            max_latency: max,
            latency_variance: variance,
            fraction_above_threshold: fraction,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn default_matrix() -> LatencyMatrix {
        let mut map = HashMap::new();
        map.insert((Region::NA, Region::NA), 20.0);
        map.insert((Region::EU, Region::EU), 25.0);
        map.insert((Region::NA, Region::EU), 80.0);
        LatencyMatrix::new(map)
    }
    #[test]
    fn same_region_latency() {
        let matrix = default_matrix();
        assert!((matrix.base_latency(Region::NA, Region::NA) - 20.0).abs() < 1e-9);
        assert!((matrix.base_latency(Region::EU, Region::EU) - 25.0).abs() < 1e-9);
    }
    #[test]
    fn cross_region_latency() {
        let matrix = default_matrix();
        assert!((matrix.base_latency(Region::NA, Region::EU) - 80.0).abs() < 1e-9);
        assert!((matrix.base_latency(Region::EU, Region::NA) - 80.0).abs() < 1e-9);
    }
    #[test]
    fn unknown_region_pair_uses_default() {
        let matrix = default_matrix();
        let latency = matrix.base_latency(Region::NA, Region::Asia);
        assert!(latency > 0.0, "unknown pair should have default latency");
    }
    #[test]
    fn latency_estimation_deterministic() {
        let matrix = default_matrix();
        let model = LatencyModel::new(matrix, 5.0);
        let a = GeoLocation::new(Region::NA, 40.0, -74.0);
        let b = GeoLocation::new(Region::NA, 34.0, -118.0);
        let l1 = model.estimate_latency(&a, &b, &mut SimRng::from_seed(42));
        let l2 = model.estimate_latency(&a, &b, &mut SimRng::from_seed(42));
        assert!((l1 - l2).abs() < 1e-9, "latency must be deterministic");
    }
    #[test]
    fn jitter_adds_variance() {
        let matrix = default_matrix();
        let model = LatencyModel::new(matrix, 10.0);
        let a = GeoLocation::new(Region::NA, 40.0, -74.0);
        let b = GeoLocation::new(Region::NA, 34.0, -118.0);
        let mut latencies = Vec::new();
        for i in 0..100 {
            let l = model.estimate_latency(&a, &b, &mut SimRng::from_seed(i));
            latencies.push(l);
        }
        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let variance =
            latencies.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / latencies.len() as f64;
        assert!(variance > 0.0, "jitter should produce variance");
        assert!(mean > 0.0, "mean latency should be positive");
    }
    #[test]
    fn team_latency_stats_correct() {
        let matrix = default_matrix();
        let model = LatencyModel::new(matrix, 0.0);
        let mut world = matchlab_core::world::World::new(SimRng::from_seed(42));
        let id1 = world.next_player_id();
        let id2 = world.next_player_id();
        let reality1 = matchlab_core::player::PlayerReality {
            id: id1,
            skill: matchlab_core::player::SkillVector::one_dimensional(1000.0),
            skill_volatility: 0.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::NA,
            location: GeoLocation::new(Region::NA, 40.0, -74.0),
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "test".to_string(),
            role: None,
        };
        let reality2 = matchlab_core::player::PlayerReality {
            id: id2,
            skill: matchlab_core::player::SkillVector::one_dimensional(1000.0),
            skill_volatility: 0.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::EU,
            location: GeoLocation::new(Region::EU, 51.0, 0.0),
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "test".to_string(),
            role: None,
        };
        let obs1 = matchlab_core::player::PlayerObservation {
            id: id1,
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: matchlab_core::player::VisibleRank {
                tier: "gold".to_string(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: std::collections::VecDeque::new(),
            quit_history: std::collections::VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            role: None,
            skill_vector: matchlab_core::player::SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
        };
        let obs2 = matchlab_core::player::PlayerObservation {
            id: id2,
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: matchlab_core::player::VisibleRank {
                tier: "gold".to_string(),
                division: 1,
            },
            rating_deviation: 350.0,
            volatility: 0.06,
            games_played: 0,
            win_rate: 0.5,
            recent_performances: Vec::new(),
            queue_joined_at: None,
            is_online: true,
            party_id: None,
            session_history: std::collections::VecDeque::new(),
            quit_history: std::collections::VecDeque::new(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            role: None,
            skill_vector: matchlab_core::player::SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
        };
        world.add_player(reality1, obs1);
        world.add_player(reality2, obs2);
        let reference = GeoLocation::new(Region::NA, 40.0, -74.0);
        let stats = model.team_latency(&[id1, id2], &reference, &world);
        assert!(stats.mean_latency > 0.0, "mean latency should be positive");
        assert!(stats.max_latency >= stats.mean_latency);
    }
}
