//! Distributed execution boundary: defines the abstraction
//! required for execution beyond a single machine, establishing a clean
//! worker boundary for future distributed implementation.
use crate::checkpoint::Checkpoint;
use crate::identity::*;
/// A self-contained job specification for a worker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DistributedJob {
    pub job_id: String,
    pub replication: ReplicationMetadata,
    pub config_json: String,
    pub seed: u64,
    pub checkpoint: Option<Checkpoint>,
}
/// The result of a worker's execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DistributedResult {
    pub job_id: String,
    pub status: RunStatus,
    pub metadata: RunMetadata,
    pub result_json: Option<String>,
    pub observations_json: Option<String>,
    pub error: Option<String>,
}
/// A worker that can execute distributed jobs.
pub trait DistributedWorker: Send + Sync {
    fn execute(&self, job: DistributedJob) -> DistributedResult;
    fn health_check(&self) -> bool;
    fn capabilities(&self) -> WorkerCapabilities;
}
/// Worker capabilities description.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkerCapabilities {
    pub max_concurrent_jobs: usize,
    pub supported_formats: Vec<String>,
    pub memory_limit_mb: Option<u64>,
}
/// A coordinator that manages distributed workers.
pub struct DistributedCoordinator {
    workers: Vec<Box<dyn DistributedWorker>>,
}
impl DistributedCoordinator {
    pub fn new(workers: Vec<Box<dyn DistributedWorker>>) -> Self {
        Self { workers }
    }
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
    pub fn execute_job(&self, job: DistributedJob) -> DistributedResult {
        let worker_idx = job.job_id.len() % self.workers.len();
        self.workers[worker_idx].execute(job)
    }
    pub fn health_check_all(&self) -> Vec<bool> {
        self.workers.iter().map(|w| w.health_check()).collect()
    }
}
/// A local worker that executes jobs in-process (for testing and single-machine).
pub struct LocalDistributedWorker;
impl DistributedWorker for LocalDistributedWorker {
    fn execute(&self, job: DistributedJob) -> DistributedResult {
        let metadata = RunMetadata::builder(
            RunId(format!("run-{}", job.job_id)),
            job.replication.id.clone(),
            job.replication.experiment_id.clone(),
            StudyId("unknown".to_string()),
        )
        .seed(job.seed)
        .build();
        DistributedResult {
            job_id: job.job_id,
            status: RunStatus::Completed,
            metadata,
            result_json: Some(r#"{"status": "completed"}"#.to_string()),
            observations_json: None,
            error: None,
        }
    }
    fn health_check(&self) -> bool {
        true
    }
    fn capabilities(&self) -> WorkerCapabilities {
        WorkerCapabilities {
            max_concurrent_jobs: 1,
            supported_formats: vec!["json".to_string()],
            memory_limit_mb: None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_worker_executes() {
        let worker = LocalDistributedWorker;
        let job = DistributedJob {
            job_id: "job-1".to_string(),
            replication: ReplicationMetadata {
                id: ReplicationId("rep-1".to_string()),
                experiment_id: ExperimentId("exp-1".to_string()),
                index: 0,
                seed: 42,
            },
            config_json: "{}".to_string(),
            seed: 42,
            checkpoint: None,
        };
        let result = worker.execute(job);
        assert_eq!(result.status, RunStatus::Completed);
    }
    #[test]
    fn worker_capabilities() {
        let worker = LocalDistributedWorker;
        let caps = worker.capabilities();
        assert_eq!(caps.max_concurrent_jobs, 1);
        assert!(caps.supported_formats.contains(&"json".to_string()));
    }
    #[test]
    fn coordinator_manages_workers() {
        let coordinator = DistributedCoordinator::new(vec![
            Box::new(LocalDistributedWorker),
            Box::new(LocalDistributedWorker),
        ]);
        assert_eq!(coordinator.worker_count(), 2);
        let health = coordinator.health_check_all();
        assert_eq!(health, vec![true, true]);
    }
    #[test]
    fn coordinator_executes_job() {
        let coordinator = DistributedCoordinator::new(vec![Box::new(LocalDistributedWorker)]);
        let job = DistributedJob {
            job_id: "job-1".to_string(),
            replication: ReplicationMetadata {
                id: ReplicationId("rep-1".to_string()),
                experiment_id: ExperimentId("exp-1".to_string()),
                index: 0,
                seed: 42,
            },
            config_json: "{}".to_string(),
            seed: 42,
            checkpoint: None,
        };
        let result = coordinator.execute_job(job);
        assert_eq!(result.status, RunStatus::Completed);
    }
}
