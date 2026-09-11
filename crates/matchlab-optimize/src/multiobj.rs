#[derive(Debug, Clone, Default)]
pub struct ParetoFront {
    pub indices: Vec<usize>,
}

impl ParetoFront {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, objectives: &[Vec<f64>], directions: &[bool]) {
        let n = objectives.len();
        let mut dominated = vec![false; n];

        for i in 0..n {
            for j in 0..n {
                if i == j || dominated[i] {
                    continue;
                }
                if dominates(&objectives[j], &objectives[i], directions) {
                    dominated[i] = true;
                    break;
                }
            }
        }

        self.indices = (0..n).filter(|&i| !dominated[i]).collect();
    }

    pub fn front_objectives<'a>(&self, objectives: &'a [Vec<f64>]) -> Vec<&'a Vec<f64>> {
        self.indices.iter().map(|&i| &objectives[i]).collect()
    }
}

pub fn dominates(a: &[f64], b: &[f64], directions: &[bool]) -> bool {
    let mut at_least_one_better = false;
    for (i, (&ai, &bi)) in a.iter().zip(b.iter()).enumerate() {
        let (ai_adj, bi_adj) = if directions[i] { (ai, bi) } else { (-ai, -bi) };
        if ai_adj < bi_adj {
            return false;
        }
        if ai_adj > bi_adj {
            at_least_one_better = true;
        }
    }
    at_least_one_better
}

pub fn hypervolume_contribution(
    point: &[f64],
    front: &[Vec<f64>],
    directions: &[bool],
    reference: &[f64],
) -> f64 {
    let n = point.len();
    let mut volume = 1.0;

    for i in 0..n {
        let p_val = if directions[i] { point[i] } else { -point[i] };
        let r_val = if directions[i] {
            reference[i]
        } else {
            -reference[i]
        };

        let mut max_covered = p_val;
        for other in front {
            let o_val = if directions[i] { other[i] } else { -other[i] };
            if o_val <= p_val && o_val > max_covered {
                max_covered = o_val;
            }
        }

        let contribution = (r_val - max_covered).max(0.0);
        volume *= contribution;
    }

    volume
}

pub fn nadir_point(objectives: &[Vec<f64>], directions: &[bool]) -> Vec<f64> {
    if objectives.is_empty() {
        return vec![];
    }
    let n_obj = objectives[0].len();
    let mut nadir = vec![f64::NEG_INFINITY; n_obj];

    for obj in objectives {
        for (i, (&val, nadir_val)) in obj.iter().zip(nadir.iter_mut()).enumerate() {
            let adj = if directions[i] { -val } else { val };
            if adj > *nadir_val {
                *nadir_val = adj;
            }
        }
    }

    nadir.iter().map(|&v| -v).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominates_basic() {
        let a = vec![1.0, 2.0];
        let b = vec![0.5, 1.5];
        let dirs = vec![true, true];
        assert!(dominates(&a, &b, &dirs));
        assert!(!dominates(&b, &a, &dirs));
    }

    #[test]
    fn dominates_incomparable() {
        let a = vec![1.0, 0.5];
        let b = vec![0.5, 1.0];
        let dirs = vec![true, true];
        assert!(!dominates(&a, &b, &dirs));
        assert!(!dominates(&b, &a, &dirs));
    }

    #[test]
    fn dominates_with_minimize() {
        let a = vec![0.5, 2.0];
        let b = vec![1.0, 3.0];
        let dirs = vec![false, false];
        assert!(dominates(&a, &b, &dirs));
    }

    #[test]
    fn pareto_front_update() {
        let mut pf = ParetoFront::new();
        let objectives = vec![
            vec![1.0, 3.0],
            vec![2.0, 2.0],
            vec![3.0, 1.0],
            vec![1.5, 2.5],
        ];
        let dirs = vec![true, true];
        pf.update(&objectives, &dirs);
        assert_eq!(pf.indices.len(), 4);
    }
}
