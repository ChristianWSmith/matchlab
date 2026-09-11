//! Information-budget enforcement (spec §8.2).
//!
//! Strips a `MatchResult` down to only the fields a rating system's declared
//! `information_budget()` permits it to see. The simulation calls
//! `filter_match_result` before invoking `system.update()`, so a system
//! declaring only `WinLoss` never observes kills/deaths/score/duration — budget
//! declarations are enforced at runtime, not merely decorative.
use crate::system::ObservationType;
use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::PlayerId;
use matchlab_core::time::SimTime;
use tracing;
/// A `MatchResult` reduced to only the observable fields a rating system may
/// consume. `Some` means the system declared that data in its budget.
#[derive(Debug, Clone)]
pub struct FilteredMatchResult {
    pub winner: Team,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
    pub team_a_score: Option<f64>,
    pub team_b_score: Option<f64>,
    pub player_performances: Option<Vec<PlayerPerformance>>,
    pub duration: Option<SimTime>,
    pub disconnected: Option<bool>,
    pub forfeited: Option<bool>,
    pub unexpected_events: Option<Vec<String>>,
}
impl FilteredMatchResult {
    /// Rebuild a `MatchResult` from the filtered view, zeroing/emptying every
    /// field outside the system's budget. The rating trait takes a `MatchResult`
    /// (§8.1); this is the bridge that makes the budgets genuinely enforced.
    pub fn into_match_result(&self, match_id: MatchId) -> MatchResult {
        MatchResult {
            match_id,
            winner: self.winner,
            team_a: self.team_a.clone(),
            team_b: self.team_b.clone(),
            team_a_score: self.team_a_score.unwrap_or(0.0),
            team_b_score: self.team_b_score.unwrap_or(0.0),
            player_performances: self
                .player_performances
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|mut p| {
                    p.stats.clear();
                    p.variance = 0.0;
                    p
                })
                .collect(),
            duration: self.duration.unwrap_or(SimTime::ZERO),
            disconnected: self.disconnected.unwrap_or(false),
            forfeited: self.forfeited.unwrap_or(false),
            variance: 0.0,
            unexpected_events: self.unexpected_events.clone().unwrap_or_default(),
        }
    }
}
pub fn filter_match_result(mr: &MatchResult, budget: &[ObservationType]) -> FilteredMatchResult {
    let has = |o: ObservationType| budget.contains(&o);
    let stripped: Vec<&str> = [
        (!has(ObservationType::Score)).then_some("score"),
        (!has(ObservationType::PerformanceData)).then_some("performances"),
        (!has(ObservationType::Duration)).then_some("duration"),
        (!has(ObservationType::Disconnects)).then_some("disconnects"),
        (!has(ObservationType::SessionHistory)).then_some("session_history"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !stripped.is_empty() {
        tracing::trace!(fields_stripped = ?stripped, "information budget filtering applied");
    }
    FilteredMatchResult {
        winner: mr.winner,
        team_a: mr.team_a.clone(),
        team_b: mr.team_b.clone(),
        team_a_score: if has(ObservationType::Score) {
            Some(mr.team_a_score)
        } else {
            None
        },
        team_b_score: if has(ObservationType::Score) {
            Some(mr.team_b_score)
        } else {
            None
        },
        player_performances: if has(ObservationType::PerformanceData) {
            Some(mr.player_performances.clone())
        } else {
            None
        },
        duration: if has(ObservationType::Duration) {
            Some(mr.duration)
        } else {
            None
        },
        disconnected: if has(ObservationType::Disconnects) {
            Some(mr.disconnected)
        } else {
            None
        },
        forfeited: Some(mr.forfeited),
        unexpected_events: if has(ObservationType::SessionHistory) {
            Some(mr.unexpected_events.clone())
        } else {
            None
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::match_::{MatchId, PlayerPerformance};
    use std::collections::HashMap;
    fn perf() -> PlayerPerformance {
        let mut stats = HashMap::new();
        stats.insert("kills".to_string(), 12.0);
        stats.insert("deaths".to_string(), 3.0);
        stats.insert("assists".to_string(), 7.0);
        stats.insert("impact".to_string(), 1.2);
        PlayerPerformance {
            player_id: PlayerId(1),
            stats,
            variance: 0.4,
        }
    }
    fn mr() -> MatchResult {
        MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
            team_a_score: 13.0,
            team_b_score: 9.0,
            player_performances: vec![perf()],
            duration: SimTime::from_secs(1500.0),
            disconnected: true,
            forfeited: false,
            variance: 0.05,
            unexpected_events: vec!["dc".to_string()],
        }
    }
    #[test]
    fn winloss_budget_only_exposes_winner_and_roster() {
        let f = filter_match_result(&mr(), &[ObservationType::WinLoss]);
        assert_eq!(f.winner, Team::A);
        assert_eq!(f.team_a, vec![PlayerId(1)]);
        assert_eq!(f.team_b, vec![PlayerId(2)]);
        assert_eq!(f.team_a_score, None);
        assert_eq!(f.team_b_score, None);
        assert!(f.player_performances.is_none());
        assert!(f.duration.is_none());
        assert!(f.disconnected.is_none());
        assert_eq!(f.forfeited, Some(false));
        assert!(f.unexpected_events.is_none());
    }
    #[test]
    fn winloss_sanitized_match_result_strips_all_non_observable_data() {
        let f = filter_match_result(&mr(), &[ObservationType::WinLoss]);
        let s = f.into_match_result(MatchId(1));
        assert_eq!(s.winner, Team::A);
        assert_eq!(s.team_a, vec![PlayerId(1)]);
        assert_eq!(s.team_b, vec![PlayerId(2)]);
        assert_eq!(s.team_a_score, 0.0);
        assert_eq!(s.team_b_score, 0.0);
        assert!(s.player_performances.is_empty());
        assert_eq!(s.duration, SimTime::ZERO);
        assert!(!s.disconnected);
        assert!(!s.forfeited);
        assert!(s.unexpected_events.is_empty());
    }
    #[test]
    fn performance_data_budget_exposes_stats() {
        let f = filter_match_result(&mr(), &[ObservationType::PerformanceData]);
        assert_eq!(f.team_a_score, None);
        let perfs = f.player_performances.expect("PerformanceData in budget");
        assert_eq!(perfs.len(), 1);
        assert_eq!(perfs[0].stats.get("kills"), Some(&12.0));
        assert_eq!(perfs[0].stats.get("deaths"), Some(&3.0));
        assert!(f.duration.is_none());
        assert_eq!(f.forfeited, Some(false));
    }
    #[test]
    fn score_budget_exposes_score_but_not_performances() {
        let f = filter_match_result(&mr(), &[ObservationType::Score]);
        assert_eq!(f.team_a_score, Some(13.0));
        assert_eq!(f.team_b_score, Some(9.0));
        assert!(f.player_performances.is_none());
    }
    #[test]
    fn empty_budget_only_keeps_winner_roster_and_forfeit() {
        let f = filter_match_result(&mr(), &[]);
        assert_eq!(f.winner, Team::A);
        assert_eq!(f.team_a_score, None);
        assert!(f.player_performances.is_none());
        assert!(f.duration.is_none());
        assert!(f.disconnected.is_none());
        assert_eq!(f.forfeited, Some(false));
        assert!(f.unexpected_events.is_none());
        let s = f.into_match_result(MatchId(1));
        assert_eq!(s.team_a_score, 0.0);
        assert!(s.player_performances.is_empty());
    }
    #[test]
    fn full_budget_exposes_everything() {
        let budget = vec![
            ObservationType::Score,
            ObservationType::PerformanceData,
            ObservationType::Duration,
            ObservationType::Disconnects,
            ObservationType::SessionHistory,
        ];
        let f = filter_match_result(&mr(), &budget);
        assert_eq!(f.team_a_score, Some(13.0));
        assert_eq!(f.duration, Some(SimTime::from_secs(1500.0)));
        assert_eq!(f.disconnected, Some(true));
        assert_eq!(f.forfeited, Some(false));
        assert_eq!(f.unexpected_events, Some(vec!["dc".to_string()]));
        let perfs = f.player_performances.expect("PerformanceData in budget");
        assert_eq!(perfs[0].stats.get("kills"), Some(&12.0));
        assert_eq!(perfs[0].stats.get("deaths"), Some(&3.0));
        assert_eq!(perfs[0].stats.get("assists"), Some(&7.0));
        assert_eq!(perfs[0].stats.get("impact"), Some(&1.2));
    }
    #[test]
    fn sanitized_performance_clears_stats() {
        let budget = vec![ObservationType::PerformanceData];
        let f = filter_match_result(&mr(), &budget);
        let s = f.into_match_result(MatchId(1));
        let perf = &s.player_performances[0];
        assert!(
            perf.stats.is_empty(),
            "stats should be cleared after sanitization"
        );
    }
}
