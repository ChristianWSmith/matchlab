use matchlab_core::player::{PlayerId, PlayerObservation, Region};
use matchlab_core::time::SimTime;

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub player_id: PlayerId,
    pub joined_at: SimTime,
    pub observation: PlayerObservation,
    pub region: Region,
    pub party_id: Option<u64>,
    pub game_mode: String,
    pub role: Option<String>,
    pub latency_ms: f64,
}

#[derive(Debug, Default)]
pub struct Queue {
    entries: Vec<QueueEntry>,
}

impl Queue {
    pub fn enqueue(&mut self, entry: QueueEntry) {
        self.entries.push(entry);
    }

    pub fn remove(&mut self, player_id: PlayerId) -> Option<QueueEntry> {
        self.entries
            .iter()
            .position(|e| e.player_id == player_id)
            .map(|pos| self.entries.remove(pos))
    }

    pub fn remove_batch(&mut self, player_ids: &[PlayerId]) -> Vec<QueueEntry> {
        let mut removed = Vec::new();
        for &pid in player_ids {
            if let Some(entry) = self.remove(pid) {
                removed.push(entry);
            }
        }
        removed
    }

    pub fn waiting_time(&self, player_id: PlayerId, now: SimTime) -> Option<SimTime> {
        self.entries
            .iter()
            .find(|e| e.player_id == player_id)
            .map(|e| now.duration_since(e.joined_at))
    }

    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn from_entries(entries: Vec<QueueEntry>) -> Self {
        Self { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{SkillVector, VisibleRank};
    use std::collections::VecDeque;

    fn entry(id: u64, joined_at: SimTime) -> QueueEntry {
        let rating = 1000.0;
        QueueEntry {
            player_id: PlayerId(id),
            joined_at,
            observation: PlayerObservation {
                id: PlayerId(id),
                rating,
                hidden_mmr: rating,
                visible_rank: VisibleRank {
                    tier: "unranked".to_string(),
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
                session_history: VecDeque::new(),
                quit_history: VecDeque::new(),
                tilt_level: 0.0,
                game_mode: "ranked".to_string(),
                skill_vector: SkillVector::one_dimensional(rating),
                detection_flags: Vec::new(),
                role: None,
            },
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".to_string(),
            role: None,
            latency_ms: 30.0,
        }
    }

    #[test]
    fn enqueue_preserves_fifo_order() {
        let mut q = Queue::default();
        q.enqueue(entry(1, SimTime::from_secs(1.0)));
        q.enqueue(entry(2, SimTime::from_secs(2.0)));
        q.enqueue(entry(3, SimTime::from_secs(3.0)));
        assert_eq!(q.len(), 3);
        assert!(!q.is_empty());
        let ids: Vec<u64> = q.entries().iter().map(|e| e.player_id.0).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn waiting_time_measures_since_joined_at_and_advances() {
        let mut q = Queue::default();
        q.enqueue(entry(1, SimTime::from_secs(10.0)));
        assert_eq!(
            q.waiting_time(PlayerId(1), SimTime::from_secs(15.0)),
            Some(SimTime::from_secs(5.0))
        );
        assert_eq!(
            q.waiting_time(PlayerId(1), SimTime::from_secs(30.0)),
            Some(SimTime::from_secs(20.0))
        );
        assert_eq!(q.waiting_time(PlayerId(99), SimTime::from_secs(30.0)), None);
    }

    #[test]
    fn remove_returns_entry_and_shrinks_queue() {
        let mut q = Queue::default();
        q.enqueue(entry(1, SimTime::ZERO));
        q.enqueue(entry(2, SimTime::ZERO));
        q.enqueue(entry(3, SimTime::ZERO));

        let removed = q.remove(PlayerId(2)).expect("player 2 should be present");
        assert_eq!(removed.player_id, PlayerId(2));
        assert_eq!(q.len(), 2);
        let ids: Vec<u64> = q.entries().iter().map(|e| e.player_id.0).collect();
        assert_eq!(ids, vec![1, 3]);

        assert!(q.remove(PlayerId(42)).is_none());
    }

    #[test]
    fn remove_batch_removes_those_present_only() {
        let mut q = Queue::default();
        q.enqueue(entry(1, SimTime::ZERO));
        q.enqueue(entry(2, SimTime::ZERO));
        q.enqueue(entry(3, SimTime::ZERO));

        let removed = q.remove_batch(&[PlayerId(1), PlayerId(3), PlayerId(99)]);
        assert_eq!(removed.len(), 2);
        let ids: Vec<u64> = q.entries().iter().map(|e| e.player_id.0).collect();
        assert_eq!(ids, vec![2]);
    }

    #[test]
    fn from_entries_builds_queue() {
        let q = Queue::from_entries(vec![entry(1, SimTime::ZERO), entry(2, SimTime::ZERO)]);
        assert_eq!(q.len(), 2);
        assert!(!q.is_empty());
    }
}
