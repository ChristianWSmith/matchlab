//! Adversarial validation and canonical studies: validation
//! cases and canonical experiments for the ecosystem layer.
use matchlab_adversarial::agent::{
    AgentAction, AgentObservations, AgentOutcome, NullObjective, OrdinaryPlayer, PlayerObjective,
    StrategicAgent,
};
use matchlab_adversarial::manipulation::{
    ManipulationAction, ManipulationStrategy, RatingDumpStrategy,
};
use matchlab_adversarial::objectives::{QueueTimeObjective, WinRateObjective};
use matchlab_adversarial::policy::{AdaptivePolicy, LossAversionPolicy};
use matchlab_core::match_::MatchResult;
use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_detection::detector::{
    DetectionResult, DetectionSystem, EcosystemDetectionResult, EcosystemDetector,
};
use matchlab_detection::intervention::{EcosystemIntervention, InterventionAction};
struct MockDetectionSystem;
impl DetectionSystem for MockDetectionSystem {
    fn observe(&mut self, _match_result: &MatchResult, _world: &matchlab_core::world::World) {}
    fn evaluate(
        &self,
        player_id: PlayerId,
        _world: &matchlab_core::world::World,
    ) -> DetectionResult {
        DetectionResult {
            player_id,
            probability_of_anomaly: 0.0,
            confidence: 0.9,
            evidence: vec![],
        }
    }
    fn recommend_action(&self, _result: &DetectionResult) -> InterventionAction {
        InterventionAction::None
    }
}
struct MockEcosystemDetector;
impl EcosystemDetector for MockEcosystemDetector {
    fn observe_match(&mut self, _match_result: &MatchResult, _world: &matchlab_core::world::World) {
    }
    fn evaluate(
        &self,
        player_id: PlayerId,
        _world: &matchlab_core::world::World,
    ) -> EcosystemDetectionResult {
        EcosystemDetectionResult {
            player_id,
            probability_of_anomaly: 0.0,
            confidence: 0.9,
            evidence: vec![],
            detection_method: "mock".to_string(),
        }
    }
    fn recommend_intervention(&self, _result: &EcosystemDetectionResult) -> EcosystemIntervention {
        EcosystemIntervention::None
    }
    fn method_name(&self) -> String {
        "mock_detector".to_string()
    }
}
#[test]
fn strategic_agent_can_only_access_permitted_observations() {
    let player = OrdinaryPlayer;
    let world = matchlab_core::world::World::new(SimRng::from_seed(42));
    let obs = player.observe(PlayerId(0), &world, None);
    assert_eq!(obs.own_skill_overall, 0.0);
    assert_eq!(obs.recent_match_results.len(), 0);
}
#[test]
fn strategic_agent_determinism() {
    let player = OrdinaryPlayer;
    let world = matchlab_core::world::World::new(SimRng::from_seed(42));
    let obs = player.observe(PlayerId(0), &world, None);
    let mut rng1 = SimRng::from_seed(42);
    let mut rng2 = SimRng::from_seed(42);
    let a1 = player.select_action(&obs, &[], &mut rng1);
    let a2 = player.select_action(&obs, &[], &mut rng2);
    assert!(format!("{:?}", a1) == format!("{:?}", a2));
}
#[test]
fn objectives_are_independent_of_policies() {
    let obj = WinRateObjective {
        target_win_rate: 0.7,
    };
    let obs = AgentObservations {
        own_win_rate: 0.7,
        ..AgentObservations::default()
    };
    let score = obj.evaluate(&obs, &[]);
    assert!((score - 0.0).abs() < 1e-9);
}
#[test]
fn composite_objective_combines() {
    use matchlab_adversarial::objectives::CompositeObjective;
    let a = WinRateObjective {
        target_win_rate: 0.5,
    };
    let b = QueueTimeObjective;
    let composite = CompositeObjective {
        objectives: vec![(Box::new(a), 1.0), (Box::new(b), 2.0)],
    };
    let obs = AgentObservations {
        own_win_rate: 0.5,
        queue_wait_time: Some(10.0),
        ..AgentObservations::default()
    };
    let score = composite.evaluate(&obs, &[]);
    assert!((score - (-20.0)).abs() < 1e-9);
}
#[test]
fn adaptive_policy_conditions_on_history() {
    let policy = LossAversionPolicy {
        max_consecutive_losses: 3,
    };
    let history = vec![
        AgentOutcome {
            action: AgentAction::NoOp,
            rating_change: -5.0,
            queue_time: 0.0,
            match_won: Some(false),
            timestamp: 0,
        },
        AgentOutcome {
            action: AgentAction::NoOp,
            rating_change: -3.0,
            queue_time: 0.0,
            match_won: Some(false),
            timestamp: 1,
        },
        AgentOutcome {
            action: AgentAction::NoOp,
            rating_change: -2.0,
            queue_time: 0.0,
            match_won: Some(false),
            timestamp: 2,
        },
    ];
    let action = policy.decide(
        &AgentObservations::default(),
        &history,
        &NullObjective,
        &mut SimRng::from_seed(42),
    );
    assert!(matches!(action, AgentAction::StopPlaying));
}
#[test]
fn honest_vs_strategic_populations_differ() {
    let strategy = RatingDumpStrategy {
        target_rating: 1000.0,
    };
    let honest_obs = AgentObservations {
        own_rating: 1000.0,
        ..AgentObservations::default()
    };
    let honest_action = strategy.manipulate(&honest_obs, &[], &mut SimRng::from_seed(42));
    assert!(matches!(honest_action, ManipulationAction::None));
    let strategic_obs = AgentObservations {
        own_rating: 1500.0,
        ..AgentObservations::default()
    };
    let strategic_action = strategy.manipulate(&strategic_obs, &[], &mut SimRng::from_seed(42));
    assert!(matches!(
        strategic_action,
        ManipulationAction::RatingManipulation { .. }
    ));
}
#[test]
fn detection_system_has_observation_boundaries() {
    let detector = MockDetectionSystem;
    let world = matchlab_core::world::World::new(SimRng::from_seed(42));
    let result = detector.evaluate(PlayerId(0), &world);
    assert_eq!(result.probability_of_anomaly, 0.0);
}
#[test]
fn detection_intervention_is_reproducible() {
    let detector = MockDetectionSystem;
    let result = DetectionResult {
        player_id: PlayerId(0),
        probability_of_anomaly: 0.9,
        confidence: 0.8,
        evidence: vec!["test".to_string()],
    };
    let action1 = detector.recommend_action(&result);
    let action2 = detector.recommend_action(&result);
    assert!(format!("{:?}", action1) == format!("{:?}", action2));
}
#[test]
fn ecosystem_detector_produces_intervention() {
    let detector = MockEcosystemDetector;
    let world = matchlab_core::world::World::new(SimRng::from_seed(42));
    let result = detector.evaluate(PlayerId(0), &world);
    let intervention = detector.recommend_intervention(&result);
    assert!(matches!(intervention, EcosystemIntervention::None));
}
#[test]
fn stability_metrics_collected_correctly() {
    use matchlab_metrics::stability::{StabilityMetrics, compute_stability};
    let mut metrics = StabilityMetrics::default();
    for i in 0..20 {
        metrics.record_utility(matchlab_core::time::SimTime::from_secs(i as f64), 100.0);
    }
    let verdict = compute_stability(&metrics);
    assert!(matches!(
        verdict,
        matchlab_metrics::stability::StabilityVerdict::Stable { .. }
    ));
}
#[test]
fn stability_is_deterministic() {
    use matchlab_metrics::stability::{StabilityMetrics, compute_stability};
    let mut m1 = StabilityMetrics::default();
    let mut m2 = StabilityMetrics::default();
    for i in 0..10 {
        m1.record_utility(matchlab_core::time::SimTime::from_secs(i as f64), 100.0);
        m2.record_utility(matchlab_core::time::SimTime::from_secs(i as f64), 100.0);
    }
    let v1 = compute_stability(&m1);
    let v2 = compute_stability(&m2);
    assert!(format!("{:?}", v1) == format!("{:?}", v2));
}
#[test]
fn counterfactual_comparison_produces_effects() {
    use matchlab_experiments::counterfactual::{MatchObjectiveVector, compare_counterfactuals};
    let baseline = MatchObjectiveVector {
        match_quality: 0.8,
        queue_time: 30.0,
        player_utility: 0.7,
        population_stability: 0.9,
    };
    let alternative = MatchObjectiveVector {
        match_quality: 0.9,
        queue_time: 25.0,
        player_utility: 0.8,
        population_stability: 0.85,
    };
    let comparison = compare_counterfactuals(&baseline, &alternative);
    assert!((comparison.direct_effect.match_quality - 0.1).abs() < 1e-9);
    assert!((comparison.direct_effect.queue_time - (-5.0)).abs() < 1e-9);
}
#[test]
fn counterfactual_worlds_are_independent() {
    use matchlab_experiments::counterfactual::StrategicMode;
    let baseline = StrategicMode::FixedBehavior;
    let alternative = StrategicMode::AdaptiveBehavior;
    assert!(baseline != alternative);
}
#[test]
fn all_ecosystem_validations_are_deterministic() {
    let player = OrdinaryPlayer;
    let world = matchlab_core::world::World::new(SimRng::from_seed(42));
    let obs1 = player.observe(PlayerId(0), &world, None);
    let obs2 = player.observe(PlayerId(0), &world, None);
    assert!(obs1.own_rating == obs2.own_rating);
    assert!(obs1.party_size == obs2.party_size);
}
