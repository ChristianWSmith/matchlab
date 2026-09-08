//! Experiment store: persistent storage for experiments, runs,
//! and results backed by SQLite.
use crate::identity::*;
/// Error type for store operations.
#[derive(Debug, Clone)]
pub enum StoreError {
    Database(String),
    NotFound,
    AlreadyExists,
    Serialization(String),
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Database(e) => write!(f, "database error: {e}"),
            StoreError::NotFound => write!(f, "not found"),
            StoreError::AlreadyExists => write!(f, "already exists"),
            StoreError::Serialization(e) => write!(f, "serialization error: {e}"),
        }
    }
}
impl std::error::Error for StoreError {}
/// Trait for experiment storage (abstract, not tied to SQLite).
pub trait ExperimentStore: Send + Sync {
    fn register_study(&self, study: &StudyMetadata) -> Result<StudyId, StoreError>;
    fn register_experiment(&self, exp: &ExperimentMetadata) -> Result<ExperimentId, StoreError>;
    fn register_replication(&self, rep: &ReplicationMetadata) -> Result<ReplicationId, StoreError>;
    fn register_run(&self, run: &RunMetadata) -> Result<RunId, StoreError>;
    fn update_run_status(&self, run_id: &RunId, status: RunStatus) -> Result<(), StoreError>;
    fn get_run(&self, run_id: &RunId) -> Result<Option<RunMetadata>, StoreError>;
    fn list_runs(&self, experiment_id: &ExperimentId) -> Result<Vec<RunMetadata>, StoreError>;
    fn store_result(&self, run_id: &RunId, result_json: &str) -> Result<(), StoreError>;
    fn get_result(&self, run_id: &RunId) -> Result<Option<String>, StoreError>;
}
/// SQLite-backed experiment store.
pub struct SqliteStore {
    conn: std::sync::Mutex<rusqlite::Connection>,
}
impl SqliteStore {
    /// Open or create a SQLite database at the given path.
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let conn =
            rusqlite::Connection::open(path).map_err(|e| StoreError::Database(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let store = Self {
            conn: std::sync::Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }
    /// Create an in-memory store for testing.
    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let store = Self {
            conn: std::sync::Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }
    fn migrate(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
                CREATE TABLE IF NOT EXISTS studies (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    config_hash TEXT,
                    git_commit TEXT,
                    engine_version TEXT
                );
                CREATE TABLE IF NOT EXISTS experiments (
                    id TEXT PRIMARY KEY,
                    study_id TEXT NOT NULL,
                    name TEXT,
                    config TEXT,
                    FOREIGN KEY (study_id) REFERENCES studies(id)
                );
                CREATE TABLE IF NOT EXISTS replications (
                    id TEXT PRIMARY KEY,
                    experiment_id TEXT NOT NULL,
                    index_num INTEGER,
                    seed INTEGER,
                    FOREIGN KEY (experiment_id) REFERENCES experiments(id)
                );
                CREATE TABLE IF NOT EXISTS runs (
                    id TEXT PRIMARY KEY,
                    replication_id TEXT NOT NULL,
                    experiment_id TEXT NOT NULL,
                    study_id TEXT NOT NULL,
                    parent_run TEXT,
                    config_hash TEXT,
                    git_commit TEXT,
                    engine_version TEXT,
                    seed INTEGER,
                    started_at TEXT,
                    finished_at TEXT,
                    status TEXT NOT NULL,
                    FOREIGN KEY (replication_id) REFERENCES replications(id)
                );
                CREATE TABLE IF NOT EXISTS results (
                    run_id TEXT PRIMARY KEY,
                    result_json TEXT NOT NULL,
                    FOREIGN KEY (run_id) REFERENCES runs(id)
                );
                ",
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }
}
impl ExperimentStore for SqliteStore {
    fn register_study(&self, study: &StudyMetadata) -> Result<StudyId, StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO studies (id, name, description, config_hash, git_commit, engine_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                study.id.0, study.name, study.description, study.config_hash,
                study.git_commit, study.engine_version
            ],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(study.id.clone())
    }
    fn register_experiment(&self, exp: &ExperimentMetadata) -> Result<ExperimentId, StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO experiments (id, study_id, name, config) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![exp.id.0, exp.study_id.0, exp.name, exp.config],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(exp.id.clone())
    }
    fn register_replication(&self, rep: &ReplicationMetadata) -> Result<ReplicationId, StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO replications (id, experiment_id, index_num, seed) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![rep.id.0, rep.experiment_id.0, rep.index, rep.seed],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(rep.id.clone())
    }
    fn register_run(&self, run: &RunMetadata) -> Result<RunId, StoreError> {
        let conn = self.conn.lock().unwrap();
        let status = serde_json::to_string(&run.status).unwrap_or_default();
        conn.execute(
            "INSERT INTO runs (id, replication_id, experiment_id, study_id, parent_run, config_hash, git_commit, engine_version, seed, started_at, finished_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                run.run_id.0, run.replication_id.0, run.experiment_id.0,
                run.study_id.0, run.parent_run.as_ref().map(|r| r.0.clone()),
                run.config_hash, run.git_commit, run.engine_version, run.seed,
                run.started_at, run.finished_at, status
            ],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(run.run_id.clone())
    }
    fn update_run_status(&self, run_id: &RunId, status: RunStatus) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let status_json = serde_json::to_string(&status).unwrap_or_default();
        let now = "2024-01-01T00:00:00Z".to_string();
        let finished = match &status {
            RunStatus::Completed | RunStatus::Failed { .. } | RunStatus::Cancelled => Some(&now),
            _ => None,
        };
        conn.execute(
            "UPDATE runs SET status = ?1, finished_at = ?2 WHERE id = ?3",
            rusqlite::params![status_json, finished, run_id.0],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }
    fn get_run(&self, run_id: &RunId) -> Result<Option<RunMetadata>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT id, replication_id, experiment_id, study_id, parent_run, config_hash, git_commit, engine_version, seed, started_at, finished_at, status FROM runs WHERE id = ?1",
                rusqlite::params![run_id.0],
                |row| {
                    let status_str: String = row.get(11)?;
                    let status: RunStatus =
                        serde_json::from_str(&status_str).unwrap_or(RunStatus::Pending);
                    Ok(RunMetadata {
                        run_id: RunId(row.get(0)?),
                        replication_id: ReplicationId(row.get(1)?),
                        experiment_id: ExperimentId(row.get(2)?),
                        study_id: StudyId(row.get(3)?),
                        parent_run: row
                            .get::<_, Option<String>>(4)?
                            .map(RunId),
                        config_hash: row.get(5)?,
                        git_commit: row.get(6)?,
                        engine_version: row.get(7)?,
                        seed: row.get(8)?,
                        started_at: row.get(9)?,
                        finished_at: row.get(10)?,
                        status,
                    })
                },
            )
            .ok();
        Ok(result)
    }
    fn list_runs(&self, experiment_id: &ExperimentId) -> Result<Vec<RunMetadata>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, replication_id, experiment_id, study_id, parent_run, config_hash, git_commit, engine_version, seed, started_at, finished_at, status FROM runs WHERE experiment_id = ?1",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![experiment_id.0], |row| {
                let status_str: String = row.get(11)?;
                let status: RunStatus =
                    serde_json::from_str(&status_str).unwrap_or(RunStatus::Pending);
                Ok(RunMetadata {
                    run_id: RunId(row.get(0)?),
                    replication_id: ReplicationId(row.get(1)?),
                    experiment_id: ExperimentId(row.get(2)?),
                    study_id: StudyId(row.get(3)?),
                    parent_run: row.get::<_, Option<String>>(4)?.map(RunId),
                    config_hash: row.get(5)?,
                    git_commit: row.get(6)?,
                    engine_version: row.get(7)?,
                    seed: row.get(8)?,
                    started_at: row.get(9)?,
                    finished_at: row.get(10)?,
                    status,
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(row.map_err(|e| StoreError::Database(e.to_string()))?);
        }
        Ok(runs)
    }
    fn store_result(&self, run_id: &RunId, result_json: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO results (run_id, result_json) VALUES (?1, ?2)",
            rusqlite::params![run_id.0, result_json],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }
    fn get_result(&self, run_id: &RunId) -> Result<Option<String>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT result_json FROM results WHERE run_id = ?1",
                rusqlite::params![run_id.0],
                |row| row.get::<_, String>(0),
            )
            .ok();
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn test_store() -> SqliteStore {
        SqliteStore::in_memory().expect("failed to create in-memory store")
    }
    #[test]
    fn register_and_retrieve_study() {
        let store = test_store();
        let study = StudyMetadata {
            id: StudyId("s1".to_string()),
            name: "test study".to_string(),
            description: "desc".to_string(),
            config_hash: "h".to_string(),
            git_commit: "g".to_string(),
            engine_version: "0.1.0".to_string(),
        };
        let id = store.register_study(&study).unwrap();
        assert_eq!(id, StudyId("s1".to_string()));
    }
    #[test]
    fn register_and_retrieve_run() {
        let store = test_store();
        let study = StudyMetadata {
            id: StudyId("s1".to_string()),
            name: "test".to_string(),
            description: "".to_string(),
            config_hash: "h".to_string(),
            git_commit: "g".to_string(),
            engine_version: "0.1.0".to_string(),
        };
        store.register_study(&study).unwrap();
        let exp = ExperimentMetadata {
            id: ExperimentId("e1".to_string()),
            study_id: StudyId("s1".to_string()),
            name: "exp1".to_string(),
            config: "{}".to_string(),
        };
        store.register_experiment(&exp).unwrap();
        let rep = ReplicationMetadata {
            id: ReplicationId("r1".to_string()),
            experiment_id: ExperimentId("e1".to_string()),
            index: 0,
            seed: 42,
        };
        store.register_replication(&rep).unwrap();
        let run = RunMetadata::builder(
            RunId("run1".to_string()),
            ReplicationId("r1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .seed(42)
        .build();
        store.register_run(&run).unwrap();
        let retrieved = store.get_run(&RunId("run1".to_string())).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().seed, 42);
    }
    #[test]
    fn update_run_status() {
        let store = test_store();
        let study = StudyMetadata {
            id: StudyId("s1".to_string()),
            name: "test".to_string(),
            description: "".to_string(),
            config_hash: "h".to_string(),
            git_commit: "g".to_string(),
            engine_version: "0.1.0".to_string(),
        };
        store.register_study(&study).unwrap();
        let exp = ExperimentMetadata {
            id: ExperimentId("e1".to_string()),
            study_id: StudyId("s1".to_string()),
            name: "exp1".to_string(),
            config: "{}".to_string(),
        };
        store.register_experiment(&exp).unwrap();
        let rep = ReplicationMetadata {
            id: ReplicationId("r1".to_string()),
            experiment_id: ExperimentId("e1".to_string()),
            index: 0,
            seed: 42,
        };
        store.register_replication(&rep).unwrap();
        let run = RunMetadata::builder(
            RunId("run1".to_string()),
            ReplicationId("r1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .build();
        store.register_run(&run).unwrap();
        store
            .update_run_status(&RunId("run1".to_string()), RunStatus::Completed)
            .unwrap();
        let retrieved = store.get_run(&RunId("run1".to_string())).unwrap().unwrap();
        assert_eq!(retrieved.status, RunStatus::Completed);
    }
    #[test]
    fn store_and_retrieve_result() {
        let store = test_store();
        let study = StudyMetadata {
            id: StudyId("s1".to_string()),
            name: "test".to_string(),
            description: "".to_string(),
            config_hash: "h".to_string(),
            git_commit: "g".to_string(),
            engine_version: "0.1.0".to_string(),
        };
        store.register_study(&study).unwrap();
        let exp = ExperimentMetadata {
            id: ExperimentId("e1".to_string()),
            study_id: StudyId("s1".to_string()),
            name: "exp1".to_string(),
            config: "{}".to_string(),
        };
        store.register_experiment(&exp).unwrap();
        let rep = ReplicationMetadata {
            id: ReplicationId("r1".to_string()),
            experiment_id: ExperimentId("e1".to_string()),
            index: 0,
            seed: 42,
        };
        store.register_replication(&rep).unwrap();
        let run = RunMetadata::builder(
            RunId("run1".to_string()),
            ReplicationId("r1".to_string()),
            ExperimentId("e1".to_string()),
            StudyId("s1".to_string()),
        )
        .build();
        store.register_run(&run).unwrap();
        store
            .store_result(&RunId("run1".to_string()), r#"{"matches": 100}"#)
            .unwrap();
        let result = store.get_result(&RunId("run1".to_string())).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("100"));
    }
}
