use crate::event::EventEngine;
use crate::time::SimTime;
use crate::world::World;

/// Composed `World` + `EventEngine`, the top-level simulation driver.
pub struct Simulation {
    pub world: World,
    pub engine: EventEngine,
}

impl Simulation {
    pub fn new(world: World, engine: EventEngine) -> Self {
        Self { world, engine }
    }

    /// Run until the event queue is empty or the next event time exceeds `until`.
    pub fn run(&mut self, until: SimTime) {
        while !self.engine.is_empty() {
            if let Some(peek_time) = self.engine.peek_time() {
                if peek_time > until {
                    break;
                }
            }
            self.engine.tick(&mut self.world);
        }
    }

    /// Run until the event queue is empty.
    pub fn run_to_completion(&mut self) {
        while self.engine.tick(&mut self.world) {}
    }
}
