use crate::match_::{MatchId, MatchState};
use crate::player::{PlayerId, PlayerObservation, PlayerReality, SkillVector};
use crate::rng::SimRng;
use crate::time::SimTime;
use std::collections::HashMap;
/// Global simulation state.
///
/// Truth separation (AGENTS.md principle 1): `players` holds ground truth that
/// only simulation logic and metrics may read; `observations` is what rating,
/// matchmaking, and detection systems are allowed to see. Algorithms must use
/// [`World::observe`], never `World::players` directly.
pub struct World {
    /// Ground truth. Only simulation logic and metrics read this.
    pub players: HashMap<PlayerId, PlayerReality>,
    /// What algorithms are permitted to see.
    pub observations: HashMap<PlayerId, PlayerObservation>,
    /// Match lifecycle states, keyed by match id.
    pub matches: HashMap<MatchId, MatchState>,
    /// Deterministic RNG advanced by game/match simulation logic.
    pub rng: SimRng,
    /// Monotonic simulation clock, advanced by the event engine.
    pub time: SimTime,
    next_player_id: u64,
    next_match_id: u64,
    /// Skill history for temporal ground truth . Records skill snapshots
    /// at each SkillChangeEvent. Empty when history recording is disabled.
    skill_history: HashMap<PlayerId, Vec<(SimTime, SkillVector)>>,
}
impl World {
    pub fn new(rng: SimRng) -> Self {
        Self {
            players: HashMap::new(),
            observations: HashMap::new(),
            matches: HashMap::new(),
            rng,
            time: SimTime::ZERO,
            next_player_id: 0,
            next_match_id: 0,
            skill_history: HashMap::new(),
        }
    }
    pub fn next_player_id(&mut self) -> PlayerId {
        let id = PlayerId(self.next_player_id);
        self.next_player_id += 1;
        id
    }
    pub fn next_match_id(&mut self) -> MatchId {
        let id = MatchId(self.next_match_id);
        self.next_match_id += 1;
        id
    }
    pub fn add_player(&mut self, reality: PlayerReality, observation: PlayerObservation) {
        let id = reality.id;
        self.players.insert(id, reality);
        self.observations.insert(id, observation);
    }
    /// What algorithms and detection systems see.
    pub fn observe(&self, player_id: PlayerId) -> Option<&PlayerObservation> {
        self.observations.get(&player_id)
    }
    /// Ground truth. Only simulation logic and metrics should call this.
    pub fn reality(&self, player_id: PlayerId) -> Option<&PlayerReality> {
        self.players.get(&player_id)
    }
    /// Record a skill snapshot at the current time. Called by `SkillChangeEvent`
    /// when history recording is enabled.
    pub fn record_skill_snapshot(&mut self, player_id: PlayerId, skill: SkillVector) {
        self.skill_history
            .entry(player_id)
            .or_default()
            .push((self.time, skill));
        if let Some(history) = self.skill_history.get_mut(&player_id) {
            history.sort_by_key(|(t, _)| t.0);
        }
    }
    /// Get the most recent skill at or before the given time. Falls back to the
    /// current `PlayerReality` if no history exists.
    pub fn player_skill_at(&self, player_id: PlayerId, time: SimTime) -> Option<SkillVector> {
        if let Some(history) = self.skill_history.get(&player_id) {
            if let Some((_, skill)) = history.iter().rev().find(|(t, _)| *t <= time) {
                return Some(skill.clone());
            }
        }
        self.players.get(&player_id).map(|r| r.skill.clone())
    }
    /// Check if history recording is enabled (any history exists).
    pub fn has_history(&self, player_id: PlayerId) -> bool {
        self.skill_history
            .get(&player_id)
            .is_some_and(|h| !h.is_empty())
    }
    /// Get the full skill history for a player.
    pub fn skill_history(&self, player_id: PlayerId) -> &[(SimTime, SkillVector)] {
        self.skill_history
            .get(&player_id)
            .map_or(&[], |h| h.as_slice())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{PlayerObservation, PlayerReality, SkillVector};
    fn sample_reality(id: PlayerId) -> PlayerReality {
        PlayerReality {
            id,
            skill: SkillVector::one_dimensional(1000.0),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: crate::player::Region::NA,
            location: crate::player::GeoLocation::new(crate::player::Region::NA, 0.0, 0.0),
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".to_string(),
            role: None,
        }
    }
    fn sample_observation(id: PlayerId) -> PlayerObservation {
        PlayerObservation {
            id,
            rating: 1000.0,
            hidden_mmr: 1000.0,
            visible_rank: crate::player::VisibleRank {
                tier: "gold".to_string(),
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
            session_history: Default::default(),
            quit_history: Default::default(),
            tilt_level: 0.0,
            game_mode: "ranked".to_string(),
            skill_vector: SkillVector::one_dimensional(1000.0),
            detection_flags: Vec::new(),
            role: None,
        }
    }
    #[test]
    fn player_ids_are_monotonic_and_unique() {
        let mut world = World::new(SimRng::from_seed(1));
        let a = world.next_player_id();
        let b = world.next_player_id();
        let c = world.next_player_id();
        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
        assert_eq!(c.0, 2);
        assert_ne!(a, b);
        assert_ne!(b, c);
    }
    #[test]
    fn match_ids_are_monotonic_and_unique() {
        let mut world = World::new(SimRng::from_seed(2));
        let a = world.next_match_id();
        let b = world.next_match_id();
        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
        assert_ne!(a, b);
    }
    #[test]
    fn add_player_registers_reality_and_observation() {
        let mut world = World::new(SimRng::from_seed(3));
        let id = PlayerId(10);
        world.add_player(sample_reality(id), sample_observation(id));
        assert!(world.observe(id).is_some());
        assert!(world.reality(id).is_some());
        let obs = world.observe(id).unwrap();
        assert_eq!(obs.rating, 1000.0);
        let reality = world.reality(id).unwrap();
        assert_eq!(reality.skill.overall(), 1000.0);
    }
    #[test]
    fn observe_and_reality_are_distinct() {
        let mut world = World::new(SimRng::from_seed(4));
        let id = PlayerId(5);
        let mut obs = sample_observation(id);
        obs.rating = 700.0;
        world.add_player(sample_reality(id), obs);
        assert_eq!(world.observe(id).unwrap().rating, 700.0);
        assert_eq!(world.reality(id).unwrap().skill.overall(), 1000.0);
    }
    #[test]
    fn skill_history_starts_empty() {
        let world = World::new(SimRng::from_seed(5));
        assert!(!world.has_history(PlayerId(0)));
        assert!(world.skill_history(PlayerId(0)).is_empty());
    }
    #[test]
    fn record_skill_snapshot_adds_to_history() {
        let mut world = World::new(SimRng::from_seed(5));
        let id = PlayerId(1);
        world.add_player(sample_reality(id), sample_observation(id));
        world.record_skill_snapshot(id, SkillVector::one_dimensional(1100.0));
        assert!(world.has_history(id));
        assert_eq!(world.skill_history(id).len(), 1);
        assert_eq!(world.skill_history(id)[0].1.overall(), 1100.0);
    }
    #[test]
    fn player_skill_at_returns_current_when_no_history() {
        let mut world = World::new(SimRng::from_seed(5));
        let id = PlayerId(1);
        world.add_player(sample_reality(id), sample_observation(id));
        let skill = world.player_skill_at(id, SimTime::ZERO);
        assert_eq!(skill.unwrap().overall(), 1000.0);
    }
    #[test]
    fn player_skill_at_returns_most_recent_snapshot() {
        let mut world = World::new(SimRng::from_seed(5));
        let id = PlayerId(1);
        world.add_player(sample_reality(id), sample_observation(id));
        world.time = SimTime::from_secs(10.0);
        world.record_skill_snapshot(id, SkillVector::one_dimensional(1100.0));
        world.time = SimTime::from_secs(20.0);
        world.record_skill_snapshot(id, SkillVector::one_dimensional(1200.0));
        let skill = world.player_skill_at(id, SimTime::from_secs(15.0));
        assert_eq!(skill.unwrap().overall(), 1100.0);
        let skill = world.player_skill_at(id, SimTime::from_secs(20.0));
        assert_eq!(skill.unwrap().overall(), 1200.0);
        let skill = world.player_skill_at(id, SimTime::from_secs(5.0));
        assert_eq!(skill.unwrap().overall(), 1000.0);
    }
    #[test]
    fn skill_history_sorted_by_time() {
        let mut world = World::new(SimRng::from_seed(5));
        let id = PlayerId(1);
        world.add_player(sample_reality(id), sample_observation(id));
        world.time = SimTime::from_secs(30.0);
        world.record_skill_snapshot(id, SkillVector::one_dimensional(1300.0));
        world.time = SimTime::from_secs(10.0);
        world.record_skill_snapshot(id, SkillVector::one_dimensional(1100.0));
        world.time = SimTime::from_secs(20.0);
        world.record_skill_snapshot(id, SkillVector::one_dimensional(1200.0));
        let history = world.skill_history(id);
        assert_eq!(history.len(), 3);
        assert!(history[0].0 <= history[1].0);
        assert!(history[1].0 <= history[2].0);
    }
}
