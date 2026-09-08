//! Pareto and tradeoff exploration: inspection of Pareto-efficient
//! policies, dominated policies, knee points, objective tradeoffs, and
//! frontier comparisons under different populations.
use crate::comparison::ConditionComparison;
/// A point on the Pareto frontier.
#[derive(Debug, Clone)]
pub struct ParetoPoint {
    pub label: String,
    pub values: Vec<f64>,
}
/// Result of comparing two Pareto frontiers.
#[derive(Debug, Clone)]
pub struct FrontierComparison {
    pub shared_dominated: usize,
    pub only_in_a: usize,
    pub only_in_b: usize,
}
/// Summary of a tradeoff between two objectives.
#[derive(Debug, Clone)]
pub struct TradeoffSummary {
    pub objective_a: String,
    pub objective_b: String,
    pub cost_per_unit_a: f64,
    pub description: String,
}
/// Explorer for Pareto analysis .
pub struct ParetoExplorer {
    pub front: Vec<ParetoPoint>,
    pub dominated: Vec<ParetoPoint>,
}
impl ParetoExplorer {
    /// Build a Pareto explorer from comparison results.
    pub fn from_comparison_results(results: Vec<ConditionComparison>) -> Self {
        let points: Vec<ParetoPoint> = results
            .iter()
            .map(|r| ParetoPoint {
                label: format!("{}_{}", r.condition_a, r.condition_b),
                values: vec![r.mean_a, r.mean_b, r.difference],
            })
            .collect();
        let mut front = Vec::new();
        let mut dominated = Vec::new();
        for point in &points {
            let is_dominated = points.iter().any(|other| {
                other.label != point.label
                    && other.values.iter().zip(&point.values).all(|(a, b)| a <= b)
                    && other.values.iter().zip(&point.values).any(|(a, b)| a < b)
            });
            if is_dominated {
                dominated.push(point.clone());
            } else {
                front.push(point.clone());
            }
        }
        Self { front, dominated }
    }
    /// Find knee points (points with high curvature in the Pareto front).
    pub fn find_knee_points(&self) -> Vec<&ParetoPoint> {
        if self.front.len() < 3 {
            return self.front.iter().collect();
        }
        let mut knees = Vec::new();
        for i in 1..self.front.len() - 1 {
            let prev = &self.front[i - 1];
            let curr = &self.front[i];
            let next = &self.front[i + 1];
            if prev.values.len() >= 2 && curr.values.len() >= 2 && next.values.len() >= 2 {
                let dx1 = curr.values[0] - prev.values[0];
                let dy1 = curr.values[1] - prev.values[1];
                let dx2 = next.values[0] - curr.values[0];
                let dy2 = next.values[1] - curr.values[1];
                let angle1 = dy1.atan2(dx1);
                let angle2 = dy2.atan2(dx2);
                if (angle1 - angle2).abs() > 0.5 {
                    knees.push(curr);
                }
            }
        }
        knees
    }
    /// Quantify the tradeoff between two objectives.
    pub fn identify_tradeoffs(&self, a: usize, b: usize) -> TradeoffSummary {
        let cost = if self.front.len() >= 2 {
            let min_a = self
                .front
                .iter()
                .map(|p| p.values[a])
                .fold(f64::INFINITY, f64::min);
            let max_a = self
                .front
                .iter()
                .map(|p| p.values[a])
                .fold(f64::NEG_INFINITY, f64::max);
            let min_b = self
                .front
                .iter()
                .map(|p| p.values[b])
                .fold(f64::INFINITY, f64::min);
            let max_b = self
                .front
                .iter()
                .map(|p| p.values[b])
                .fold(f64::NEG_INFINITY, f64::max);
            if (max_a - min_a).abs() > 1e-12 {
                (max_b - min_b) / (max_a - min_a)
            } else {
                0.0
            }
        } else {
            0.0
        };
        TradeoffSummary {
            objective_a: format!("obj_{a}"),
            objective_b: format!("obj_{b}"),
            cost_per_unit_a: cost,
            description: format!(
                "Increasing objective {a} by 1 unit costs approximately {cost:.2} units of objective {b}"
            ),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::comparison::ConditionComparison;
    fn sample_comparison() -> ConditionComparison {
        ConditionComparison {
            metric: "queue_time".to_string(),
            condition_a: "batch".to_string(),
            condition_b: "expanding".to_string(),
            mean_a: 5.0,
            mean_b: 3.0,
            difference: -2.0,
            ci: crate::effect::ConfidenceInterval {
                lower: -3.0,
                upper: -1.0,
                conf: 0.95,
                method: crate::effect::CiMethod::WelchT,
            },
            effect_size: None,
        }
    }
    #[test]
    fn pareto_explorer_from_comparisons() {
        let results = vec![
            sample_comparison(),
            ConditionComparison {
                metric: "queue_time".to_string(),
                condition_a: "batch".to_string(),
                condition_b: "strict".to_string(),
                mean_a: 5.0,
                mean_b: 2.0,
                difference: -3.0,
                ci: crate::effect::ConfidenceInterval {
                    lower: -4.0,
                    upper: -2.0,
                    conf: 0.95,
                    method: crate::effect::CiMethod::WelchT,
                },
                effect_size: None,
            },
        ];
        let explorer = ParetoExplorer::from_comparison_results(results);
        assert_eq!(explorer.front.len() + explorer.dominated.len(), 2);
    }
    #[test]
    fn tradeoff_quantification() {
        let explorer = ParetoExplorer {
            front: vec![
                ParetoPoint {
                    label: "a".to_string(),
                    values: vec![1.0, 10.0],
                },
                ParetoPoint {
                    label: "b".to_string(),
                    values: vec![2.0, 8.0],
                },
                ParetoPoint {
                    label: "c".to_string(),
                    values: vec![3.0, 6.0],
                },
            ],
            dominated: vec![],
        };
        let tradeoff = explorer.identify_tradeoffs(0, 1);
        assert!(tradeoff.cost_per_unit_a > 0.0);
    }
}
