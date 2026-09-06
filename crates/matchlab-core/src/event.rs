use crate::match_::MatchId;
use crate::player::PlayerId;
use crate::time::SimTime;
use crate::world::World;
use std::any::Any;
use std::collections::{BinaryHeap, HashMap};

pub trait Event: std::fmt::Debug + Send + Sync + Any {
    fn time(&self) -> SimTime;
    fn kind(&self) -> EventKind;
    /// Type-erased self reference for checked payload downcasting
    /// (`event.as_any().downcast_ref::<ConcreteEvent>()`). Handlers use this to
    /// read the payload carried by a concrete event after matching on `kind()`.
    fn as_any(&self) -> &dyn Any;
}

/// Convenience: downcast a `&dyn Event` to a concrete event type.
pub fn downcast<T: Event>(event: &dyn Event) -> Option<&T> {
    event.as_any().downcast_ref::<T>()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    PlayerJoin,
    PlayerLeave,
    PlayerQueue,
    PlayerQuit,
    PlayerReturn,
    PlayerDisconnect,
    MatchFormed,
    MatchStart,
    MatchEnd,
    RatingUpdate,
    DetectionCheck,
    SkillChange,
    MatchTimer,
}

/// An event wrapped with its dispatch metadata for the priority queue.
///
/// Ordered by reverse time so `BinaryHeap` behaves as a min-heap on `time`
/// (earliest events pop first).
pub struct TimestampedEvent {
    pub time: SimTime,
    pub kind: EventKind,
    pub inner: Box<dyn Event>,
}

impl TimestampedEvent {
    fn new(inner: Box<dyn Event>) -> Self {
        Self {
            time: inner.time(),
            kind: inner.kind(),
            inner,
        }
    }
}

impl PartialEq for TimestampedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for TimestampedEvent {}

impl PartialOrd for TimestampedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimestampedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse so BinaryHeap pops earliest time first.
        other.time.cmp(&self.time)
    }
}

pub type EventHandler = Box<dyn Fn(&mut World, &dyn Event) -> Vec<Box<dyn Event>> + Send + Sync>;

pub struct EventEngine {
    queue: BinaryHeap<TimestampedEvent>,
    handlers: HashMap<EventKind, Vec<EventHandler>>,
}

impl Default for EventEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEngine {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            handlers: HashMap::new(),
        }
    }

    pub fn register_handler(&mut self, kind: EventKind, handler: EventHandler) {
        self.handlers.entry(kind).or_default().push(handler);
    }

    pub fn schedule(&mut self, event: Box<dyn Event>) {
        self.queue.push(TimestampedEvent::new(event));
    }

    pub fn next_event(&mut self) -> Option<TimestampedEvent> {
        self.queue.pop()
    }

    pub fn peek_time(&self) -> Option<SimTime> {
        self.queue.peek().map(|e| e.time)
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Pop the next event, advance the clock to its time, run its handlers, and
    /// schedule any events they emit. Returns `false` when the queue is empty.
    pub fn tick(&mut self, world: &mut World) -> bool {
        let event = match self.next_event() {
            Some(e) => e,
            None => return false,
        };

        world.time = event.time;

        let mut pending: Vec<Box<dyn Event>> = Vec::new();
        if let Some(handlers) = self.handlers.get(&event.kind) {
            for handler in handlers {
                pending.extend(handler(world, event.inner.as_ref()));
            }
        }
        for e in pending {
            self.schedule(e);
        }

        true
    }
}

#[derive(Debug)]
pub struct PlayerJoinEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerJoinEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::PlayerJoin
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct PlayerLeaveEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerLeaveEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::PlayerLeave
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct PlayerQueueEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerQueueEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::PlayerQueue
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct PlayerQuitEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerQuitEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::PlayerQuit
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct PlayerReturnEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for PlayerReturnEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::PlayerReturn
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct PlayerDisconnectEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
    pub match_id: MatchId,
}

impl Event for PlayerDisconnectEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::PlayerDisconnect
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct MatchFormedEvent {
    pub time: SimTime,
    pub match_id: MatchId,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
}

impl Event for MatchFormedEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::MatchFormed
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct MatchEndEvent {
    pub time: SimTime,
    pub match_id: MatchId,
}

impl Event for MatchEndEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::MatchEnd
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Fired when a formed match begins play. Lets handlers distinguish formation
/// from play start (e.g. for detecting quits/forfeits during the match).
#[derive(Debug)]
pub struct MatchStartEvent {
    pub time: SimTime,
    pub match_id: MatchId,
    pub team_a: Vec<PlayerId>,
    pub team_b: Vec<PlayerId>,
}

impl Event for MatchStartEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::MatchStart
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Fired after rating updates are applied, so detection systems and metrics
/// can observe rating changes.
#[derive(Debug)]
pub struct RatingUpdateEvent {
    pub time: SimTime,
    pub match_id: MatchId,
    pub players: Vec<PlayerId>,
}

impl Event for RatingUpdateEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::RatingUpdate
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Periodic trigger to run detection evaluation for a specific player.
#[derive(Debug)]
pub struct DetectionCheckEvent {
    pub time: SimTime,
    pub player_id: PlayerId,
}

impl Event for DetectionCheckEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::DetectionCheck
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Periodic trigger that asks the active matchmaker to form matches.
#[derive(Debug)]
pub struct MatchTimerEvent {
    pub time: SimTime,
}

impl Event for MatchTimerEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::MatchTimer
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct SkillChangeEvent {
    pub time: SimTime,
}

impl Event for SkillChangeEvent {
    fn time(&self) -> SimTime {
        self.time
    }
    fn kind(&self) -> EventKind {
        EventKind::SkillChange
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SimRng;
    use std::sync::{Arc, Mutex};

    fn record_into(recorded: &Arc<Mutex<Vec<SimTime>>>) -> EventHandler {
        let recorded = Arc::clone(recorded);
        Box::new(move |_world: &mut World, event: &dyn Event| {
            let mut guard = recorded.lock().unwrap();
            guard.push(event.time());
            Vec::new()
        })
    }

    #[test]
    fn downcast_helper_recovers_concrete_payload() {
        let event: Box<dyn Event> = Box::new(PlayerJoinEvent {
            time: SimTime::from_secs(5.0),
            player_id: PlayerId(7),
        });
        let recovered = downcast::<PlayerJoinEvent>(event.as_ref()).expect("downcast");
        assert_eq!(recovered.player_id, PlayerId(7));
        assert_eq!(recovered.time, SimTime::from_secs(5.0));

        // Wrong type yields None.
        assert!(downcast::<PlayerLeaveEvent>(event.as_ref()).is_none());
    }

    #[test]
    fn match_start_event_kind_and_downcast() {
        let event: Box<dyn Event> = Box::new(MatchStartEvent {
            time: SimTime::from_secs(10.0),
            match_id: MatchId(3),
            team_a: vec![PlayerId(1), PlayerId(2)],
            team_b: vec![PlayerId(3)],
        });
        assert_eq!(event.kind(), EventKind::MatchStart);
        let recovered = downcast::<MatchStartEvent>(event.as_ref()).expect("downcast");
        assert_eq!(recovered.match_id, MatchId(3));
        assert_eq!(recovered.team_a, vec![PlayerId(1), PlayerId(2)]);
    }

    #[test]
    fn rating_update_event_kind_and_downcast() {
        let event: Box<dyn Event> = Box::new(RatingUpdateEvent {
            time: SimTime::from_secs(12.0),
            match_id: MatchId(4),
            players: vec![PlayerId(1), PlayerId(2)],
        });
        assert_eq!(event.kind(), EventKind::RatingUpdate);
        let recovered = downcast::<RatingUpdateEvent>(event.as_ref()).expect("downcast");
        assert_eq!(recovered.match_id, MatchId(4));
        assert_eq!(recovered.players, vec![PlayerId(1), PlayerId(2)]);
    }

    #[test]
    fn detection_check_event_kind_and_downcast() {
        let event: Box<dyn Event> = Box::new(DetectionCheckEvent {
            time: SimTime::from_secs(15.0),
            player_id: PlayerId(9),
        });
        assert_eq!(event.kind(), EventKind::DetectionCheck);
        let recovered = downcast::<DetectionCheckEvent>(event.as_ref()).expect("downcast");
        assert_eq!(recovered.player_id, PlayerId(9));
    }

    #[test]
    fn new_event_kinds_are_in_eventkind_enum() {
        assert_eq!(EventKind::MatchStart, EventKind::MatchStart);
        assert_eq!(EventKind::RatingUpdate, EventKind::RatingUpdate);
        assert_eq!(EventKind::DetectionCheck, EventKind::DetectionCheck);
    }

    #[test]
    fn events_execute_in_time_order() {
        let mut world = World::new(SimRng::from_seed(1));
        let mut engine = EventEngine::new();

        let recorded = Arc::new(Mutex::new(Vec::new()));
        engine.register_handler(EventKind::PlayerJoin, record_into(&recorded));
        engine.register_handler(EventKind::PlayerLeave, record_into(&recorded));

        engine.schedule(Box::new(PlayerLeaveEvent {
            time: SimTime::from_secs(10.0),
            player_id: PlayerId(1),
        }));
        engine.schedule(Box::new(PlayerJoinEvent {
            time: SimTime::from_secs(1.0),
            player_id: PlayerId(0),
        }));
        engine.schedule(Box::new(PlayerLeaveEvent {
            time: SimTime::from_secs(5.0),
            player_id: PlayerId(2),
        }));

        while engine.tick(&mut world) {}

        assert_eq!(
            recorded.lock().unwrap().as_slice(),
            &[
                SimTime::from_secs(1.0),
                SimTime::from_secs(5.0),
                SimTime::from_secs(10.0),
            ]
        );
    }

    #[test]
    fn clock_advances_to_event_time_and_skips_idle_periods() {
        let mut world = World::new(SimRng::from_seed(2));
        let mut engine = EventEngine::new();

        engine.schedule(Box::new(PlayerJoinEvent {
            time: SimTime::from_secs(100.0),
            player_id: PlayerId(0),
        }));

        assert!(engine.tick(&mut world));
        assert_eq!(world.time, SimTime::from_secs(100.0));
        assert!(engine.is_empty());
    }

    #[test]
    fn handler_follow_up_events_are_scheduled_and_run() {
        let mut world = World::new(SimRng::from_seed(3));
        let mut engine = EventEngine::new();

        // A PlayerJoin handler schedules a follow-up PlayerLeave 5 seconds after
        // the incoming event's time (no payload downcast needed).
        engine.register_handler(
            EventKind::PlayerJoin,
            Box::new(|_world: &mut World, event: &dyn Event| {
                // +5 seconds on the incoming event's time (SimTime is a pub
                // u64, so construct the offset directly).
                let later = SimTime(event.time().0 + SimTime::from_secs(5.0).0);
                vec![Box::new(PlayerLeaveEvent {
                    time: later,
                    player_id: PlayerId(1),
                }) as Box<dyn Event>]
            }),
        );

        let left_at = Arc::new(Mutex::new(None));
        let probe = Arc::clone(&left_at);
        engine.register_handler(
            EventKind::PlayerLeave,
            Box::new(move |world: &mut World, _event: &dyn Event| {
                *probe.lock().unwrap() = Some(world.time);
                Vec::new()
            }),
        );

        engine.schedule(Box::new(PlayerJoinEvent {
            time: SimTime::ZERO,
            player_id: PlayerId(1),
        }));

        while engine.tick(&mut world) {}

        assert_eq!(*left_at.lock().unwrap(), Some(SimTime::from_secs(5.0)));
    }

    #[test]
    fn run_until_stops_at_boundary_and_leaves_later_events_queued() {
        let world = World::new(SimRng::from_seed(4));
        let mut engine = EventEngine::new();

        engine.schedule(Box::new(PlayerJoinEvent {
            time: SimTime::from_secs(2.0),
            player_id: PlayerId(0),
        }));
        engine.schedule(Box::new(PlayerLeaveEvent {
            time: SimTime::from_secs(20.0),
            player_id: PlayerId(0),
        }));

        let mut sim = crate::simulation::Simulation::new(world, engine);
        sim.run(SimTime::from_secs(10.0));

        // The 2s event ran; the 20s event is still queued.
        assert_eq!(sim.world.time, SimTime::from_secs(2.0));
        assert!(!sim.engine.is_empty());
        assert_eq!(sim.engine.peek_time(), Some(SimTime::from_secs(20.0)));

        // run_to_completion drains the rest.
        sim.run_to_completion();
        assert_eq!(sim.world.time, SimTime::from_secs(20.0));
        assert!(sim.engine.is_empty());
    }

    #[test]
    fn run_until_with_event_exactly_at_boundary_runs_it() {
        let world = World::new(SimRng::from_seed(5));
        let mut engine = EventEngine::new();
        engine.schedule(Box::new(PlayerJoinEvent {
            time: SimTime::from_secs(10.0),
            player_id: PlayerId(0),
        }));

        let mut sim = crate::simulation::Simulation::new(world, engine);
        sim.run(SimTime::from_secs(10.0));
        assert_eq!(sim.world.time, SimTime::from_secs(10.0));
        assert!(sim.engine.is_empty());
    }
}
