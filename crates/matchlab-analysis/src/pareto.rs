//! Pareto frontier (spec §14.2): the set of non-dominated points across a
//! multi-objective comparison, e.g. "best match quality for a given queue
//! time" across experiment configs.

pub struct ParetoPoint {
    pub label: String,
    pub values: Vec<f64>,
}

pub fn pareto_front<'a>(
    points: &'a [ParetoPoint],
    higher_is_better: &[bool],
) -> Vec<&'a ParetoPoint> {
    points
        .iter()
        .filter(|p| !points.iter().any(|other| dominates(other, p, higher_is_better)))
        .collect()
}

fn dominates(a: &ParetoPoint, b: &ParetoPoint, higher_is_better: &[bool]) -> bool {
    let mut strictly_better = false;
    for (i, &hib) in higher_is_better.iter().enumerate() {
        let (a_val, b_val) = if hib {
            (a.values[i], b.values[i])
        } else {
            (-a.values[i], -b.values[i])
        };
        if a_val < b_val {
            return false;
        }
        if a_val > b_val {
            strictly_better = true;
        }
    }
    strictly_better
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(label: &str, values: Vec<f64>) -> ParetoPoint {
        ParetoPoint {
            label: label.to_string(),
            values,
        }
    }

    #[test]
    fn dominated_point_is_excluded() {
        let points = vec![
            pt("a", vec![1.0, 1.0]),
            pt("b", vec![2.0, 2.0]), // dominates a on both (higher better)
            pt("c", vec![1.5, 1.5]),
        ];
        let front = pareto_front(&points, &[true, true]);
        assert_eq!(front.len(), 1);
        assert_eq!(front[0].label, "b");
    }

    #[test]
    fn all_non_dominated_returns_all() {
        let points = vec![
            pt("a", vec![1.0, 3.0]),
            pt("b", vec![3.0, 1.0]),
            pt("c", vec![2.0, 2.0]),
        ];
        let front = pareto_front(&points, &[true, true]);
        assert_eq!(front.len(), 3);
    }

    #[test]
    fn mixed_higher_lower_is_better() {
        // quality (higher better), queue_time (lower better)
        let points = vec![
            pt("a", vec![0.5, 50.0]),
            pt("b", vec![0.9, 10.0]), // better both
            pt("c", vec![0.8, 5.0]),  // lower queue but slightly lower quality
        ];
        let front = pareto_front(&points, &[true, false]);
        // b dominates a; c is non-dominated (better queue time than b).
        let labels: Vec<&str> = front.iter().map(|p| p.label.as_str()).collect();
        assert!(labels.contains(&"b"));
        assert!(labels.contains(&"c"));
        assert!(!labels.contains(&"a"));
    }
}