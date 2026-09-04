//! Population generation from archetype configs (spec §5.8).
//!
//! Each player is drawn from an archetype's skill distribution; the true skill
//! feeds `PlayerReality` and, via the observation binding, the simulation's
//! visibility of ground truth:
//!
//! - `PlayerObservation.rating` is the archetype's `initial_rating` if set,
//!   else the sampled skill — the only field rating/matchmaking systems read.
//! - `PlayerObservation.skill_vector` and `hidden_mmr` carry the *true* skill
//!   so the outcome model (the simulator of reality) can decide match winners
//!   from ground truth even after ratings diverge. Algorithms must not read
//!   these fields.
//!
//! The `initial_rating` override seeds the smurf-like mismatch — the two
//! representations diverge exactly as the design intends.

use matchlab_core::player::{
    DetectionFlag, PlayerId, PlayerObservation, PlayerReality, Region, SkillVector, VisibleRank,
};
use matchlab_core::rng::SimRng;
use std::collections::VecDeque;

use crate::archetype::{ArchetypeConfig, DistributionConfig};

pub struct PopulationConfig {
    pub size: u64,
    pub archetypes: Vec<ArchetypeConfig>,
}

pub struct PopulationGenerator;

impl PopulationGenerator {
    pub fn generate(
        config: &PopulationConfig,
        rng: &mut SimRng,
    ) -> (Vec<PlayerReality>, Vec<PlayerObservation>) {
        let counts = allocate_counts(config);
        let mut realities = Vec::with_capacity(config.size as usize);
        let mut observations = Vec::with_capacity(config.size as usize);
        let mut id_counter = 0u64;

        for (archetype, &count) in config.archetypes.iter().zip(counts.iter()) {
            for _ in 0..count {
                let id = PlayerId(id_counter);
                id_counter += 1;

                let true_skill = sample_distribution(&archetype.skill_distribution, rng);
                let rating = archetype.initial_rating.unwrap_or(true_skill);

                realities.push(PlayerReality {
                    id,
                    skill: SkillVector::one_dimensional(true_skill),
                    skill_volatility: archetype.skill_volatility,
                    improvement_rate: archetype.improvement_rate,
                    consistency: (1.0 - archetype.skill_volatility / 100.0).max(0.0),
                    play_frequency: archetype.play_frequency,
                    session_length: archetype.session_length,
                    quit_probability: archetype.quit_probability,
                    party_id: None,
                    region: Region::NA,
                    account_age: 0,
                    games_played: 0,
                    fatigue: 0.0,
                    tilt: 0.0,
                    experience: 0,
                    is_online: true,
                    archetype: archetype.name.clone(),
                });

                observations.push(PlayerObservation {
                    id,
                    rating,
                    hidden_mmr: true_skill,
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
                    skill_vector: SkillVector::one_dimensional(true_skill),
                    detection_flags: Vec::<DetectionFlag>::new(),
                });
            }
        }

        (realities, observations)
    }
}

/// Convert proportions into integer counts that sum exactly to `size` using the
/// largest-remainder method (naive `truncate` can undershoot by one per archetype).
fn allocate_counts(config: &PopulationConfig) -> Vec<u64> {
    if config.archetypes.is_empty() {
        return Vec::new();
    }
    let size = config.size;
    let mut floors: Vec<u64> = Vec::with_capacity(config.archetypes.len());
    let mut remainders: Vec<f64> = Vec::with_capacity(config.archetypes.len());
    let mut total_floor = 0u64;

    for archetype in &config.archetypes {
        let exact = archetype.proportion * size as f64;
        let floor = exact.floor() as u64;
        total_floor += floor;
        floors.push(floor);
        remainders.push(exact - floor as f64);
    }

    let mut leftover = size.saturating_sub(total_floor) as usize;
    let mut order: Vec<usize> = (0..config.archetypes.len()).collect();
    order.sort_by(|&a, &b| {
        remainders[b]
            .partial_cmp(&remainders[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for idx in order {
        if leftover == 0 {
            break;
        }
        floors[idx] += 1;
        leftover -= 1;
    }

    floors
}

fn sample_distribution(dist: &DistributionConfig, rng: &mut SimRng) -> f64 {
    match dist {
        DistributionConfig::Normal { mean, stddev } => rng.sample_normal(*mean, *stddev),
        DistributionConfig::Uniform { low, high } => rng.gen_range(*low, *high),
        DistributionConfig::LogNormal { mean, stddev } => rng.sample_normal(*mean, *stddev).exp(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable_archetype() -> ArchetypeConfig {
        ArchetypeConfig {
            name: "stable".to_string(),
            proportion: 1.0,
            skill_distribution: DistributionConfig::Normal {
                mean: 1000.0,
                stddev: 250.0,
            },
            skill_volatility: 5.0,
            improvement_rate: 0.0,
            play_frequency: 0.8,
            session_length: 1800.0,
            quit_probability: 0.01,
            initial_rating: None,
        }
    }

    fn mean(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    fn stddev(values: &[f64], m: f64) -> f64 {
        let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
        var.sqrt()
    }

    #[test]
    fn thousand_player_stable_archetype_matches_mean_and_stddev() {
        let config = PopulationConfig {
            size: 1000,
            archetypes: vec![stable_archetype()],
        };
        let mut rng = SimRng::from_seed(42);
        let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);

        assert_eq!(realities.len(), 1000);
        assert_eq!(observations.len(), 1000);

        // Acceptance criterion: observed ratings mean ≈ 1000, stddev ≈ 250.
        let observed: Vec<f64> = observations.iter().map(|o| o.rating).collect();
        let obs_mean = mean(&observed);
        let obs_stddev = stddev(&observed, obs_mean);
        // ±5% tolerance of the requested mean/stddev.
        assert!((obs_mean - 1000.0).abs() < 50.0, "mean drifted: {obs_mean}");
        assert!(
            (obs_stddev - 250.0).abs() < 12.5,
            "stddev drifted: {obs_stddev}"
        );

        // For a stable archetype with no initial_rating, observed rating equals
        // true skill.
        let skills: Vec<f64> = realities.iter().map(|r| r.skill.overall()).collect();
        let m = mean(&skills);
        let sd = stddev(&skills, m);
        assert!((m - 1000.0).abs() < 50.0, "reality mean drifted: {m}");
        assert!((sd - 250.0).abs() < 12.5, "reality stddev drifted: {sd}");
    }

    #[test]
    fn initial_rating_overrides_observation_while_true_skill_stays_sampled() {
        let mut archetype = stable_archetype();
        archetype.initial_rating = Some(700.0);
        let config = PopulationConfig {
            size: 200,
            archetypes: vec![archetype],
        };
        let mut rng = SimRng::from_seed(7);
        let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);

        for (reality, obs) in realities.iter().zip(observations.iter()) {
            assert_eq!(
                obs.rating, 700.0,
                "observation rating must equal initial_rating"
            );
            assert!(
                (reality.skill.overall() - 1000.0).abs() < 300.0 * 4.0,
                "true skill should be sampled around the distribution mean"
            );
            assert!(obs.rating != reality.skill.overall());
            // skill_vector / hidden_mmr carry ground truth so the outcome
            // model simulates reality: equal to the true skill, distinct from
            // the visible rating.
            assert_eq!(obs.skill_vector.overall(), reality.skill.overall());
            assert_eq!(obs.hidden_mmr, reality.skill.overall());
            assert_ne!(obs.skill_vector.overall(), obs.rating);
        }
    }

    #[test]
    fn skill_vector_mirrors_true_skill_when_no_initial_rating() {
        let config = PopulationConfig {
            size: 300,
            archetypes: vec![stable_archetype()],
        };
        let mut rng = SimRng::from_seed(21);
        let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
        for (reality, obs) in realities.iter().zip(observations.iter()) {
            assert_eq!(obs.skill_vector.overall(), reality.skill.overall());
            assert_eq!(obs.hidden_mmr, reality.skill.overall());
            assert_eq!(obs.rating, reality.skill.overall());
        }
    }

    #[test]
    fn proportions_convert_to_integer_counts_that_sum_to_size() {
        let names = ["a", "b", "c"];
        let props = [0.33, 0.33, 0.34];
        let archetypes: Vec<ArchetypeConfig> = names
            .iter()
            .zip(props.iter())
            .map(|(name, prop)| ArchetypeConfig {
                name: name.to_string(),
                proportion: *prop,
                skill_distribution: DistributionConfig::Normal {
                    mean: 500.0,
                    stddev: 50.0,
                },
                skill_volatility: 5.0,
                improvement_rate: 0.0,
                play_frequency: 0.8,
                session_length: 1800.0,
                quit_probability: 0.01,
                initial_rating: None,
            })
            .collect();

        let config = PopulationConfig {
            size: 100,
            archetypes,
        };
        let counts = allocate_counts(&config);
        assert_eq!(counts.iter().sum::<u64>(), 100);

        let mut rng = SimRng::from_seed(11);
        let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
        assert_eq!(realities.len(), config.size as usize);
        assert_eq!(observations.len(), config.size as usize);
    }

    #[test]
    fn generation_is_deterministic_given_seed() {
        let config = PopulationConfig {
            size: 500,
            archetypes: vec![stable_archetype()],
        };
        let mut rng_a = SimRng::from_seed(99);
        let mut rng_b = SimRng::from_seed(99);
        let (ra, oa) = PopulationGenerator::generate(&config, &mut rng_a);
        let (rb, ob) = PopulationGenerator::generate(&config, &mut rng_b);

        assert_eq!(ra.len(), rb.len());
        for (a, b) in ra.iter().zip(rb.iter()) {
            assert_eq!(a.skill.overall(), b.skill.overall());
            assert_eq!(a.id, b.id);
        }
        for (a, b) in oa.iter().zip(ob.iter()) {
            assert_eq!(a.rating, b.rating);
            assert_eq!(a.id, b.id);
        }
    }

    #[test]
    fn empty_archetypes_produce_empty_population() {
        let config = PopulationConfig {
            size: 10,
            archetypes: Vec::new(),
        };
        let mut rng = SimRng::from_seed(1);
        let (realities, observations) = PopulationGenerator::generate(&config, &mut rng);
        assert!(realities.is_empty());
        assert!(observations.is_empty());
    }
}
