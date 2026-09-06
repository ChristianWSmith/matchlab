//! Match quality analytical baselines (ticket T-03).
//!
//! The batch matchmaker's `match_quality` is `1 − |avg_a − avg_b| / 400`
//! clamped to `[0, 1]` and computed from observations (visible rating) only.
//! These tests pin the exact values for uniform, two-level, and alternating
//! populations, and guard the truth-separation invariant (quality must track
//! rating, never the ground-truth skill vector).

use matchlab_core::match_::TeamComposition;
use matchlab_core::player::{PlayerId, SkillVector};
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
use matchlab_matchmaking::lua::LuaMatchmaker;
use matchlab_matchmaking::matchmaker::{Matchmaker, ProposedMatch};
use matchlab_matchmaking::queue::Queue;
use matchlab_validation::{observation, queue_entry};

fn sym(n: usize) -> TeamComposition {
    TeamComposition {
        team_size_a: n,
        team_size_b: n,
        role_a: None,
        role_b: None,
    }
}

fn batch() -> LuaMatchmaker {
    LuaMatchmaker::load("plugins/matchmaking/batch.lua", &serde_yaml::Value::Null).unwrap()
}

fn world_with(ratings: &[(u64, f64)]) -> World {
    let mut world = World::new(SimRng::from_seed(1));
    for &(id, rating) in ratings {
        world
            .observations
            .insert(PlayerId(id), observation(id, rating));
    }
    world
}

/// Independent reference for the batch alternate-assignment quality of a fixed
/// rating multiset: sort ascending (as the script does) and hand ratings
/// alternately to team A / team B in order; quality is `1 − |avg da − avg b|/400`.
fn expected_batch_quality(ratings: &mut [f64]) -> f64 {
    ratings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut a: Vec<f64> = Vec::new();
    let mut b: Vec<f64> = Vec::new();
    let mut alternate = false;
    for &r in ratings.iter() {
        if alternate {
            b.push(r);
        } else {
            a.push(r);
        }
        alternate = !alternate;
    }
    let avg = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    let diff = (avg(&a) - avg(&b)).abs();
    1.0 - (diff / 400.0).min(1.0)
}

#[test]
fn uniform_population_quality_is_exact_one() {
    let mm = batch();
    let mut queue = Queue::default();
    for id in 1..=6u64 {
        queue.enqueue(queue_entry(id, SimTime::ZERO, 1200.0));
    }
    let world = world_with(&[
        (1, 1200.0),
        (2, 1200.0),
        (3, 1200.0),
        (4, 1200.0),
        (5, 1200.0),
        (6, 1200.0),
    ]);
    let mut rng = SimRng::from_seed(1);
    let matches = mm.find_matches(&queue, &world, &sym(3), SimTime::ZERO, &mut rng);
    assert_eq!(matches.len(), 1, "all 6 uniform players form one match");
    assert!(
        (matches[0].quality_score - 1.0).abs() < 1e-9,
        "uniform quality = {}",
        matches[0].quality_score
    );
}

#[test]
fn two_level_population_clamps_at_zero() {
    // Separated teams (all-1000 vs all-1400) are exactly 400 apart → quality
    // 0.0 (clamped lower bound). Proven on `ProposedMatch::match_quality`
    // directly with teams of known composition.
    let world = world_with(&[
        (1, 1000.0),
        (2, 1000.0),
        (3, 1000.0),
        (4, 1400.0),
        (5, 1400.0),
        (6, 1400.0),
    ]);
    let q = ProposedMatch::match_quality(
        &[PlayerId(1), PlayerId(2), PlayerId(3)],
        &[PlayerId(4), PlayerId(5), PlayerId(6)],
        &world,
    );
    assert!((q - 0.0).abs() < 1e-9, "quality = {q}");

    // Beyond 400 the quality clamps at the lower bound, it does not go
    // negative.
    let wide = world_with(&[
        (1, 1000.0),
        (2, 1000.0),
        (3, 1000.0),
        (4, 1450.0),
        (5, 1450.0),
        (6, 1450.0),
    ]);
    let q = ProposedMatch::match_quality(
        &[PlayerId(1), PlayerId(2), PlayerId(3)],
        &[PlayerId(4), PlayerId(5), PlayerId(6)],
        &wide,
    );
    assert_eq!(q, 0.0, "450-point gap must clamp, not go negative");
}

#[test]
fn mixed_population_quality_matches_alternate_assignment() {
    // Fixed multiset [1000, 1050, 1100, 1150, 1200, 1250]. Batch alternates
    // the sorted ratings: team_a = {1000, 1100, 1200} (avg 1100), team_b =
    // {1050, 1150, 1250} (avg 1150) → diff 50 → quality 0.875.
    let mm = batch();
    let ratings = [1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0];
    let mut queue = Queue::default();
    for (i, &r) in ratings.iter().enumerate() {
        queue.enqueue(queue_entry(1 + i as u64, SimTime::ZERO, r));
    }
    let world = world_with(&[
        (1, 1000.0),
        (2, 1050.0),
        (3, 1100.0),
        (4, 1150.0),
        (5, 1200.0),
        (6, 1250.0),
    ]);
    let mut rng = SimRng::from_seed(2);
    let matches = mm.find_matches(&queue, &world, &sym(3), SimTime::ZERO, &mut rng);
    assert_eq!(matches.len(), 1);
    let expected = expected_batch_quality(&mut ratings.clone());
    assert!(
        (matches[0].quality_score - expected).abs() < 1e-9,
        "observed {} vs analytic {}",
        matches[0].quality_score,
        expected
    );

    // Negative control: the uniform-average value (1.0) is NOT what the
    // alternate-assignment formula produces — asserts the formula is actually
    // exercised rather than trivially passing.
    assert!(
        (matches[0].quality_score - 1.0).abs() > 1e-6,
        "alternate-assignment quality must differ from uniform-average quality"
    );
}

#[test]
fn truth_separation_quality_tracks_rating_not_skill() {
    // Observations: all four players rated 1000 (balanced teams → quality
    // 1.0), but the ground-truth skill binding is wildly unbalanced — team A
    // is 1800 and team B is 500. Quality must follow the visible rating, never
    // skill_vector (matchmaking may not read ground truth).
    let mut world = World::new(SimRng::from_seed(3));
    for (id, skill) in [(1u64, 1800.0), (2, 1800.0), (3, 500.0), (4, 500.0)] {
        let mut o = observation(id, 1000.0);
        o.skill_vector = SkillVector::one_dimensional(skill);
        world.observations.insert(PlayerId(id), o);
    }
    let q = ProposedMatch::match_quality(
        &[PlayerId(1), PlayerId(2)],
        &[PlayerId(3), PlayerId(4)],
        &world,
    );
    assert!(
        (q - 1.0).abs() < 1e-9,
        "quality must be rating-based, got {q}"
    );
}
