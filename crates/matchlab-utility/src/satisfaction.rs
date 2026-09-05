pub struct SatisfactionModel {
    pub weights: SatisfactionWeights,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SatisfactionWeights {
    pub match_quality: f64,
    pub queue_time_penalty: f64,
    pub win_bonus: f64,
    pub loss_streak_penalty: f64,
    pub rank_progression_bonus: f64,
    pub fairness_sensitivity: f64,
    pub rematch_bonus: f64,
}

impl Default for SatisfactionWeights {
    fn default() -> Self {
        Self {
            match_quality: 1.0,
            queue_time_penalty: -0.01,
            win_bonus: 0.5,
            loss_streak_penalty: -0.3,
            rank_progression_bonus: 0.2,
            fairness_sensitivity: -0.8,
            rematch_bonus: 0.1,
        }
    }
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

impl SatisfactionModel {
    pub fn new(weights: SatisfactionWeights) -> Self {
        Self { weights }
    }

    /// Compute satisfaction score from experience history.
    pub fn satisfaction(&self, exp: &PlayerExperience) -> f64 {
        let avg_quality = mean_or(&exp.recent_match_qualities, 0.5);
        let avg_queue = mean_or(&exp.recent_queue_times, 30.0);
        let win_rate = exp.recent_outcomes.iter().filter(|&&w| w).count() as f64
            / exp.recent_outcomes.len().max(1) as f64;
        let streak_penalty = if exp.current_streak < -3 {
            self.weights.loss_streak_penalty * (exp.current_streak.abs() as f64 - 3.0)
        } else {
            0.0
        };

        self.weights.match_quality * avg_quality
            + self.weights.queue_time_penalty * avg_queue
            + self.weights.win_bonus * win_rate
            + streak_penalty
            + self.weights.rank_progression_bonus * exp.rank_change
            + self.weights.fairness_sensitivity * (1.0 - exp.perceived_fairness)
            + self.weights.rematch_bonus * exp.rematch_rate
    }

    /// Probability that the player continues playing next session.
    pub fn retention_probability(&self, satisfaction: f64) -> f64 {
        // Logistic transform: higher satisfaction → higher retention
        1.0 / (1.0 + (-satisfaction).exp())
    }

    /// Probability the player requeues for another match (rematch).
    pub fn rematch_probability(&self, satisfaction: f64) -> f64 {
        // Rematch is a stronger commitment than staying in the population;
        // require a higher satisfaction threshold before a player requeues.
        1.0 / (1.0 + (-0.5 * (satisfaction - 2.0)).exp())
    }
}

fn mean_or(values: &[f64], default: f64) -> f64 {
    if values.is_empty() {
        default
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> SatisfactionModel {
        SatisfactionModel::new(SatisfactionWeights::default())
    }

    #[test]
    fn empty_experience_uses_defaults() {
        let m = model();
        let exp = PlayerExperience::new();
        // avg_quality 0.5, avg_queue 30.0, win_rate 0.0, streak 0, rank_change 0,
        // fairness 0.5 → (1-0.5)=0.5, rematch 0
        let expected =
            1.0 * 0.5 + (-0.01) * 30.0 + 0.5 * 0.0 + 0.0 + 0.2 * 0.0 + (-0.8) * 0.5 + 0.1 * 0.0;
        let score = m.satisfaction(&exp);
        assert!((score - expected).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn high_match_quality_increases_score() {
        let m = model();
        let mut exp = PlayerExperience::new();
        exp.recent_match_qualities = vec![1.0, 1.0, 1.0];
        let low_quality = model().satisfaction(&PlayerExperience::new());
        let high_quality = m.satisfaction(&exp);
        assert!(high_quality > low_quality);
    }

    #[test]
    fn long_queue_times_decrease_score() {
        let m = model();
        let mut exp = PlayerExperience::new();
        exp.recent_queue_times = vec![120.0, 120.0];
        let baseline = m.satisfaction(&PlayerExperience::new());
        let long_queue = m.satisfaction(&exp);
        assert!(long_queue < baseline);
    }

    #[test]
    fn loss_streak_below_minus_three_applies_penalty() {
        let m = model();
        let mut exp = PlayerExperience::new();
        exp.current_streak = -5;
        let score = m.satisfaction(&exp);
        let mut expected = 1.0 * 0.5
            + (-0.01) * 30.0
            + 0.5 * 0.0
            + (-0.3) * 2.0
            + 0.2 * 0.0
            + (-0.8) * 0.5
            + 0.1 * 0.0;
        assert!((score - expected).abs() < 1e-9, "score = {score}");
        let _ = &mut expected;
    }

    #[test]
    fn loss_streak_at_minus_two_no_penalty() {
        let m = model();
        let mut exp = PlayerExperience::new();
        exp.current_streak = -2;
        let score = m.satisfaction(&exp);
        let expected =
            1.0 * 0.5 + (-0.01) * 30.0 + 0.5 * 0.0 + 0.0 + 0.2 * 0.0 + (-0.8) * 0.5 + 0.1 * 0.0;
        assert!((score - expected).abs() < 1e-9, "score = {score}");
    }

    #[test]
    fn retention_is_monotonic_in_satisfaction() {
        let m = model();
        let p_low = m.retention_probability(-5.0);
        let p_high = m.retention_probability(5.0);
        assert!(p_high > p_low);
        assert!(p_low > 0.0 && p_low < 1.0);
        assert!(p_high > 0.0 && p_high < 1.0);
    }

    #[test]
    fn rematch_threshold_higher_than_retention() {
        let m = model();
        for s in [0.0, 1.0, 2.0, 4.0, 6.0] {
            let retention = m.retention_probability(s);
            let rematch = m.rematch_probability(s);
            assert!(
                rematch < retention,
                "rematch {rematch} should be < retention {retention} at s={s}"
            );
        }
        // Retention reaches 0.5 at s=0; rematch needs s=2 (higher threshold).
        assert!((m.retention_probability(0.0) - 0.5).abs() < 1e-9);
        assert!((m.rematch_probability(2.0) - 0.5).abs() < 1e-9);
        assert!(m.rematch_probability(0.0) < 0.5);
    }

    #[test]
    fn default_weights_match_spec_values() {
        let w = SatisfactionWeights::default();
        assert_eq!(w.match_quality, 1.0);
        assert_eq!(w.queue_time_penalty, -0.01);
        assert_eq!(w.win_bonus, 0.5);
        assert_eq!(w.loss_streak_penalty, -0.3);
        assert_eq!(w.rank_progression_bonus, 0.2);
        assert_eq!(w.fairness_sensitivity, -0.8);
        assert_eq!(w.rematch_bonus, 0.1);
    }

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
