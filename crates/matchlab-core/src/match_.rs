use crate::player::PlayerId;
use crate::time::SimTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    A,
    B,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchState {
    Formed,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub match_id: MatchId,
    pub winner: Team,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
    pub team_a_score: f64,
    pub team_b_score: f64,
    pub player_performances: Vec<PlayerPerformance>,
    pub duration: SimTime,
    pub disconnected: bool,
    pub forfeited: bool,
    /// Match-to-match outcome randomness for this game.
    pub variance: f64,
    pub unexpected_events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlayerPerformance {
    pub player_id: PlayerId,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub objective_score: f64,
    pub impact: f64,
    /// Per-performance randomness.
    pub variance: f64,
}

#[derive(Debug, Clone)]
pub struct MatchConfig {
    pub team_size: usize,
}
