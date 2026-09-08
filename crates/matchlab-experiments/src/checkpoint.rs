//! Checkpoints and resume: checkpointing for long-running
//! simulations, allowing a research run to survive process termination,
//! machine restart, or intentional pause and resume without changing the
//! scientific trajectory.
use crate::identity::RunId;
use crate::store::StoreError;
/// Snapshot of world state for checkpointing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldSnapshot {
    pub player_count: usize,
    pub match_count: usize,
    pub time_secs: f64,
    /// Serialized player states (JSON).
    pub players_json: String,
}
/// Snapshot of queue state for checkpointing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueSnapshot {
    pub entry_count: usize,
    /// Serialized queue entries (JSON).
    pub entries_json: String,
}
/// Snapshot of metrics for checkpointing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsSnapshot {
    /// Serialized metrics map (JSON).
    pub metrics_json: String,
}
/// Progress of a single replication.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplicationProgress {
    pub matches_completed: u64,
    pub simulated_time_secs: f64,
    pub is_complete: bool,
}
/// A checkpoint capturing sufficient simulation state to resume.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub run_id: RunId,
    pub timestamp_secs: f64,
    pub world_state: WorldSnapshot,
    pub queue_state: QueueSnapshot,
    pub metrics_snapshot: MetricsSnapshot,
    /// Serialized RNG state (base64-encoded).
    pub rng_state: String,
    pub replication_progress: std::collections::HashMap<String, ReplicationProgress>,
}
/// Trait for managing checkpoints.
pub trait CheckpointManager: Send + Sync {
    fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), StoreError>;
    fn load_checkpoint(&self, run_id: &RunId) -> Result<Option<Checkpoint>, StoreError>;
    fn list_checkpoints(&self, run_id: &RunId) -> Result<Vec<Checkpoint>, StoreError>;
}
/// File-based checkpoint manager.
pub struct FileCheckpointManager {
    base_dir: String,
}
impl FileCheckpointManager {
    pub fn new(base_dir: &str) -> Self {
        Self {
            base_dir: base_dir.to_string(),
        }
    }
    fn checkpoint_path(&self, run_id: &RunId) -> String {
        format!("{}/{}/checkpoint.json", self.base_dir, run_id.0)
    }
}
impl CheckpointManager for FileCheckpointManager {
    fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), StoreError> {
        let path = self.checkpoint_path(&checkpoint.run_id);
        let dir = std::path::Path::new(&path)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| StoreError::Database(e.to_string()))?;
        let json = serde_json::to_string_pretty(checkpoint)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }
    fn load_checkpoint(&self, run_id: &RunId) -> Result<Option<Checkpoint>, StoreError> {
        let path = self.checkpoint_path(run_id);
        if !std::path::Path::new(&path).exists() {
            return Ok(None);
        }
        let json =
            std::fs::read_to_string(&path).map_err(|e| StoreError::Database(e.to_string()))?;
        let checkpoint: Checkpoint =
            serde_json::from_str(&json).map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(Some(checkpoint))
    }
    fn list_checkpoints(&self, run_id: &RunId) -> Result<Vec<Checkpoint>, StoreError> {
        let path = self.checkpoint_path(run_id);
        if !std::path::Path::new(&path).exists() {
            return Ok(Vec::new());
        }
        let json =
            std::fs::read_to_string(&path).map_err(|e| StoreError::Database(e.to_string()))?;
        let checkpoint: Checkpoint =
            serde_json::from_str(&json).map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(vec![checkpoint])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::*;
    fn test_checkpoint() -> Checkpoint {
        let mut progress = std::collections::HashMap::new();
        progress.insert(
            "rep-1".to_string(),
            ReplicationProgress {
                matches_completed: 100,
                simulated_time_secs: 3600.0,
                is_complete: false,
            },
        );
        Checkpoint {
            run_id: RunId("run-1".to_string()),
            timestamp_secs: 3600.0,
            world_state: WorldSnapshot {
                player_count: 1000,
                match_count: 5000,
                time_secs: 3600.0,
                players_json: "[]".to_string(),
            },
            queue_state: QueueSnapshot {
                entry_count: 50,
                entries_json: "[]".to_string(),
            },
            metrics_snapshot: MetricsSnapshot {
                metrics_json: "{}".to_string(),
            },
            rng_state: "dGVzdA==".to_string(),
            replication_progress: progress,
        }
    }
    #[test]
    fn checkpoint_round_trips_json() {
        let cp = test_checkpoint();
        let json = serde_json::to_string(&cp).unwrap();
        let back: Checkpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(cp.run_id, back.run_id);
        assert_eq!(cp.timestamp_secs, back.timestamp_secs);
        assert_eq!(cp.world_state.player_count, back.world_state.player_count);
    }
    #[test]
    fn file_checkpoint_manager_save_and_load() {
        let dir = std::env::temp_dir().join("matchlab_test_cp");
        let _ = std::fs::remove_dir_all(&dir);
        let manager = FileCheckpointManager::new(dir.to_str().unwrap());
        let cp = test_checkpoint();
        manager.save_checkpoint(&cp).unwrap();
        let loaded = manager
            .load_checkpoint(&RunId("run-1".to_string()))
            .unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().run_id, cp.run_id);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn file_checkpoint_manager_load_nonexistent() {
        let dir = std::env::temp_dir().join("matchlab_test_cp2");
        let _ = std::fs::remove_dir_all(&dir);
        let manager = FileCheckpointManager::new(dir.to_str().unwrap());
        let loaded = manager
            .load_checkpoint(&RunId("nonexistent".to_string()))
            .unwrap();
        assert!(loaded.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
