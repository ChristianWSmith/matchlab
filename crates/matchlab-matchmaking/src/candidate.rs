//! Candidate match generation: separates "what matches could
//! the system consider" from "how does it choose among those matches."
use crate::matchmaker::ProposedMatch;
use crate::queue::Queue;
use matchlab_core::match_::TeamComposition;
use matchlab_core::player::PlayerId;
use matchlab_core::rng::SimRng;
use matchlab_core::time::SimTime;
use matchlab_core::world::World;
/// Generate candidate matches from the queue.
pub trait CandidateGenerator: Send + Sync {
    fn generate_candidates(
        &self,
        queue: &Queue,
        teams: &TeamComposition,
        world: &World,
        now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch>;
}
/// Exhaustive generator: all valid combinations (small queues only).
pub struct ExhaustiveGenerator;
impl CandidateGenerator for ExhaustiveGenerator {
    fn generate_candidates(
        &self,
        queue: &Queue,
        teams: &TeamComposition,
        _world: &World,
        _now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let entries = queue.entries();
        let size_a = teams.team_size_a;
        let size_b = teams.team_size_b;
        let total = size_a + size_b;
        if entries.len() < total {
            return Vec::new();
        }
        let mut candidates = Vec::new();
        let indices: Vec<usize> = (0..entries.len()).collect();
        for combo in combinations(&indices, total) {
            let team_a: Vec<PlayerId> = combo[..size_a]
                .iter()
                .map(|&i| entries[i].player_id)
                .collect();
            let team_b: Vec<PlayerId> = combo[size_a..]
                .iter()
                .map(|&i| entries[i].player_id)
                .collect();
            candidates.push(ProposedMatch {
                team_a,
                team_b,
                quality_score: 0.0,
            });
        }
        candidates
    }
}
/// Greedy nearest generator: fill teams by nearest-by-rating.
pub struct GreedyNearestGenerator {
    pub max_candidates: usize,
}
impl CandidateGenerator for GreedyNearestGenerator {
    fn generate_candidates(
        &self,
        queue: &Queue,
        teams: &TeamComposition,
        world: &World,
        _now: SimTime,
        _rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let entries = queue.entries();
        let size_a = teams.team_size_a;
        let size_b = teams.team_size_b;
        let total = size_a + size_b;
        if entries.len() < total {
            return Vec::new();
        }
        let mut sorted: Vec<(usize, f64)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let rating = world
                    .observations
                    .get(&e.player_id)
                    .map(|o| o.rating)
                    .unwrap_or(1000.0);
                (i, rating)
            })
            .collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let mut candidates = Vec::new();
        let max = self.max_candidates.min(sorted.len() / total);
        for chunk in sorted.chunks(total).take(max) {
            if chunk.len() < total {
                break;
            }
            let team_a: Vec<PlayerId> = chunk[..size_a]
                .iter()
                .map(|&(i, _)| entries[i].player_id)
                .collect();
            let team_b: Vec<PlayerId> = chunk[size_a..]
                .iter()
                .map(|&(i, _)| entries[i].player_id)
                .collect();
            candidates.push(ProposedMatch {
                team_a,
                team_b,
                quality_score: 0.0,
            });
        }
        candidates
    }
}
/// Random sampling generator: draw random compositions, keep best.
pub struct RandomSamplingGenerator {
    pub samples: usize,
}
impl CandidateGenerator for RandomSamplingGenerator {
    fn generate_candidates(
        &self,
        queue: &Queue,
        teams: &TeamComposition,
        _world: &World,
        _now: SimTime,
        rng: &mut SimRng,
    ) -> Vec<ProposedMatch> {
        let entries = queue.entries();
        let size_a = teams.team_size_a;
        let size_b = teams.team_size_b;
        let total = size_a + size_b;
        if entries.len() < total {
            return Vec::new();
        }
        let mut candidates = Vec::new();
        for _ in 0..self.samples {
            let mut indices: Vec<usize> = (0..entries.len()).collect();
            for i in (1..indices.len()).rev() {
                let j = (rng.gen_u64() % (i + 1) as u64) as usize;
                indices.swap(i, j);
            }
            let team_a: Vec<PlayerId> = indices[..size_a]
                .iter()
                .map(|&i| entries[i].player_id)
                .collect();
            let team_b: Vec<PlayerId> = indices[size_a..total]
                .iter()
                .map(|&i| entries[i].player_id)
                .collect();
            candidates.push(ProposedMatch {
                team_a,
                team_b,
                quality_score: 0.0,
            });
        }
        candidates
    }
}
/// Generate all combinations of `k` elements from a slice.
fn combinations<T: Clone>(items: &[T], k: usize) -> Vec<Vec<T>> {
    if k == 0 {
        return vec![vec![]];
    }
    if items.is_empty() || k > items.len() {
        return vec![];
    }
    let mut result = Vec::new();
    let first = &items[0];
    let rest = &items[1..];
    for mut combo in combinations(rest, k - 1) {
        combo.insert(0, first.clone());
        result.push(combo);
    }
    result.extend(combinations(rest, k));
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combinations_basic() {
        let items = vec![1, 2, 3];
        let combos = combinations(&items, 2);
        assert_eq!(combos.len(), 3);
    }
    #[test]
    fn combinations_empty() {
        let items: Vec<i32> = vec![];
        let combos = combinations(&items, 2);
        assert!(combos.is_empty());
    }
    #[test]
    fn combinations_k_zero() {
        let items = vec![1, 2, 3];
        let combos = combinations(&items, 0);
        assert_eq!(combos, vec![vec![] as Vec<i32>]);
    }
    #[test]
    fn combinations_k_exceeds_len() {
        let items = vec![1, 2];
        let combos = combinations(&items, 5);
        assert!(combos.is_empty());
    }
}
