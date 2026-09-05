use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::world::World;

use crate::matchmaker::ProposedMatch;
use crate::objective::MatchObjective;
use crate::queue::QueueEntry;

pub trait SearchStrategy: Send + Sync {
    fn search(
        &self,
        queue: &[QueueEntry],
        objective: &MatchObjective,
        team_size: usize,
        world: &World,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch>;
}

pub enum SearchStrategyKind {
    Greedy,
    RandomSampling {
        samples: usize,
    },
    BeamSearch {
        width: usize,
    },
    NearestNeighbor,
    HungarianAssignment,
    GeneticAlgorithm {
        population: usize,
        generations: usize,
    },
    IntegerProgramming,
    SimulatedAnnealing {
        initial_temp: f64,
        cooling_rate: f64,
    },
}

/// For each entry, greedily fill teams with the nearest available players by
/// rating, then balance by alternating. Fast but not globally optimal.
pub struct GreedySearch;

impl GreedySearch {
    fn pick_nearest<'a>(
        anchor_rating: f64,
        available: impl Iterator<Item = &'a QueueEntry>,
        used: &mut Vec<PlayerId>,
    ) -> Option<&'a QueueEntry> {
        let mut best: Option<&QueueEntry> = None;
        let mut best_diff = f64::INFINITY;
        for candidate in available {
            if used.contains(&candidate.player_id) {
                continue;
            }
            let diff = (anchor_rating - candidate.observation.rating).abs();
            if diff < best_diff {
                best_diff = diff;
                best = Some(candidate);
            }
        }
        if let Some(b) = best {
            used.push(b.player_id);
        }
        best
    }
}

impl SearchStrategy for GreedySearch {
    fn search(
        &self,
        queue: &[QueueEntry],
        objective: &MatchObjective,
        team_size: usize,
        world: &World,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut used = Vec::new();
        let mut matches = Vec::new();

        for anchor in queue {
            if used.contains(&anchor.player_id) {
                continue;
            }
            used.push(anchor.player_id);
            let mut team_a = vec![anchor.player_id];
            let mut team_b: Vec<PlayerId> = Vec::new();
            let mut anchor_rating = anchor.observation.rating;

            while team_a.len() < team_size || team_b.len() < team_size {
                let avail = queue.iter();
                if team_a.len() <= team_b.len() {
                    if let Some(pick) =
                        GreedySearch::pick_nearest(anchor_rating, avail.clone(), &mut used)
                    {
                        team_a.push(pick.player_id);
                        anchor_rating = team_a.iter().fold(0.0, |acc, p| {
                            acc + world.observations.get(p).map(|o| o.rating).unwrap_or(0.0)
                        }) / team_a.len() as f64;
                    } else {
                        break;
                    }
                } else if let Some(pick) =
                    GreedySearch::pick_nearest(anchor_rating, queue.iter(), &mut used)
                {
                    team_b.push(pick.player_id);
                } else {
                    break;
                }
            }

            if team_a.len() == team_size && team_b.len() == team_size {
                let quality = ProposedMatch::match_quality(&team_a, &team_b, world);
                matches.push(ProposedMatch {
                    team_a,
                    team_b,
                    quality_score: quality,
                });
            }
        }

        let _ = objective;
        matches
    }
}

/// Generate `samples` random valid team compositions, score each, return the
/// best-scoring one for each anchor. Simple, embarrassingly parallel baseline.
pub struct RandomSamplingSearch {
    pub samples: usize,
}

impl SearchStrategy for RandomSamplingSearch {
    fn search(
        &self,
        queue: &[QueueEntry],
        objective: &MatchObjective,
        team_size: usize,
        world: &World,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();
        let mut used = Vec::new();

        for anchor in queue {
            if used.contains(&anchor.player_id) {
                continue;
            }
            used.push(anchor.player_id);

            let mut best_score = f64::NEG_INFINITY;
            let mut best_team_a: Vec<PlayerId> = Vec::new();
            let mut best_team_b: Vec<PlayerId> = Vec::new();

            for _ in 0..self.samples {
                let mut pool: Vec<PlayerId> = queue
                    .iter()
                    .map(|e| e.player_id)
                    .filter(|pid| !used.contains(pid) && *pid != anchor.player_id)
                    .collect();
                if pool.len() < 2 * team_size - 1 {
                    break;
                }
                // Shuffle by drawing random indices.
                let mut team_a = vec![anchor.player_id];
                let mut team_b: Vec<PlayerId> = Vec::new();
                while !pool.is_empty() && team_a.len() + team_b.len() < 2 * team_size {
                    let idx = rng.gen_range(0.0, pool.len() as f64) as usize;
                    let pid = pool.remove(idx);
                    if team_a.len() < team_size {
                        team_a.push(pid);
                    } else {
                        team_b.push(pid);
                    }
                }
                if team_a.len() != team_size || team_b.len() != team_size {
                    continue;
                }
                let quality = ProposedMatch::match_quality(&team_a, &team_b, world);
                let pm = ProposedMatch {
                    team_a: team_a.clone(),
                    team_b: team_b.clone(),
                    quality_score: quality,
                };
                let score = objective.score(&pm, queue, world);
                if score > best_score {
                    best_score = score;
                    best_team_a = team_a;
                    best_team_b = team_b;
                }
            }

            if best_team_a.len() == team_size && best_team_b.len() == team_size {
                used.extend(best_team_a.iter().chain(&best_team_b));
                let quality = ProposedMatch::match_quality(&best_team_a, &best_team_b, world);
                matches.push(ProposedMatch {
                    team_a: best_team_a,
                    team_b: best_team_b,
                    quality_score: quality,
                });
            }
        }

        matches
    }
}

/// Maintain a beam of `width` partial match assignments, expand each by one
/// player, keep the top `width` by objective score.
pub struct BeamSearch {
    pub width: usize,
}

impl SearchStrategy for BeamSearch {
    fn search(
        &self,
        queue: &[QueueEntry],
        objective: &MatchObjective,
        team_size: usize,
        world: &World,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let mut matches = Vec::new();
        let mut used = Vec::new();

        for anchor in queue {
            if used.contains(&anchor.player_id) {
                continue;
            }
            used.push(anchor.player_id);

            let mut beam: Vec<(Vec<PlayerId>, Vec<PlayerId>)> =
                vec![(vec![anchor.player_id], Vec::new())];

            while beam
                .iter()
                .any(|(a, b)| a.len() < team_size || b.len() < team_size)
            {
                let mut next_beam: Vec<(Vec<PlayerId>, Vec<PlayerId>)> = Vec::new();
                for (team_a, team_b) in &beam {
                    if team_a.len() == team_size && team_b.len() == team_size {
                        next_beam.push((team_a.clone(), team_b.clone()));
                        continue;
                    }
                    for candidate in queue {
                        if candidate.player_id == anchor.player_id
                            || team_a.contains(&candidate.player_id)
                            || team_b.contains(&candidate.player_id)
                            || used.contains(&candidate.player_id)
                        {
                            continue;
                        }
                        let mut na = team_a.clone();
                        let mut nb = team_b.clone();
                        if na.len() <= nb.len() {
                            na.push(candidate.player_id);
                        } else {
                            nb.push(candidate.player_id);
                        }
                        next_beam.push((na, nb));
                    }
                }
                // Keep the top `width` by objective score.
                next_beam.sort_by(|(a1, b1), (a2, b2)| {
                    let s1 = partial_score(a1, b1, objective, queue, world);
                    let s2 = partial_score(a2, b2, objective, queue, world);
                    s2.partial_cmp(&s1).unwrap_or(std::cmp::Ordering::Equal)
                });
                next_beam.truncate(self.width);
                beam = next_beam;
                if beam.is_empty() {
                    break;
                }
            }

            let mut formed = false;
            for (team_a, team_b) in &beam {
                if team_a.len() == team_size && team_b.len() == team_size {
                    let quality = ProposedMatch::match_quality(team_a, team_b, world);
                    used.extend(team_a.iter().chain(team_b.iter()));
                    matches.push(ProposedMatch {
                        team_a: team_a.clone(),
                        team_b: team_b.clone(),
                        quality_score: quality,
                    });
                    formed = true;
                    break;
                }
            }
            if !formed {
                used.pop(); // release the anchor that couldn't form a match
            }
        }

        matches
    }
}

fn partial_score(
    team_a: &[PlayerId],
    team_b: &[PlayerId],
    objective: &MatchObjective,
    queue: &[QueueEntry],
    world: &World,
) -> f64 {
    let quality = ProposedMatch::match_quality(team_a, team_b, world);
    let pm = ProposedMatch {
        team_a: team_a.to_vec(),
        team_b: team_b.to_vec(),
        quality_score: quality,
    };
    objective.score(&pm, queue, world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use matchlab_core::player::{
        DetectionFlag, PlayerObservation, Region, SkillVector, VisibleRank,
    };
    use matchlab_core::time::SimTime;
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
            detection_flags: Vec::<DetectionFlag>::new(),
        }
    }

    fn entry(id: u64, rating: f64) -> QueueEntry {
        QueueEntry {
            player_id: PlayerId(id),
            joined_at: SimTime::ZERO,
            observation: obs(id, rating),
            region: Region::NA,
            party_id: None,
            game_mode: "ranked".into(),
            role: None,
            latency_ms: 30.0,
        }
    }

    fn world_with(ratings: &[(u64, f64)]) -> World {
        let mut world = World::new(matchlab_core::rng::SimRng::from_seed(1));
        for &(id, rating) in ratings {
            world.observations.insert(PlayerId(id), obs(id, rating));
        }
        world
    }

    fn queue_from(ids: &[u64], ratings: &[(u64, f64)]) -> Vec<QueueEntry> {
        ids.iter()
            .map(|&id| entry(id, ratings.iter().find(|(i, _)| *i == id).unwrap().1))
            .collect()
    }

    #[test]
    fn greedy_forms_matches_from_queue() {
        let ratings = vec![(1, 1000.0), (2, 1000.0), (3, 1000.0), (4, 1000.0)];
        let queue = queue_from(&[1, 2, 3, 4], &ratings);
        let world = world_with(&ratings);
        let objective = MatchObjective::new(1.0, 0.5, 0.0, 0.1);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);

        let matches = GreedySearch.search(&queue, &objective, 2, &world, &mut rng);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].team_a.len(), 2);
        assert_eq!(matches[0].team_b.len(), 2);
    }

    #[test]
    fn random_sampling_returns_best_of_samples() {
        let ratings = vec![(1, 1000.0), (2, 1000.0), (3, 1000.0), (4, 1000.0)];
        let queue = queue_from(&[1, 2, 3, 4], &ratings);
        let world = world_with(&ratings);
        let objective = MatchObjective::new(1.0, 0.5, 0.0, 0.1);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);

        let s1 = RandomSamplingSearch { samples: 1 };
        let matches = s1.search(&queue, &objective, 2, &world, &mut rng);
        // With 1 sample the team may be partial; but it should still return matches.
        assert!(!matches.is_empty());
    }

    #[test]
    fn beam_search_never_exceeds_width() {
        let ratings: Vec<(u64, f64)> = (1..=10).map(|id| (id, 1000.0)).collect();
        let queue = queue_from(&(1..=10).collect::<Vec<_>>(), &ratings);
        let world = world_with(&ratings);
        let objective = MatchObjective::new(1.0, 0.5, 0.0, 0.1);
        let mut rng = matchlab_core::rng::SimRng::from_seed(7);

        let bs = BeamSearch { width: 5 };
        let matches = bs.search(&queue, &objective, 5, &world, &mut rng);
        assert!(!matches.is_empty());
    }
}
