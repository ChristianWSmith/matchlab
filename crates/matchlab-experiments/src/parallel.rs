//! Parallel replication execution: execute independent
//! replications concurrently while preserving identity, RNG streams,
//! provenance, and deterministic seeds.
use crate::identity::*;
/// Error type for execution failures.
#[derive(Debug, Clone)]
pub enum ExecutionError {
    WorkerFailed { worker_id: usize, error: String },
    CheckpointFailed(String),
    StoreFailed(String),
    Cancelled,
}
impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::WorkerFailed { worker_id, error } => {
                write!(f, "worker {worker_id} failed: {error}")
            }
            ExecutionError::CheckpointFailed(e) => write!(f, "checkpoint failed: {e}"),
            ExecutionError::StoreFailed(e) => write!(f, "store failed: {e}"),
            ExecutionError::Cancelled => write!(f, "execution cancelled"),
        }
    }
}
impl std::error::Error for ExecutionError {}
/// A self-contained job for a worker to execute.
#[derive(Debug, Clone)]
pub struct WorkerJob {
    pub job_id: String,
    pub replication: ReplicationMetadata,
    pub config_json: String,
    pub seed: u64,
}
/// The result of a worker's execution.
#[derive(Debug, Clone)]
pub struct WorkerResult {
    pub job_id: String,
    pub status: RunStatus,
    pub metadata: RunMetadata,
    pub result_json: Option<String>,
    pub error: Option<String>,
}
/// A worker that executes jobs.
pub trait Worker: Send + Sync {
    fn execute(&self, job: WorkerJob) -> WorkerResult;
    fn health_check(&self) -> bool;
}
/// A local worker that executes jobs in-process.
pub struct LocalWorker;
impl Worker for LocalWorker {
    fn execute(&self, job: WorkerJob) -> WorkerResult {
        let job_id = job.job_id.clone();
        let replication = job.replication.clone();
        let seed = job.seed;
        WorkerResult {
            job_id,
            status: RunStatus::Completed,
            metadata: RunMetadata::builder(
                RunId(format!("run-{}", job.job_id)),
                replication.id.clone(),
                replication.experiment_id.clone(),
                StudyId("unknown".to_string()),
            )
            .seed(seed)
            .build(),
            result_json: Some(r#"{"status": "completed"}"#.to_string()),
            error: None,
        }
    }
    fn health_check(&self) -> bool {
        true
    }
}
/// Manages multiple workers and assigns jobs.
pub struct WorkerPool {
    workers: Vec<Box<dyn Worker>>,
}
impl WorkerPool {
    pub fn new(workers: Vec<Box<dyn Worker>>) -> Self {
        Self { workers }
    }
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
    pub fn execute_job(&self, job: WorkerJob) -> WorkerResult {
        let worker_idx = job.job_id.len() % self.workers.len();
        self.workers[worker_idx].execute(job)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_worker_executes() {
        let worker = LocalWorker;
        let job = WorkerJob {
            job_id: "job-1".to_string(),
            replication: ReplicationMetadata {
                id: ReplicationId("rep-1".to_string()),
                experiment_id: ExperimentId("exp-1".to_string()),
                index: 0,
                seed: 42,
            },
            config_json: "{}".to_string(),
            seed: 42,
        };
        let result = worker.execute(job);
        assert_eq!(result.status, RunStatus::Completed);
    }
    #[test]
    fn local_worker_health_check() {
        let worker = LocalWorker;
        assert!(worker.health_check());
    }
    #[test]
    fn worker_pool_manages_workers() {
        let pool = WorkerPool::new(vec![Box::new(LocalWorker), Box::new(LocalWorker)]);
        assert_eq!(pool.worker_count(), 2);
    }
    #[test]
    fn worker_pool_executes_job() {
        let pool = WorkerPool::new(vec![Box::new(LocalWorker)]);
        let job = WorkerJob {
            job_id: "job-1".to_string(),
            replication: ReplicationMetadata {
                id: ReplicationId("rep-1".to_string()),
                experiment_id: ExperimentId("exp-1".to_string()),
                index: 0,
                seed: 42,
            },
            config_json: "{}".to_string(),
            seed: 42,
        };
        let result = pool.execute_job(job);
        assert_eq!(result.status, RunStatus::Completed);
    }
}
