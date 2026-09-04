# Ticket 16: Wire Detection, Ranking, Adversarial, Satisfaction into Loop

## Context
Integrate all new systems into the simulation loop. The loop currently handles PlayerJoin → Queue → MatchTimer → MatchFormed → MatchEnd. It needs to support detection checks, ranking updates, adversarial agent ticks, and satisfaction-based quit/requeue.

## Scope
- Update `crates/matchlab-loop/Cargo.toml` — add deps: `matchlab-detection`, `matchlab-ranking`, `matchlab-adversarial`, `matchlab-utility`
- Update `crates/matchlab-loop/src/machine.rs`:
  - Add optional fields to `MachineState`:
    - `detection_system: Option<Box<dyn DetectionSystem>>`
    - `ranker: Option<Box<dyn RankMapper>>`
    - `adversarial_agents: HashMap<PlayerId, Box<dyn AdversarialAgent>>`
    - `satisfaction_model: Option<SatisfactionModel>`
    - `player_experiences: HashMap<PlayerId, PlayerExperience>`
  - Add handler functions:
    - `handle_detection_check(world, event, state) -> Vec<Box<dyn Event>>`
    - `handle_ranking_update(world, event, state) -> Vec<Box<dyn Event>>`
    - `handle_adversarial_tick(world, event, state) -> Vec<Box<dyn Event>>`
  - Modify `handle_match_end`:
    - Run adversarial agent ticks for adversarial players
    - Update player experiences (quality, queue time, outcome, streak)
    - Compute satisfaction → retention probability
    - If retention check fails → schedule `PlayerQuit` instead of re-queue
    - If detection system present → call `observe()` with match result
    - If ranker present → update visible ranks
    - Schedule `RatingUpdateEvent` after rating updates
    - Schedule `DetectionCheckEvent` for players with anomalous performance

## Handler Logic

### handle_detection_check
1. Call `detection_system.evaluate(player_id, world)`
2. If probability > threshold → call `recommend_action()`
3. Apply intervention action (e.g., increase K factor, restrict queue)
4. If action is `Ban` → schedule `PlayerQuit`

### handle_ranking_update
1. For each player in the match:
   - Get new rating from `world.observations`
   - Call `ranker.rating_to_rank(rating)`
   - Update `obs.visible_rank`

### handle_adversarial_tick
1. For each adversarial agent:
   - Call `agent.tick(player_id, world)`
   - Agent may modify reality (quit_probability, etc.)

### handle_match_end (modified)
1. Existing: rating update, mark completed, increment counter
2. NEW: `detection_system.observe(&result, world)`
3. NEW: Update `player_experiences` for each participant
4. NEW: For each participant, compute satisfaction → retention check
5. NEW: If retained → re-queue after delay; if not → schedule `PlayerQuit`
6. NEW: Schedule `RatingUpdateEvent`
7. NEW: Schedule `DetectionCheckEvent` for players with |rating_delta| > threshold

## Acceptance Criteria
- [ ] `cargo build -p matchlab-loop` succeeds
- [ ] `cargo test -p matchlab-loop` passes
- [ ] Detection system observes matches and evaluates players
- [ ] Ranking updates visible ranks after rating changes
- [ ] Adversarial agents modify world state on tick
- [ ] Satisfaction model influences re-queue vs quit decision
- [ ] `RatingUpdateEvent` fired after rating updates
- [ ] `DetectionCheckEvent` fired for anomalous players
- [ ] Existing behavior unchanged when optional systems are `None`

## Testing
- Unit test: `handle_detection_check` with high-probability detection → intervention
- Unit test: `handle_ranking_update` updates visible_rank correctly
- Unit test: `handle_adversarial_tick` modifies player reality
- Unit test: low satisfaction → `PlayerQuit` scheduled instead of re-queue
- Unit test: high satisfaction → re-queue scheduled as before
- Unit test: `handle_match_end` with all systems `None` → same behavior as v0.1
- Integration test: full loop with detection + ranking + satisfaction

## Dependencies
- Tickets 04, 05, 07, 08 (detection, ranking, adversarial, utility crates)
- Ticket 15 (new event types)
- `matchlab-core`, `matchlab-loop` (existing)
