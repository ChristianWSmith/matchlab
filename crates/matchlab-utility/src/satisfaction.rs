pub trait SatisfactionModel: Send + Sync {
    /// Compute satisfaction score from experience history.
    fn satisfaction(&self, exp: &PlayerExperience) -> f64;

    /// Probability that the player continues playing next session.
    fn retention_probability(&self, satisfaction: f64) -> f64;

    /// Probability the player requeues for another match (rematch).
    fn rematch_probability(&self, satisfaction: f64) -> f64;
}

pub struct PlayerExperience {
    pub recent_match_qualities: Vec<f64>,
    pub recent_queue_times: Vec<f64>,
    pub recent_outcomes: Vec<bool>,
    pub current_streak: i32,
    pub rank_change: f64,
    pub perceived_fairness: f64,
    /// Fraction of recent matches the player chose to rematch/requeue.
    pub rematch_rate: f64,
}

impl PlayerExperience {
    pub fn new() -> Self {
        Self {
            recent_match_qualities: Vec::new(),
            recent_queue_times: Vec::new(),
            recent_outcomes: Vec::new(),
            current_streak: 0,
            rank_change: 0.0,
            perceived_fairness: 0.5,
            rematch_rate: 0.0,
        }
    }

    pub fn record_match(&mut self, quality: f64, queue_time_secs: f64, won: bool) {
        self.recent_match_qualities.push(quality);
        self.recent_queue_times.push(queue_time_secs);
        self.recent_outcomes.push(won);
        self.current_streak = if won {
            if self.current_streak < 0 {
                1
            } else {
                self.current_streak + 1
            }
        } else if self.current_streak > 0 {
            -1
        } else {
            self.current_streak - 1
        };
    }
}

impl Default for PlayerExperience {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_match_updates_streak() {
        let mut exp = PlayerExperience::new();
        exp.record_match(0.8, 30.0, true);
        exp.record_match(0.9, 25.0, true);
        assert_eq!(exp.current_streak, 2);
        exp.record_match(0.7, 40.0, false);
        assert_eq!(exp.current_streak, -1);
        assert_eq!(exp.recent_outcomes.len(), 3);
    }
}
