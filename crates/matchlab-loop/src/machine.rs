use matchlab_core::event::{MatchEndEvent, MatchFormedEvent, MatchTimerEvent, downcast};
use matchlab_core::match_::{MatchId, MatchResult, MatchState};
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality, Region};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_game::outcome::OutcomeModel;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_matchmaking::queue::{Queue, QueueEntry};
use matchlab_metrics::MetricsEngine;
use matchlab_rating::filter::filter_match_result;
use matchlab_rating::system::RatingSystem;
use std::collections::HashMap;

/// Config for a full simulation loop.
#[derive(Clone)]
pub struct LoopConfig {
    pub team_size: usize,
    pub batch_interval_ticks: u64,
    pub rejoin_delay: SimTime,
    pub max_matches: u64,
}

/// Shared mutable state used by the event handlers.
pub struct MachineState {
    pub population: HashMap<PlayerId, (PlayerReality, PlayerObservation)>,
    pub queue: Queue,
    pub active_matches: HashMap<MatchId, MatchResult>,
    pub matches_completed: u64,
    matches_formed: u64,
    rating_system: Box<dyn RatingSystem>,
    outcome_model: Box<dyn OutcomeModel>,
    matchmaker: Box<dyn Matchmaker>,
    pub metrics: MetricsEngine,
    team_size: usize,
    batch_interval: SimTime,
    rejoin_delay: SimTime,
    max_matches: u64,
}

impl MachineState {
    pub fn new(
        population: Vec<(PlayerReality, PlayerObservation)>,
        rating_system: Box<dyn RatingSystem>,
        outcome_model: Box<dyn OutcomeModel>,
        matchmaker: Box<dyn Matchmaker>,
        metrics: MetricsEngine,
        config: LoopConfig,
    ) -> Self {
        let pop_map: HashMap<PlayerId, (PlayerReality, PlayerObservation)> = population
            .into_iter()
            .map(|(r, o)| (r.id, (r, o)))
            .collect();
        let batch_interval = SimTime::from_secs(config.batch_interval_ticks as f64);
        Self {
            population: pop_map,
            queue: Queue::default(),
            active_matches: HashMap::new(),
            matches_completed: 0,
            matches_formed: 0,
            rating_system,
            outcome_model,
            matchmaker,
            metrics,
            team_size: config.team_size,
            batch_interval,
            rejoin_delay: config.rejoin_delay,
            max_matches: config.max_matches,
        }
    }

    pub fn batch_interval(&self) -> SimTime {
        self.batch_interval
    }

    pub fn matches_formed(&self) -> u64 {
        self.matches_formed
    }
}

pub fn handle_player_join(
    world: &mut World,
    event: &dyn matchlab_core::event::Event,
    state: &mut MachineState,
) -> Vec<Box<dyn matchlab_core::event::Event>> {
    let join = downcast::<matchlab_core::event::PlayerJoinEvent>(event).expect("PlayerJoinEvent");
    let pid = join.player_id;
    match state.population.get(&pid) {
        Some((reality, observation)) => {
            world.add_player(reality.clone(), observation.clone());
            if let Some(o) = world.observations.get_mut(&pid) {
                o.queue_joined_at = Some(world.time);
            }
            vec![Box::new(matchlab_core::event::PlayerQueueEvent {
                time: world.time,
                player_id: pid,
            })]
        }
        None => Vec::new(),
    }
}

pub fn handle_player_queue(
    world: &mut World,
    event: &dyn matchlab_core::event::Event,
    state: &mut MachineState,
) -> Vec<Box<dyn matchlab_core::event::Event>> {
    let queue_event =
        downcast::<matchlab_core::event::PlayerQueueEvent>(event).expect("PlayerQueueEvent");
    let pid = queue_event.player_id;
    if let Some(obs) = world.observe(pid).cloned() {
        let entry = QueueEntry {
            player_id: pid,
            joined_at: world.time,
            observation: obs.clone(),
            region: Region::NA,
            party_id: obs.party_id,
            game_mode: obs.game_mode.clone(),
            role: None,
            latency_ms: 30.0,
        };
        if let Some(live) = world.observations.get_mut(&pid) {
            live.queue_joined_at = Some(world.time);
        }
        state.queue.enqueue(entry);
    }
    Vec::new()
}

pub fn handle_match_timer(
    world: &mut World,
    event: &dyn matchlab_core::event::Event,
    state: &mut MachineState,
) -> Vec<Box<dyn matchlab_core::event::Event>> {
    let _timer = downcast::<MatchTimerEvent>(event);
    let mut out: Vec<Box<dyn matchlab_core::event::Event>> = Vec::new();

    let remaining = state.max_matches.saturating_sub(state.matches_formed) as usize;
    if remaining > 0 {
        let team_size = state.team_size;
        let now = world.time;
        let mut rng = std::mem::replace(&mut world.rng, SimRng::from_seed(0));
        let proposed = state
            .matchmaker
            .find_matches(&state.queue, world, team_size, now, &mut rng);
        world.rng = rng;

        let mut matched_ids: Vec<PlayerId> = Vec::new();
        for pm in proposed.into_iter().take(remaining) {
            for id in pm.team_a.iter().chain(pm.team_b.iter()) {
                matched_ids.push(*id);
            }
            let match_id = world.next_match_id();
            state.matches_formed += 1;
            out.push(Box::new(MatchFormedEvent {
                time: world.time,
                match_id,
                team_a: pm.team_a.clone(),
                team_b: pm.team_b.clone(),
            }));
        }
        state.queue.remove_batch(&matched_ids);

        let next_time = SimTime(world.time.0 + state.batch_interval.0);
        out.push(Box::new(MatchTimerEvent { time: next_time }));
    }

    out
}

pub fn handle_match_formed(
    world: &mut World,
    event: &dyn matchlab_core::event::Event,
    state: &mut MachineState,
) -> Vec<Box<dyn matchlab_core::event::Event>> {
    let formed = downcast::<MatchFormedEvent>(event).expect("MatchFormedEvent");
    let match_id = formed.match_id;

    let team_a: Vec<PlayerObservation> = formed
        .team_a
        .iter()
        .filter_map(|pid| world.observe(*pid))
        .cloned()
        .collect();
    let team_b: Vec<PlayerObservation> = formed
        .team_b
        .iter()
        .filter_map(|pid| world.observe(*pid))
        .cloned()
        .collect();

    let result = state
        .outcome_model
        .simulate(match_id, &team_a, &team_b, &mut world.rng);
    let duration = result.duration;

    state.metrics.record_match(&result, world);
    state.active_matches.insert(match_id, result);
    world.matches.insert(match_id, MatchState::InProgress);

    vec![Box::new(MatchEndEvent {
        time: SimTime(world.time.0 + duration.0),
        match_id,
    })]
}

pub fn handle_match_end(
    world: &mut World,
    event: &dyn matchlab_core::event::Event,
    state: &mut MachineState,
) -> Vec<Box<dyn matchlab_core::event::Event>> {
    let end = downcast::<MatchEndEvent>(event).expect("MatchEndEvent");
    let match_id = end.match_id;
    let result = match state.active_matches.remove(&match_id) {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut obs_map: HashMap<PlayerId, PlayerObservation> = HashMap::new();
    for pid in result.team_a.iter().chain(result.team_b.iter()) {
        if let Some(o) = world.observe(*pid) {
            obs_map.insert(*pid, o.clone());
        }
    }

    let budget = state.rating_system.information_budget();
    let filtered = filter_match_result(&result, &budget).into_match_result(result.match_id);
    let updates = state.rating_system.update(&filtered, &obs_map);
    for (pid, rs) in updates {
        if let Some(o) = world.observations.get_mut(&pid) {
            o.rating = rs.rating;
            o.rating_deviation = rs.rating_deviation;
            o.volatility = rs.volatility;
            o.games_played = rs.games_played;
        }
    }

    world.matches.insert(match_id, MatchState::Completed);
    state.matches_completed += 1;

    let mut out: Vec<Box<dyn matchlab_core::event::Event>> = Vec::new();
    if state.matches_formed < state.max_matches {
        let rejoin = SimTime(world.time.0 + state.rejoin_delay.0);
        for pid in result.team_a.iter().chain(result.team_b.iter()) {
            out.push(Box::new(matchlab_core::event::PlayerQueueEvent {
                time: rejoin,
                player_id: *pid,
            }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MatchLoop;
    use matchlab_core::event::{Event, EventKind, PlayerJoinEvent, PlayerQueueEvent};
    use matchlab_core::match_::Team;
    use matchlab_core::player::{DetectionFlag, SkillVector, VisibleRank};
    use matchlab_core::rng::SimRng;
    use matchlab_game::logistic::LogisticOutcomeModel;
    use matchlab_matchmaking::batch::BatchMatchmaker;
    use matchlab_metrics::{MatchQualityCollector, MetricsEngine};
    use matchlab_players::archetype::{ArchetypeConfig, DistributionConfig};
    use matchlab_players::population::{PopulationConfig, PopulationGenerator};
    use matchlab_rating::elo::{EloConfig, EloRatingSystem};
    use std::collections::VecDeque;

    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
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
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn reality(id: u64, skill: f64) -> PlayerReality {
        PlayerReality {
            id: PlayerId(id),
            skill: SkillVector::one_dimensional(skill),
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            consistency: 0.9,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            party_id: None,
            region: Region::NA,
            account_age: 0,
            games_played: 0,
            fatigue: 0.0,
            tilt: 0.0,
            experience: 0,
            is_online: true,
            archetype: "stable".to_string(),
        }
    }

    fn default_state(pop: Vec<(PlayerReality, PlayerObservation)>) -> MachineState {
        MachineState::new(
            pop,
            Box::new(EloRatingSystem::new(EloConfig {
                k_factor: 32.0,
                initial_rating: 1000.0,
                beta: 400.0,
            })),
            Box::new(LogisticOutcomeModel::new(400.0, 0.1)),
            Box::new(BatchMatchmaker::new(10)),
            MetricsEngine::new(),
            LoopConfig {
                team_size: 1,
                batch_interval_ticks: 10,
                rejoin_delay: SimTime::from_secs(60.0),
                max_matches: 100,
            },
        )
    }

    #[test]
    fn player_join_adds_to_world_and_schedules_queue() {
        let mut state = default_state(vec![(reality(1, 1000.0), obs(1, 1000.0))]);
        let mut world = World::new(SimRng::from_seed(1));
        let evt: Box<dyn Event> = Box::new(PlayerJoinEvent {
            time: SimTime::from_secs(0.0),
            player_id: PlayerId(1),
        });
        let out = handle_player_join(&mut world, evt.as_ref(), &mut state);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind(), EventKind::PlayerQueue);
        let q = crate::machine::downcast::<PlayerQueueEvent>(out[0].as_ref()).unwrap();
        assert_eq!(q.player_id, PlayerId(1));
        assert!(world.observe(PlayerId(1)).is_some());
    }

    #[test]
    fn player_queue_enqueues_missing_player_is_noop() {
        let mut state = default_state(Vec::new());
        let mut world = World::new(SimRng::from_seed(2));
        let evt: Box<dyn Event> = Box::new(PlayerQueueEvent {
            time: SimTime::from_secs(0.0),
            player_id: PlayerId(99),
        });
        let out = handle_player_queue(&mut world, evt.as_ref(), &mut state);
        assert!(out.is_empty());
        assert_eq!(state.queue.len(), 0);
    }

    #[test]
    fn player_queue_enqueues_existing_player() {
        let _p0 = obs(1, 1000.0);
        let mut state = default_state(vec![(reality(1, 1000.0), obs(1, 1000.0))]);
        let mut world = World::new(SimRng::from_seed(3));
        world.add_player(reality(1, 1000.0), obs(1, 1000.0));
        let evt: Box<dyn Event> = Box::new(PlayerQueueEvent {
            time: SimTime::from_secs(0.0),
            player_id: PlayerId(1),
        });
        let out = handle_player_queue(&mut world, evt.as_ref(), &mut state);
        assert!(out.is_empty());
        assert_eq!(state.queue.len(), 1);
    }

    #[test]
    fn match_timer_forms_matches_and_reschedules() {
        let mut state = default_state(vec![
            (reality(1, 1000.0), obs(1, 1000.0)),
            (reality(2, 1000.0), obs(2, 1000.0)),
        ]);
        let mut world = World::new(SimRng::from_seed(4));
        world.add_player(reality(1, 1000.0), obs(1, 1000.0));
        world.add_player(reality(2, 1000.0), obs(2, 1000.0));
        // Two players queued.
        state.queue.enqueue(QueueEntry {
            player_id: PlayerId(1),
            joined_at: SimTime::ZERO,
            observation: obs(1, 1000.0),
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".to_string(),
            role: None,
            latency_ms: 30.0,
        });
        state.queue.enqueue(QueueEntry {
            player_id: PlayerId(2),
            joined_at: SimTime::ZERO,
            observation: obs(2, 1000.0),
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".to_string(),
            role: None,
            latency_ms: 30.0,
        });

        world.time = SimTime::from_secs(10.0);
        let evt: Box<dyn Event> = Box::new(MatchTimerEvent { time: world.time });
        let out = handle_match_timer(&mut world, evt.as_ref(), &mut state);
        // 1 MatchFormed + 1 rescheduled MatchTimer.
        let formed = out
            .iter()
            .filter(|e| e.kind() == EventKind::MatchFormed)
            .count();
        let timers = out
            .iter()
            .filter(|e| e.kind() == EventKind::MatchTimer)
            .count();
        assert_eq!(formed, 1);
        assert_eq!(timers, 1);
        // Both players removed from queue.
        assert_eq!(state.queue.len(), 0);
    }

    #[test]
    fn match_formed_simulates_and_schedules_end() {
        let mut state = default_state(vec![]);
        let mut world = World::new(SimRng::from_seed(5));
        world.add_player(reality(1, 1000.0), obs(1, 1000.0));
        world.add_player(reality(2, 1000.0), obs(2, 1000.0));
        world.time = SimTime::from_secs(10.0);
        let evt: Box<dyn Event> = Box::new(MatchFormedEvent {
            time: world.time,
            match_id: MatchId(1),
            team_a: vec![PlayerId(1)],
            team_b: vec![PlayerId(2)],
        });
        let out = handle_match_formed(&mut world, evt.as_ref(), &mut state);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind(), EventKind::MatchEnd);
        assert!(out[0].time() > world.time);
        let mid = downcast::<MatchEndEvent>(out[0].as_ref()).unwrap();
        assert_eq!(mid.match_id, MatchId(1));
        assert_eq!(state.active_matches.len(), 1);
        assert_eq!(
            world.matches.get(&MatchId(1)),
            Some(&MatchState::InProgress)
        );
    }

    #[test]
    fn match_end_applies_ratings_and_requeues() {
        let mut state = default_state(vec![]);
        let mut world = World::new(SimRng::from_seed(6));
        let a = obs(1, 1000.0);
        let b = obs(2, 1000.0);
        world.add_player(reality(1, 1000.0), a.clone());
        world.add_player(reality(2, 1000.0), b.clone());

        let team_a = vec![PlayerId(1)];
        let team_b = vec![PlayerId(2)];
        let result = MatchResult {
            match_id: MatchId(1),
            winner: Team::A,
            team_a: team_a.clone(),
            team_b: team_b.clone(),
            team_a_score: 1.0,
            team_b_score: 0.0,
            player_performances: Vec::new(),
            duration: SimTime::from_secs(1800.0),
            disconnected: false,
            forfeited: false,
            variance: 0.1,
            unexpected_events: Vec::new(),
        };
        state.active_matches.insert(MatchId(1), result.clone());

        world.time = SimTime::from_secs(1810.0);
        let evt: Box<dyn Event> = Box::new(MatchEndEvent {
            time: world.time,
            match_id: MatchId(1),
        });
        let out = handle_match_end(&mut world, evt.as_ref(), &mut state);
        // Requeue both players.
        assert_eq!(out.len(), 2);
        assert_ne!(world.observations[&PlayerId(1)].rating, 1000.0);
        assert!(
            !world.observations[&PlayerId(1)]
                .recent_performances
                .is_empty()
                || world.observations[&PlayerId(1)].games_played > 0
        );
        assert_eq!(state.matches_completed, 1);
        assert_eq!(world.matches.get(&MatchId(1)), Some(&MatchState::Completed));
    }

    #[test]
    fn full_pipeline_deterministic_and_reaches_max_matches() {
        let archetype = ArchetypeConfig {
            name: "stable".to_string(),
            proportion: 1.0,
            skill_distribution: DistributionConfig::Normal {
                mean: 1000.0,
                stddev: 150.0,
            },
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            initial_rating: None,
        };
        let config = PopulationConfig {
            size: 100,
            archetypes: vec![archetype],
        };
        let mut rng = SimRng::from_seed(42);
        let (realities, obs_list) = PopulationGenerator::generate(&config, &mut rng);
        let pop: Vec<(PlayerReality, PlayerObservation)> =
            realities.into_iter().zip(obs_list).collect();

        let cfg = LoopConfig {
            team_size: 5,
            batch_interval_ticks: 60,
            rejoin_delay: SimTime::from_secs(30.0),
            max_matches: 40,
        };
        let mut loop_a = MatchLoop::new(
            pop.clone(),
            Box::new(EloRatingSystem::new(EloConfig {
                k_factor: 32.0,
                initial_rating: 1000.0,
                beta: 400.0,
            })),
            Box::new(LogisticOutcomeModel::new(400.0, 0.1)),
            Box::new(BatchMatchmaker::new(60)),
            MetricsEngine::new(),
            cfg,
            1234,
        );
        loop_a.run();

        let total_games: u64 = {
            let st = loop_a.state.lock().unwrap();
            st.matches_completed
        };

        let observed_games: u64 = loop_a
            .world
            .observations
            .values()
            .map(|o| o.games_played)
            .sum();

        assert_eq!(
            total_games, 40,
            "loop should run exactly max_matches matches"
        );
        assert!(observed_games >= 40 * 10, "all participants gained games");
    }

    #[test]
    fn same_seed_produces_identical_results() {
        let archetype = ArchetypeConfig {
            name: "stable".to_string(),
            proportion: 1.0,
            skill_distribution: DistributionConfig::Normal {
                mean: 1000.0,
                stddev: 150.0,
            },
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            initial_rating: None,
        };
        let config = PopulationConfig {
            size: 100,
            archetypes: vec![archetype],
        };
        let mut rng = SimRng::from_seed(42);
        let (realities, obs_list) = PopulationGenerator::generate(&config, &mut rng);
        let pop: Vec<(PlayerReality, PlayerObservation)> =
            realities.into_iter().zip(obs_list).collect();

        let cfg = LoopConfig {
            team_size: 5,
            batch_interval_ticks: 60,
            rejoin_delay: SimTime::from_secs(30.0),
            max_matches: 40,
        };

        let build = |pop: Vec<(PlayerReality, PlayerObservation)>, cfg: LoopConfig| {
            MatchLoop::new(
                pop,
                Box::new(EloRatingSystem::new(EloConfig {
                    k_factor: 32.0,
                    initial_rating: 1000.0,
                    beta: 400.0,
                })),
                Box::new(LogisticOutcomeModel::new(400.0, 0.1)),
                Box::new(BatchMatchmaker::new(60)),
                MetricsEngine::new(),
                cfg,
                1234,
            )
        };
        let mut loop_a = build(pop.clone(), cfg.clone());
        let mut loop_b = build(pop, cfg);
        loop_a.run();
        loop_b.run();

        let snapshot = |l: &MatchLoop| {
            let st = l.state.lock().unwrap();
            let mut ratings: Vec<(u64, f64, u64)> = st
                .population
                .keys()
                .map(|pid| {
                    let o = &l.world.observations[pid];
                    (pid.0, o.rating, o.games_played)
                })
                .collect();
            ratings.sort_by_key(|r| r.0);
            (st.matches_completed, ratings)
        };

        let (a_completed, a_ratings) = snapshot(&loop_a);
        let (b_completed, b_ratings) = snapshot(&loop_b);
        assert_eq!(a_completed, b_completed);
        assert_eq!(
            a_ratings, b_ratings,
            "same seed must give identical ratings"
        );
    }

    #[test]
    fn full_pipeline_records_and_finalizes_metrics() {
        let archetype = ArchetypeConfig {
            name: "stable".to_string(),
            proportion: 1.0,
            skill_distribution: DistributionConfig::Normal {
                mean: 1000.0,
                stddev: 150.0,
            },
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            initial_rating: None,
        };
        let config = PopulationConfig {
            size: 40,
            archetypes: vec![archetype],
        };
        let mut rng = SimRng::from_seed(42);
        let (realities, obs_list) = PopulationGenerator::generate(&config, &mut rng);
        let pop: Vec<(PlayerReality, PlayerObservation)> =
            realities.into_iter().zip(obs_list).collect();

        let mut metrics = MetricsEngine::new();
        metrics.register(Box::new(MatchQualityCollector::new()));

        let cfg = LoopConfig {
            team_size: 5,
            batch_interval_ticks: 60,
            rejoin_delay: SimTime::from_secs(30.0),
            max_matches: 20,
        };
        let mut loop_a = MatchLoop::new(
            pop,
            Box::new(EloRatingSystem::new(EloConfig {
                k_factor: 32.0,
                initial_rating: 1000.0,
                beta: 400.0,
            })),
            Box::new(LogisticOutcomeModel::new(400.0, 0.1)),
            Box::new(BatchMatchmaker::new(60)),
            metrics,
            cfg,
            1234,
        );
        loop_a.run();

        let completed = loop_a.state.lock().unwrap().matches_completed;
        assert_eq!(completed, 20);
        let results = loop_a.finalize_metrics();
        assert!(
            results.contains_key("match_quality"),
            "match_quality should be recorded and finalized"
        );
    }
}
