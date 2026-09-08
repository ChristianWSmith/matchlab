//! Experiment identity and run metadata: first-class identity
//! for executed experiments with explicit distinction between study, experiment,
//! replication, and run.
use serde::{Deserialize, Serialize};
/// Unique identifier for a study (the research question).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StudyId(pub String);
/// Unique identifier for an experiment (one arm/condition within a study).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExperimentId(pub String);
/// Unique identifier for a replication (one run of a condition).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplicationId(pub String);
/// Unique identifier for a run (one execution attempt).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub String);
/// Status of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String },
    Cancelled,
    Checkpointed,
}
/// Metadata for a single run execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    pub run_id: RunId,
    pub replication_id: ReplicationId,
    pub experiment_id: ExperimentId,
    pub study_id: StudyId,
    pub parent_run: Option<RunId>,
    pub config_hash: String,
    pub git_commit: String,
    pub engine_version: String,
    pub seed: u64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: RunStatus,
}
/// Metadata for a study.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyMetadata {
    pub id: StudyId,
    pub name: String,
    pub description: String,
    pub config_hash: String,
    pub git_commit: String,
    pub engine_version: String,
}
/// Metadata for an experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentMetadata {
    pub id: ExperimentId,
    pub study_id: StudyId,
    pub name: String,
    pub config: String,
}
/// Metadata for a replication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationMetadata {
    pub id: ReplicationId,
    pub experiment_id: ExperimentId,
    pub index: u64,
    pub seed: u64,
}
/// Builder for constructing RunMetadata.
pub struct RunMetadataBuilder {
    run_id: RunId,
    replication_id: ReplicationId,
    experiment_id: ExperimentId,
    study_id: StudyId,
    parent_run: Option<RunId>,
    config_hash: String,
    git_commit: String,
    engine_version: String,
    seed: u64,
}
impl RunMetadataBuilder {
    pub fn config_hash(mut self, hash: String) -> Self {
        self.config_hash = hash;
        self
    }
    pub fn git_commit(mut self, commit: String) -> Self {
        self.git_commit = commit;
        self
    }
    pub fn engine_version(mut self, version: String) -> Self {
        self.engine_version = version;
        self
    }
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
    pub fn parent_run(mut self, parent: RunId) -> Self {
        self.parent_run = Some(parent);
        self
    }
    pub fn build(self) -> RunMetadata {
        RunMetadata {
            run_id: self.run_id,
            replication_id: self.replication_id,
            experiment_id: self.experiment_id,
            study_id: self.study_id,
            parent_run: self.parent_run,
            config_hash: self.config_hash,
            git_commit: self.git_commit,
            engine_version: self.engine_version,
            seed: self.seed,
            started_at: None,
            finished_at: None,
            status: RunStatus::Pending,
        }
    }
}
impl RunMetadata {
    /// Create a new run metadata with pending status.
    pub fn builder(
        run_id: RunId,
        replication_id: ReplicationId,
        experiment_id: ExperimentId,
        study_id: StudyId,
    ) -> RunMetadataBuilder {
        RunMetadataBuilder {
            run_id,
            replication_id,
            experiment_id,
            study_id,
            parent_run: None,
            config_hash: String::new(),
            git_commit: String::new(),
            engine_version: String::new(),
            seed: 0,
        }
    }
    /// Transition to running status.
    pub fn start(&mut self) {
        self.status = RunStatus::Running;
        self.started_at = Some(chrono_now());
    }
    /// Transition to completed status.
    pub fn complete(&mut self) {
        self.status = RunStatus::Completed;
        self.finished_at = Some(chrono_now());
    }
    /// Transition to failed status.
    pub fn fail(&mut self, error: String) {
        self.status = RunStatus::Failed { error };
        self.finished_at = Some(chrono_now());
    }
    /// Transition to cancelled status.
    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
        self.finished_at = Some(chrono_now());
    }
    /// Transition to checkpointed status.
    pub fn checkpoint(&mut self) {
        self.status = RunStatus::Checkpointed;
    }
}
/// Simple timestamp string (placeholder for a real time library).
fn chrono_now() -> String {
    "2024-01-01T00:00:00Z".to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_metadata_new_has_pending_status() {
        let meta = RunMetadata::builder(
            RunId("run-1".to_string()),
            ReplicationId("rep-1".to_string()),
            ExperimentId("exp-1".to_string()),
            StudyId("study-1".to_string()),
        )
        .config_hash("hash".to_string())
        .git_commit("abc".to_string())
        .engine_version("0.1.0".to_string())
        .seed(42)
        .build();
        assert_eq!(meta.status, RunStatus::Pending);
        assert!(meta.started_at.is_none());
    }
    #[test]
    fn run_metadata_status_transitions() {
        let mut meta = RunMetadata::builder(
            RunId("r1".to_string()),
            ReplicationId("rp1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .build();
        meta.start();
        assert_eq!(meta.status, RunStatus::Running);
        assert!(meta.started_at.is_some());
        meta.complete();
        assert_eq!(meta.status, RunStatus::Completed);
        assert!(meta.finished_at.is_some());
    }
    #[test]
    fn run_metadata_fail_transition() {
        let mut meta = RunMetadata::builder(
            RunId("r1".to_string()),
            ReplicationId("rp1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .build();
        meta.start();
        meta.fail("out of memory".to_string());
        assert!(matches!(meta.status, RunStatus::Failed { .. }));
    }
    #[test]
    fn run_metadata_cancel_transition() {
        let mut meta = RunMetadata::builder(
            RunId("r1".to_string()),
            ReplicationId("rp1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .build();
        meta.start();
        meta.cancel();
        assert_eq!(meta.status, RunStatus::Cancelled);
    }
    #[test]
    fn run_metadata_round_trips_json() {
        let meta = RunMetadata::builder(
            RunId("r1".to_string()),
            ReplicationId("rp1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .seed(42)
        .build();
        let json = serde_json::to_string(&meta).unwrap();
        let back: RunMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.run_id, back.run_id);
        assert_eq!(meta.status, back.status);
    }
    #[test]
    fn study_metadata_round_trips() {
        let meta = StudyMetadata {
            id: StudyId("s1".to_string()),
            name: "test".to_string(),
            description: "desc".to_string(),
            config_hash: "h".to_string(),
            git_commit: "g".to_string(),
            engine_version: "0.1.0".to_string(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: StudyMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.id, back.id);
    }
}
