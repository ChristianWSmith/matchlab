//! Raw event and observation storage: configurable persistence
//! for simulation observations at different levels of detail.
use crate::identity::ReplicationId;
/// The level of detail to persist for simulation observations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObservationPolicy {
    /// Only store final aggregate metrics.
    AggregateOnly,
    /// Store one record per replication.
    ReplicationLevel,
    /// Store one record per match.
    MatchLevel,
    /// Store one record per player-state observation.
    PlayerLevel,
    /// Complete simulation event stream.
    EventLevel,
}
/// A trait for writing observations to persistent storage.
pub trait ObservationWriter: Send + Sync {
    fn write_match_record(
        &self,
        replication_id: &ReplicationId,
        match_json: &str,
    ) -> Result<(), std::io::Error>;
    fn write_player_state(
        &self,
        replication_id: &ReplicationId,
        player_json: &str,
    ) -> Result<(), std::io::Error>;
    fn write_event(
        &self,
        replication_id: &ReplicationId,
        event_json: &str,
    ) -> Result<(), std::io::Error>;
    fn flush(&self) -> Result<(), std::io::Error>;
}
/// Null writer: discards all observations (aggregate-only mode).
pub struct NullObservationWriter;
impl ObservationWriter for NullObservationWriter {
    fn write_match_record(
        &self,
        _replication_id: &ReplicationId,
        _match_json: &str,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
    fn write_player_state(
        &self,
        _replication_id: &ReplicationId,
        _player_json: &str,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
    fn write_event(
        &self,
        _replication_id: &ReplicationId,
        _event_json: &str,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
    fn flush(&self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
/// File-based writer: writes observations as JSONL files.
pub struct FileObservationWriter {
    base_dir: String,
}
impl FileObservationWriter {
    pub fn new(base_dir: &str) -> Self {
        Self {
            base_dir: base_dir.to_string(),
        }
    }
    fn write_line(
        &self,
        subdir: &str,
        replication_id: &ReplicationId,
        json: &str,
    ) -> Result<(), std::io::Error> {
        let dir = format!("{}/{}/{}", self.base_dir, subdir, replication_id.0);
        std::fs::create_dir_all(&dir)?;
        let path = format!("{}/observations.jsonl", dir);
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }
}
impl ObservationWriter for FileObservationWriter {
    fn write_match_record(
        &self,
        replication_id: &ReplicationId,
        match_json: &str,
    ) -> Result<(), std::io::Error> {
        self.write_line("matches", replication_id, match_json)
    }
    fn write_player_state(
        &self,
        replication_id: &ReplicationId,
        player_json: &str,
    ) -> Result<(), std::io::Error> {
        self.write_line("players", replication_id, player_json)
    }
    fn write_event(
        &self,
        replication_id: &ReplicationId,
        event_json: &str,
    ) -> Result<(), std::io::Error> {
        self.write_line("events", replication_id, event_json)
    }
    fn flush(&self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
/// Buffered writer: batches observations in memory before flushing.
pub struct BufferedObservationWriter {
    inner: std::sync::Mutex<Box<dyn ObservationWriter>>,
    buffer: std::sync::Mutex<Vec<(ReplicationId, String, String)>>,
    buffer_size: usize,
}
impl BufferedObservationWriter {
    pub fn new(inner: Box<dyn ObservationWriter>, buffer_size: usize) -> Self {
        Self {
            inner: std::sync::Mutex::new(inner),
            buffer: std::sync::Mutex::new(Vec::new()),
            buffer_size,
        }
    }
    fn add_and_maybe_flush(
        &self,
        kind: &str,
        replication_id: ReplicationId,
        json: String,
    ) -> Result<(), std::io::Error> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push((replication_id, kind.to_string(), json));
        if buffer.len() >= self.buffer_size {
            let entries: Vec<(ReplicationId, String, String)> = buffer.drain(..).collect();
            let inner = self.inner.lock().unwrap();
            for (rid, kind, data) in entries {
                match kind.as_str() {
                    "match" => inner.write_match_record(&rid, &data)?,
                    "player" => inner.write_player_state(&rid, &data)?,
                    "event" => inner.write_event(&rid, &data)?,
                    _ => {}
                }
            }
            inner.flush()?;
        }
        Ok(())
    }
}
impl ObservationWriter for BufferedObservationWriter {
    fn write_match_record(
        &self,
        replication_id: &ReplicationId,
        match_json: &str,
    ) -> Result<(), std::io::Error> {
        self.add_and_maybe_flush("match", replication_id.clone(), match_json.to_string())
    }
    fn write_player_state(
        &self,
        replication_id: &ReplicationId,
        player_json: &str,
    ) -> Result<(), std::io::Error> {
        self.add_and_maybe_flush("player", replication_id.clone(), player_json.to_string())
    }
    fn write_event(
        &self,
        replication_id: &ReplicationId,
        event_json: &str,
    ) -> Result<(), std::io::Error> {
        self.add_and_maybe_flush("event", replication_id.clone(), event_json.to_string())
    }
    fn flush(&self) -> Result<(), std::io::Error> {
        let mut buffer = self.buffer.lock().unwrap();
        if !buffer.is_empty() {
            let entries: Vec<(ReplicationId, String, String)> = buffer.drain(..).collect();
            let inner = self.inner.lock().unwrap();
            for (rid, kind, data) in entries {
                match kind.as_str() {
                    "match" => inner.write_match_record(&rid, &data)?,
                    "player" => inner.write_player_state(&rid, &data)?,
                    "event" => inner.write_event(&rid, &data)?,
                    _ => {}
                }
            }
            inner.flush()?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_writer_discards_everything() {
        let writer = NullObservationWriter;
        let rid = ReplicationId("r1".to_string());
        writer.write_match_record(&rid, "{}").unwrap();
        writer.write_player_state(&rid, "{}").unwrap();
        writer.write_event(&rid, "{}").unwrap();
        writer.flush().unwrap();
    }
    #[test]
    fn observation_policy_serde_round_trip() {
        let policies = vec![
            ObservationPolicy::AggregateOnly,
            ObservationPolicy::ReplicationLevel,
            ObservationPolicy::MatchLevel,
            ObservationPolicy::PlayerLevel,
            ObservationPolicy::EventLevel,
        ];
        for policy in &policies {
            let json = serde_json::to_string(policy).unwrap();
            let back: ObservationPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(*policy, back);
        }
    }
    #[test]
    fn file_writer_creates_directories() {
        let dir = std::env::temp_dir().join("matchlab_test_obs");
        let _ = std::fs::remove_dir_all(&dir);
        let writer = FileObservationWriter::new(dir.to_str().unwrap());
        let rid = ReplicationId("r1".to_string());
        writer
            .write_match_record(&rid, r#"{"test": true}"#)
            .unwrap();
        writer.flush().unwrap();
        let path = dir.join("matches/r1/observations.jsonl");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
