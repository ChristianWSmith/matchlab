use matchlab_core::match_::MatchResult;
use matchlab_core::player::{PlayerId, PlayerObservation};
use matchlab_lua::DataRequirements;
use std::collections::HashMap;
pub trait RatingSystem: Send + Sync {
    fn initialize(&self, player_id: PlayerId) -> RatingState;
    fn predict(&self, team_a: &[PlayerObservation], team_b: &[PlayerObservation]) -> f64;
    fn update(
        &self,
        match_result: &MatchResult,
        observations: &HashMap<PlayerId, PlayerObservation>,
    ) -> HashMap<PlayerId, RatingState>;
    fn rating(&self, state: &RatingState) -> f64 {
        state.rating
    }
    fn uncertainty(&self, state: &RatingState) -> f64 {
        state.rating_deviation
    }
    fn data_requirements(&self) -> DataRequirements {
        DataRequirements::default()
    }
}
#[derive(Debug, Clone)]
pub struct RatingState {
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: u64,
}
#[cfg(test)]
mod tests {
    use super::*;
    struct DummySystem;
    impl RatingSystem for DummySystem {
        fn initialize(&self, _player_id: PlayerId) -> RatingState {
            RatingState {
                rating: 1000.0,
                rating_deviation: 350.0,
                volatility: 0.06,
                games_played: 0,
            }
        }
        fn predict(&self, _a: &[PlayerObservation], _b: &[PlayerObservation]) -> f64 {
            0.5
        }
        fn update(
            &self,
            _mr: &MatchResult,
            _obs: &HashMap<PlayerId, PlayerObservation>,
        ) -> HashMap<PlayerId, RatingState> {
            HashMap::new()
        }
    }
    #[test]
    fn default_rating_returns_state_rating() {
        let sys = DummySystem;
        let state = sys.initialize(PlayerId(0));
        assert_eq!(sys.rating(&state), 1000.0);
    }
    #[test]
    fn default_uncertainty_returns_rd() {
        let sys = DummySystem;
        let state = sys.initialize(PlayerId(0));
        assert_eq!(sys.uncertainty(&state), 350.0);
    }
}
