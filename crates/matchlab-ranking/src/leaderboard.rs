use matchlab_core::player::PlayerId;

use crate::ranker::Rank;

pub struct Leaderboard {
    entries: Vec<LeaderboardEntry>,
}

pub struct LeaderboardEntry {
    pub player_id: PlayerId,
    pub rating: f64,
    pub rank: Rank,
    pub games_played: u64,
}

impl Leaderboard {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn update(
        &mut self,
        player_id: PlayerId,
        rating: f64,
        rank: Rank,
        games_played: u64,
    ) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.player_id == player_id) {
            entry.rating = rating;
            entry.rank = rank;
            entry.games_played = games_played;
        } else {
            self.entries.push(LeaderboardEntry {
                player_id,
                rating,
                rank,
                games_played,
            });
        }
        self.entries
            .sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn rank_of(&self, player_id: PlayerId) -> Option<usize> {
        self.entries.iter().position(|e| e.player_id == player_id)
    }

    pub fn top_n(&self, n: usize) -> &[LeaderboardEntry] {
        &self.entries[..n.min(self.entries.len())]
    }

    pub fn entries(&self) -> &[LeaderboardEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Leaderboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(tier: &str) -> Rank {
        Rank {
            tier: tier.to_string(),
            division: 1,
        }
    }

    #[test]
    fn update_inserts_new_entries_and_sorts_descending() {
        let mut lb = Leaderboard::new();
        lb.update(PlayerId(1), 1000.0, rank("silver"), 10);
        lb.update(PlayerId(2), 1500.0, rank("platinum"), 20);
        lb.update(PlayerId(3), 800.0, rank("bronze"), 5);

        assert_eq!(lb.len(), 3);
        assert_eq!(lb.entries()[0].player_id, PlayerId(2));
        assert_eq!(lb.entries()[1].player_id, PlayerId(1));
        assert_eq!(lb.entries()[2].player_id, PlayerId(3));
    }

    #[test]
    fn update_replaces_existing_entry_and_resorts() {
        let mut lb = Leaderboard::new();
        lb.update(PlayerId(1), 1000.0, rank("silver"), 10);
        lb.update(PlayerId(1), 1600.0, rank("platinum"), 11);

        assert_eq!(lb.len(), 1);
        assert_eq!(lb.entries()[0].rating, 1600.0);
        assert_eq!(lb.entries()[0].rank.tier, "platinum");
        assert_eq!(lb.entries()[0].games_played, 11);
    }

    #[test]
    fn rank_of_returns_position() {
        let mut lb = Leaderboard::new();
        lb.update(PlayerId(1), 1000.0, rank("silver"), 10);
        lb.update(PlayerId(2), 1500.0, rank("platinum"), 20);
        lb.update(PlayerId(3), 800.0, rank("bronze"), 5);

        assert_eq!(lb.rank_of(PlayerId(2)), Some(0));
        assert_eq!(lb.rank_of(PlayerId(1)), Some(1));
        assert_eq!(lb.rank_of(PlayerId(3)), Some(2));
        assert_eq!(lb.rank_of(PlayerId(999)), None);
    }

    #[test]
    fn top_n_returns_correct_slice() {
        let mut lb = Leaderboard::new();
        for i in 1..=10u64 {
            lb.update(PlayerId(i), i as f64 * 100.0, rank("silver"), 1);
        }

        let top = lb.top_n(3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].player_id, PlayerId(10));
        assert_eq!(top[2].player_id, PlayerId(8));
    }

    #[test]
    fn top_n_clamps_when_n_exceeds_len() {
        let mut lb = Leaderboard::new();
        lb.update(PlayerId(1), 1000.0, rank("silver"), 1);
        lb.update(PlayerId(2), 1500.0, rank("platinum"), 1);

        let top = lb.top_n(10);
        assert_eq!(top.len(), 2);
    }

    #[test]
    fn empty_leaderboard_is_empty() {
        let lb = Leaderboard::new();
        assert!(lb.is_empty());
        assert_eq!(lb.len(), 0);
        assert!(lb.top_n(5).is_empty());
    }
}