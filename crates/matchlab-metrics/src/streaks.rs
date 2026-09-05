use matchlab_core::match_::{MatchResult, Team};
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use std::collections::HashMap;

use crate::collector::{MetricCollector, MetricResult};

/// Streaks (spec §11.3): probability of reaching 3-, 5-, 8-, and 10-game
/// winning/losing streaks. A frustrating system produces long loss streaks.
pub struct StreakCollector {
    streaks: HashMap<PlayerId, (bool, u32)>,
    max_streaks: Vec<u32>,
}

impl StreakCollector {
    pub fn new() -> Self {
        Self {
            streaks: HashMap::new(),
            max_streaks: Vec::new(),
        }
    }
}

impl Default for StreakCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for StreakCollector {
    fn name(&self) -> &str {
        "streaks"
    }

    fn record_match(&mut self, mr: &MatchResult, _world: &World) {
        for pid in mr.team_a.iter().chain(mr.team_b.iter()) {
            let is_team_a = mr.team_a.contains(pid);
            let won = (is_team_a && mr.winner == Team::A)
                || (!is_team_a && mr.winner == Team::B);
            let entry = self.streaks.entry(*pid).or_insert((true, 0));
            if (entry.0 && won) || (!entry.0 && !won) {
                entry.1 += 1;
            } else {
                self.max_streaks.push(entry.1);
                *entry = (won, 1);
            }
        }
    }

    fn compute(&self) -> MetricResult {
        let total = self.max_streaks.len() as f64;
        if total == 0.0 {
            return MetricResult::Scalar(0.0);
        }
        let p3 = self.max_streaks.iter().filter(|&&s| s >= 3).count() as f64 / total;
        let p5 = self.max_streaks.iter().filter(|&&s| s >= 5).count() as f64 / total;
        let p8 = self.max_streaks.iter().filter(|&&s| s >= 8).count() as f64 / total;
        let p10 = self.max_streaks.iter().filter(|&&s| s >= 10).count() as f64 / total;
        MetricResult::Distribution(vec![p3, p5, p8, p10])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, Team};
    use matchlab_core::player::{PlayerId, PlayerObservation, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_core::time::SimTime;
    use std::collections::VecDeque;

    fn mr(a: PlayerId, b: PlayerId, winner: Team) -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner,
            team_a: vec![a],
            team_b: vec![b],
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::ZERO,
            disconnected: false,
            forfeited: false,
            variance: 0.0,
            unexpected_events: Vec::new(),
        }
    }

    fn obs(id: u64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: VisibleRank { tier: "unranked".into(), division: 1 },
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
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
        }
    }

    #[test]
    fn streak_lengths_accumulate() {
        let mut world = World::new(SimRng::from_seed(1));
        world.observations.insert(PlayerId(1), obs(1));
        world.observations.insert(PlayerId(2), obs(2));
        let mut c = StreakCollector::new();

        // Player 1 wins 3 straight (A wins), then loses once.
        c.record_match(&mr(PlayerId(1), PlayerId(2), Team::A), &world);
        c.record_match(&mr(PlayerId(1), PlayerId(2), Team::A), &world);
        c.record_match(&mr(PlayerId(1), PlayerId(2), Team::A), &world);
        c.record_match(&mr(PlayerId(1), PlayerId(2), Team::B), &world);

        // max_streaks now has a 3-game streak recorded for player 1.
        let MetricResult::Distribution(d) = c.compute() else {
            panic!("expected distribution");
        };
        // p3 ≥ 0.5 (at least one 3-game streak among ended streaks).
        assert!(d[0] >= 0.5, "p3 = {}", d[0]);
    }

    #[test]
    fn no_streaks_is_zero() {
        let c = StreakCollector::new();
        assert_eq!(c.compute(), MetricResult::Scalar(0.0));
    }
}