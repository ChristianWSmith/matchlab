//! Cohort analysis (spec §14.3): slice per-player metrics by a `CohortFilter`
//! (skill range, archetype, games played, region, etc.) and report per-cohort
//! aggregates. Ticket extends this to replication-level cohort effects.
use crate::effect::{EffectSize, effect_sizes};
use crate::hierarchy::ReplicationScalar;
use matchlab_core::player::PlayerId;
use matchlab_core::world::World;
use matchlab_metrics::CohortFilter;
use matchlab_metrics::MetricsEngine;
use std::collections::HashMap;
pub struct CohortResult {
    pub name: String,
    pub player_count: usize,
    pub metrics: HashMap<String, matchlab_metrics::MetricResult>,
}
pub fn analyze_cohort(
    name: &str,
    filter: &CohortFilter,
    world: &World,
    _full_metrics: &MetricsEngine,
) -> CohortResult {
    let player_ids: Vec<PlayerId> = world
        .players
        .values()
        .filter(|reality| filter.matches(reality))
        .map(|reality| reality.id)
        .collect();
    let mut metrics: HashMap<String, matchlab_metrics::MetricResult> = HashMap::new();
    metrics.insert(
        "rating_accuracy".to_string(),
        cohort_rating_accuracy(&player_ids, world),
    );
    CohortResult {
        name: name.to_string(),
        player_count: player_ids.len(),
        metrics,
    }
}
fn cohort_rating_accuracy(
    player_ids: &[PlayerId],
    world: &World,
) -> matchlab_metrics::MetricResult {
    let errors: Vec<f64> = player_ids
        .iter()
        .filter_map(|pid| {
            let obs = world.observations.get(pid)?;
            let reality = world.players.get(pid)?;
            Some((obs.rating - reality.skill.overall()).abs())
        })
        .collect();
    if errors.is_empty() {
        return matchlab_metrics::MetricResult::Scalar(0.0);
    }
    matchlab_metrics::MetricResult::Summary {
        mean: errors.iter().sum::<f64>() / errors.len() as f64,
        median: 0.0,
        p75: 0.0,
        p90: 0.0,
        p95: 0.0,
        p99: 0.0,
        stddev: 0.0,
    }
}
/// Per-cohort effect heterogeneity result : one row per (cohort, metric).
#[derive(Debug, Clone)]
pub struct HeterogeneityResult {
    pub cohort: String,
    pub effect: EffectSize,
    pub n_replications: usize,
}
/// Estimate per-cohort effects from pre-computed per-cohort replication
/// scalars. Each entry in `cohort_scalars` is `(cohort_name, control_scalars,
/// treatment_scalars)`.
///
/// Empty cohorts (no scalarizable observations) are skipped. The caller is
/// responsible for computing cohort-restricted scalars from the per-replication
/// data (the analysis layer doesn't have player-level cohort membership).
pub fn analyze_heterogeneity(
    cohort_scalars: &[(String, Vec<ReplicationScalar>, Vec<ReplicationScalar>)],
    paired: bool,
    conf: f64,
    seed: u64,
) -> Vec<HeterogeneityResult> {
    let mut results = Vec::new();
    for (i, (cohort, control, treatment)) in cohort_scalars.iter().enumerate() {
        if control.is_empty() || treatment.is_empty() {
            continue;
        }
        let cohort_seed = matchlab_experiments::seed::derive(seed, i as u64);
        if let Ok(es) = effect_sizes(control, treatment, paired, conf, cohort_seed) {
            results.push(HeterogeneityResult {
                cohort: cohort.clone(),
                effect: es,
                n_replications: control.len(),
            });
        }
    }
    results
}
#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{
        PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
    };
    use matchlab_core::rng::SimRng;
    use std::collections::VecDeque;
    fn obs(id: u64, rating: f64) -> PlayerObservation {
        PlayerObservation {
            id: PlayerId(id),
            rating,
            hidden_mmr: rating,
            visible_rank: VisibleRank {
                tier: "unranked".into(),
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
            game_mode: "ranked".into(),
            skill_vector: SkillVector::one_dimensional(rating),
            detection_flags: Vec::new(),
            role: None,
        }
    }
    fn add(world: &mut World, id: u64, rating: f64, skill: f64, archetype: &str) {
        world.observations.insert(PlayerId(id), obs(id, rating));
        world.players.insert(
            PlayerId(id),
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
                location: matchlab_core::player::GeoLocation::new(Region::NA, 0.0, 0.0),
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: archetype.to_string(),
                role: None,
            },
        );
    }
    #[test]
    fn skill_range_cohort_filters_players() {
        let mut world = World::new(SimRng::from_seed(1));
        add(&mut world, 1, 1000.0, 1000.0, "stable");
        add(&mut world, 2, 1500.0, 1500.0, "stable");
        add(&mut world, 3, 1100.0, 1100.0, "stable");
        let filter = CohortFilter::SkillRange(900.0, 1200.0);
        let result = analyze_cohort("mid", &filter, &world, &MetricsEngine::new());
        assert_eq!(result.player_count, 2);
        assert_eq!(result.name, "mid");
    }
    #[test]
    fn archetype_cohort_filters_players() {
        let mut world = World::new(SimRng::from_seed(2));
        add(&mut world, 1, 1500.0, 1500.0, "smurf");
        add(&mut world, 2, 1000.0, 1000.0, "stable");
        let filter = CohortFilter::Archetype("smurf".to_string());
        let result = analyze_cohort("smurfs", &filter, &world, &MetricsEngine::new());
        assert_eq!(result.player_count, 1);
        assert!(result.metrics.contains_key("rating_accuracy"));
    }
    #[test]
    fn heterogeneity_recovers_different_cohort_effects() {
        use crate::hierarchy::per_replication;
        use matchlab_metrics::MetricResult;
        let to_s = |v: f64| per_replication::from_metric(&MetricResult::Scalar(v)).unwrap();
        let a_ctrl: Vec<ReplicationScalar> = (0..10).map(|i| to_s(100.0 + i as f64)).collect();
        let a_trt: Vec<ReplicationScalar> = (0..10).map(|i| to_s(103.0 + i as f64)).collect();
        let b_ctrl: Vec<ReplicationScalar> = (0..10).map(|i| to_s(100.0 + i as f64)).collect();
        let b_trt: Vec<ReplicationScalar> = (0..10).map(|i| to_s(120.0 + i as f64)).collect();
        let cohorts = vec![
            ("cohort_a".to_string(), a_ctrl, a_trt),
            ("cohort_b".to_string(), b_ctrl, b_trt),
        ];
        let results = analyze_heterogeneity(&cohorts, true, 0.95, 42);
        assert_eq!(results.len(), 2);
        let a = results.iter().find(|r| r.cohort == "cohort_a").unwrap();
        let b = results.iter().find(|r| r.cohort == "cohort_b").unwrap();
        assert!((a.effect.mean_delta - 3.0).abs() < 1e-9, "cohort A: Δ≈3");
        assert!((b.effect.mean_delta - 20.0).abs() < 1e-9, "cohort B: Δ≈20");
        assert!(
            b.effect.mean_delta > a.effect.mean_delta,
            "B has larger effect"
        );
    }
    #[test]
    fn empty_cohort_is_skipped() {
        let cohorts: Vec<(String, Vec<ReplicationScalar>, Vec<ReplicationScalar>)> =
            vec![("empty".to_string(), vec![], vec![])];
        let results = analyze_heterogeneity(&cohorts, true, 0.95, 42);
        assert!(results.is_empty(), "empty cohort produces no results");
    }
    #[test]
    fn heterogeneity_is_deterministic() {
        use crate::hierarchy::per_replication;
        use matchlab_metrics::MetricResult;
        let to_s = |v: f64| per_replication::from_metric(&MetricResult::Scalar(v)).unwrap();
        let ctrl: Vec<ReplicationScalar> = (0..5).map(|i| to_s(100.0 + i as f64)).collect();
        let trt: Vec<ReplicationScalar> = (0..5).map(|i| to_s(110.0 + i as f64)).collect();
        let cohorts = vec![("test".to_string(), ctrl, trt)];
        let r1 = analyze_heterogeneity(&cohorts, true, 0.95, 42);
        let r2 = analyze_heterogeneity(&cohorts, true, 0.95, 42);
        assert_eq!(r1[0].effect.mean_delta, r2[0].effect.mean_delta);
        assert_eq!(r1[0].effect.ci_lo, r2[0].effect.ci_lo);
        assert_eq!(r1[0].effect.ci_hi, r2[0].effect.ci_hi);
    }
}
