//! Information-budget enforcement (spec §8.2).
//!
//! Strips a `MatchResult` down to only the fields a rating system's declared
//! `information_budget()` permits it to see. The simulation calls
//! `filter_match_result` before invoking `system.update()`, so a system
//! declaring only `WinLoss` never observes kills/deaths/score/duration — budget
//! declarations are enforced at runtime, not merely decorative.

use matchlab_core::match_::{MatchId, MatchResult, PlayerPerformance, Team};
use matchlab_core::player::PlayerId;
use matchlab_core::time::SimTime;

use crate::system::ObservationType;

/// A `MatchResult` reduced to only the observable fields a rating system may
/// consume. `Some` means the system declared that data in its budget.
#[derive(Debug, Clone)]
pub struct FilteredMatchResult {
    pub winner: Team,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
    pub team_a_score: Option<f64>,
    pub team_b_score: Option<f64>,
    pub player_performances: Option<Vec<FilteredPerformance>>,
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
                .as_ref()
                .map(|perfs| {
                    perfs
                        .iter()
                        .map(|p| PlayerPerformance {
                            player_id: p.player_id,
                            kills: p.kills.unwrap_or(0),
                            deaths: p.deaths.unwrap_or(0),
                            assists: p.assists.unwrap_or(0),
                            objective_score: p.objective_score.unwrap_or(0.0),
                            impact: p.impact.unwrap_or(0.0),
                            variance: 0.0,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            duration: self.duration.unwrap_or(SimTime::ZERO),
            disconnected: self.disconnected.unwrap_or(false),
            forfeited: self.forfeited.unwrap_or(false),
            variance: 0.0,
            unexpected_events: self.unexpected_events.clone().unwrap_or_default(),
        }
    }
}

/// A per-player performance with optional granular fields per the budget.
#[derive(Debug, Clone)]
pub struct FilteredPerformance {
    pub player_id: PlayerId,
    pub kills: Option<u32>,
    pub deaths: Option<u32>,
    pub assists: Option<u32>,
    pub objective_score: Option<f64>,
    pub impact: Option<f64>,
}

pub fn filter_match_result(mr: &MatchResult, budget: &[ObservationType]) -> FilteredMatchResult {
    let has = |o: ObservationType| budget.contains(&o);
    let has_any = |types: &[ObservationType]| types.iter().any(|t| budget.contains(t));

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
        player_performances: if has_any(&[
            ObservationType::Kills,
            ObservationType::Deaths,
            ObservationType::Assists,
            ObservationType::ObjectiveScore,
            ObservationType::Impact,
        ]) {
            Some(
                mr.player_performances
                    .iter()
                    .map(|p| FilteredPerformance {
                        player_id: p.player_id,
                        kills: if has(ObservationType::Kills) {
                            Some(p.kills)
                        } else {
                            None
                        },
                        deaths: if has(ObservationType::Deaths) {
                            Some(p.deaths)
                        } else {
                            None
                        },
                        assists: if has(ObservationType::Assists) {
                            Some(p.assists)
                        } else {
                            None
                        },
                        objective_score: if has(ObservationType::ObjectiveScore) {
                            Some(p.objective_score)
                        } else {
                            None
                        },
                        impact: if has(ObservationType::Impact) {
                            Some(p.impact)
                        } else {
                            None
                        },
                    })
                    .collect(),
            )
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

    fn perf() -> PlayerPerformance {
        PlayerPerformance {
            player_id: PlayerId(1),
            kills: 12,
            deaths: 3,
            assists: 7,
            objective_score: 42.5,
            impact: 1.2,
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
    fn kills_budget_exposes_roster_and_winloss() {
        let f = filter_match_result(&mr(), &[ObservationType::Kills]);
        assert_eq!(f.team_a_score, None);
        let perfs = f.player_performances.expect("kills in budget");
        assert_eq!(perfs.len(), 1);
        assert_eq!(perfs[0].kills, Some(12));
        assert_eq!(perfs[0].deaths, None);
        assert_eq!(perfs[0].assists, None);
        assert_eq!(perfs[0].objective_score, None);
        assert_eq!(perfs[0].impact, None);
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
            ObservationType::Kills,
            ObservationType::Deaths,
            ObservationType::Assists,
            ObservationType::ObjectiveScore,
            ObservationType::Impact,
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
        let perfs = f.player_performances.expect("all perf fields in budget");
        assert_eq!(perfs[0].kills, Some(12));
        assert_eq!(perfs[0].deaths, Some(3));
        assert_eq!(perfs[0].assists, Some(7));
        assert_eq!(perfs[0].objective_score, Some(42.5));
        assert_eq!(perfs[0].impact, Some(1.2));
    }
}
