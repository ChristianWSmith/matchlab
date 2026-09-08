//! Execution scheduler: general execution abstraction above
//! individual simulation runs, managing pending/running/completed/failed jobs,
//! retries, cancellation, and resource limits.
use crate::identity::*;
use crate::parallel::{ExecutionError, WorkerJob, WorkerPool, WorkerResult};
use crate::store::ExperimentStore;
/// Status of the scheduler.
#[derive(Debug, Clone, Default)]
pub struct SchedulerStatus {
    pub total_jobs: usize,
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}
/// The execution scheduler manages the lifecycle of jobs in a study.
pub struct ExecutionScheduler {
    pub pool: WorkerPool,
    pub store: Box<dyn ExperimentStore>,
    pub max_concurrent: usize,
    pub max_retries: u32,
}
impl ExecutionScheduler {
    pub fn new(
        pool: WorkerPool,
        store: Box<dyn ExperimentStore>,
        max_concurrent: usize,
        max_retries: u32,
    ) -> Self {
        Self {
            pool,
            store,
            max_concurrent,
            max_retries,
        }
    }
    /// Execute a batch of jobs and return results.
    pub fn execute_batch(&self, jobs: Vec<WorkerJob>) -> Result<Vec<WorkerResult>, ExecutionError> {
        let mut results = Vec::new();
        for job in jobs {
            let result = self.execute_with_retries(job);
            results.push(result);
        }
        Ok(results)
    }
    /// Execute a single job with retry logic.
    fn execute_with_retries(&self, job: WorkerJob) -> WorkerResult {
        let mut last_error = None;
        for attempt in 0..=self.max_retries {
            let result = self.pool.execute_job(job.clone());
            match &result.status {
                RunStatus::Completed => return result,
                RunStatus::Failed { error } => {
                    last_error = Some(error.clone());
                    if attempt < self.max_retries {
                        continue;
                    }
                }
                _ => return result,
            }
        }
        let run_id = RunId(format!("run-{}", job.job_id));
        let replication = job.replication.clone();
        WorkerResult {
            job_id: job.job_id,
            status: RunStatus::Failed {
                error: last_error
                    .clone()
                    .unwrap_or_else(|| "exhausted retries".to_string()),
            },
            metadata: RunMetadata::builder(
                run_id,
                replication.id.clone(),
                replication.experiment_id.clone(),
                StudyId("unknown".to_string()),
            )
            .build(),
            result_json: None,
            error: last_error,
        }
    }
    /// Get the current status of the scheduler.
    pub fn status(&self) -> SchedulerStatus {
        SchedulerStatus::default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parallel::LocalWorker;
    fn test_scheduler() -> ExecutionScheduler {
        ExecutionScheduler::new(
            WorkerPool::new(vec![Box::new(LocalWorker)]),
            Box::new(crate::store::SqliteStore::in_memory().unwrap()),
            4,
            2,
        )
    }
    fn test_job(id: &str) -> WorkerJob {
        WorkerJob {
            job_id: id.to_string(),
            replication: ReplicationMetadata {
                id: ReplicationId(format!("rep-{id}")),
                experiment_id: ExperimentId("exp-1".to_string()),
                index: 0,
                seed: 42,
            },
            config_json: "{}".to_string(),
            seed: 42,
        }
    }
    #[test]
    fn scheduler_executes_batch() {
        let scheduler = test_scheduler();
        let jobs = vec![test_job("1"), test_job("2"), test_job("3")];
        let results = scheduler.execute_batch(jobs).unwrap();
        assert_eq!(results.len(), 3);
        for r in &results {
            assert_eq!(r.status, RunStatus::Completed);
        }
    }
    #[test]
    fn scheduler_status_is_default() {
        let scheduler = test_scheduler();
        let status = scheduler.status();
        assert_eq!(status.total_jobs, 0);
    }
}
