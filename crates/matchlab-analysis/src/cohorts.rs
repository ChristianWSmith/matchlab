//! Cohort analysis (spec §14.3): slice per-player metrics by a `CohortFilter`
//! (skill range, archetype, games played, region, etc.) and report per-cohort
//! aggregates.

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
                account_age: 0,
                games_played: 0,
                fatigue: 0.0,
                tilt: 0.0,
                experience: 0,
                is_online: true,
                archetype: archetype.to_string(),
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
}
