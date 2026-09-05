pub mod machine;

pub use machine::{
    LoopConfig, MachineState, handle_detection_check, handle_match_end, handle_match_formed,
    handle_match_timer, handle_player_join, handle_player_queue, handle_ranking_update,
};
use matchlab_adversarial::agent::AdversarialAgent;
use matchlab_core::event::{EventEngine, EventKind, MatchTimerEvent, PlayerJoinEvent};
use matchlab_core::player::{PlayerId, PlayerObservation, PlayerReality};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_detection::detector::DetectionSystem;
use matchlab_game::outcome::OutcomeModel;
use matchlab_matchmaking::matchmaker::Matchmaker;
use matchlab_metrics::MetricsEngine;
use matchlab_ranking::ranker::RankMapper;
use matchlab_rating::system::RatingSystem;
use matchlab_utility::satisfaction::SatisfactionModel;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MatchLoop {
    pub state: Arc<Mutex<MachineState>>,
    pub world: World,
    pub engine: EventEngine,
}

impl MatchLoop {
    pub fn new(
        population: Vec<(PlayerReality, PlayerObservation)>,
        rating_system: Box<dyn RatingSystem>,
        outcome_model: Box<dyn OutcomeModel>,
        matchmaker: Box<dyn Matchmaker>,
        metrics: MetricsEngine,
        config: LoopConfig,
        seed: u64,
    ) -> Self {
        Self::with_extras(
            population,
            rating_system,
            outcome_model,
            matchmaker,
            metrics,
            config,
            seed,
            None,
            None,
            HashMap::new(),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_extras(
        population: Vec<(PlayerReality, PlayerObservation)>,
        rating_system: Box<dyn RatingSystem>,
        outcome_model: Box<dyn OutcomeModel>,
        matchmaker: Box<dyn Matchmaker>,
        metrics: MetricsEngine,
        config: LoopConfig,
        seed: u64,
        detection_system: Option<Box<dyn DetectionSystem>>,
        ranker: Option<Box<dyn RankMapper>>,
        adversarial_agents: HashMap<PlayerId, Box<dyn AdversarialAgent>>,
        satisfaction_model: Option<Box<dyn SatisfactionModel>>,
    ) -> Self {
        let state = Arc::new(Mutex::new(MachineState::with_extras(
            population,
            rating_system,
            outcome_model,
            matchmaker,
            metrics,
            config,
            detection_system,
            ranker,
            adversarial_agents,
            satisfaction_model,
        )));

        let world = World::new(SimRng::from_seed(seed));
        let mut engine = EventEngine::new();

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::PlayerJoin,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_player_join(world, event, &mut st)
            }),
        );

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::PlayerQueue,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_player_queue(world, event, &mut st)
            }),
        );

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::MatchTimer,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_match_timer(world, event, &mut st)
            }),
        );

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::MatchFormed,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_match_formed(world, event, &mut st)
            }),
        );

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::MatchEnd,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_match_end(world, event, &mut st)
            }),
        );

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::DetectionCheck,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_detection_check(world, event, &mut st)
            }),
        );

        let s = Arc::clone(&state);
        engine.register_handler(
            EventKind::RatingUpdate,
            Box::new(move |world, event| {
                let mut st = s.lock().unwrap();
                handle_ranking_update(world, event, &mut st)
            }),
        );

        let mut loop_ = Self {
            state,
            world,
            engine,
        };
        loop_.seed_initial_events();
        loop_
    }

    fn seed_initial_events(&mut self) {
        let batch_interval = self.state.lock().unwrap().batch_interval();
        let mut pids: Vec<PlayerId> = self
            .state
            .lock()
            .unwrap()
            .population
            .keys()
            .cloned()
            .collect();
        pids.sort_by_key(|pid| pid.0);
        for pid in pids {
            self.engine.schedule(Box::new(PlayerJoinEvent {
                time: SimTime::ZERO,
                player_id: pid,
            }));
        }
        self.engine.schedule(Box::new(MatchTimerEvent {
            time: batch_interval,
        }));
    }

    pub fn run(&mut self) {
        while self.engine.tick(&mut self.world) {}
    }

    /// Tick until the next pending event would occur after `until` (bounded
    /// simulations: `duration.max_time`). In-flight matches past the cutoff are
    /// simply left unfinished; `matches_completed` reflects what resolved.
    pub fn run_until(&mut self, until: SimTime) {
        loop {
            let next = match self.engine.peek_time() {
                Some(t) => t,
                None => return,
            };
            if next > until {
                return;
            }
            self.engine.tick(&mut self.world);
        }
    }

    /// Fold all recorded matches into per-metric results (spec §11.1).
    pub fn finalize_metrics(&self) -> HashMap<String, matchlab_metrics::MetricResult> {
        let mut state = self.state.lock().unwrap();
        state.metrics.finalize();
        state.metrics.results().clone()
    }
}
