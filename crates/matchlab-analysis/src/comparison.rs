//! Comparison and exploration: first-class comparison operations
//! for matchmaking policies, factor levels, and other experimental conditions.
use crate::dataframe::{DataValue, ResearchDataFrame};
use crate::effect::{ConfidenceInterval, EffectSize};
/// Error type for comparison operations.
#[derive(Debug, Clone)]
pub enum ComparisonError {
    NoData(String),
    InvalidInput(String),
}
impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonError::NoData(e) => write!(f, "no data: {e}"),
            ComparisonError::InvalidInput(e) => write!(f, "invalid input: {e}"),
        }
    }
}
/// Result of comparing two conditions.
#[derive(Debug, Clone)]
pub struct ConditionComparison {
    pub metric: String,
    pub condition_a: String,
    pub condition_b: String,
    pub mean_a: f64,
    pub mean_b: f64,
    pub difference: f64,
    pub ci: ConfidenceInterval,
    pub effect_size: Option<EffectSize>,
}
/// Result of comparing across factor levels.
#[derive(Debug, Clone)]
pub struct LevelComparison {
    pub factor: String,
    pub level: String,
    pub mean: f64,
    pub ci: ConfidenceInterval,
    pub n: usize,
}
/// The comparison engine .
pub struct ComparisonEngine {
    pub data: ResearchDataFrame,
}
impl ComparisonEngine {
    pub fn new(data: ResearchDataFrame) -> Self {
        Self { data }
    }
    /// Compare two conditions on a metric.
    pub fn compare_conditions(
        &self,
        metric: &str,
        condition_a: &str,
        condition_b: &str,
    ) -> Result<ConditionComparison, ComparisonError> {
        let values_a = self.extract_condition_values(metric, condition_a)?;
        let values_b = self.extract_condition_values(metric, condition_b)?;
        if values_a.is_empty() || values_b.is_empty() {
            return Err(ComparisonError::NoData("empty condition".to_string()));
        }
        let mean_a = values_a.iter().sum::<f64>() / values_a.len() as f64;
        let mean_b = values_b.iter().sum::<f64>() / values_b.len() as f64;
        let difference = mean_b - mean_a;
        let ci = crate::effect::welch_ci(&values_a, &values_b, 0.95);
        Ok(ConditionComparison {
            metric: metric.to_string(),
            condition_a: condition_a.to_string(),
            condition_b: condition_b.to_string(),
            mean_a,
            mean_b,
            difference,
            ci,
            effect_size: None,
        })
    }
    fn extract_condition_values(
        &self,
        metric: &str,
        condition: &str,
    ) -> Result<Vec<f64>, ComparisonError> {
        let metric_idx = self
            .data
            .columns
            .iter()
            .position(|c| c.name == "metric")
            .ok_or_else(|| ComparisonError::NoData("no metric column".to_string()))?;
        let value_idx = self
            .data
            .columns
            .iter()
            .position(|c| c.name == "value")
            .ok_or_else(|| ComparisonError::NoData("no value column".to_string()))?;
        let condition_idx = self
            .data
            .columns
            .iter()
            .position(|c| c.name == "condition")
            .ok_or_else(|| ComparisonError::NoData("no condition column".to_string()))?;
        let values: Vec<f64> = self
            .data
            .rows
            .iter()
            .filter(|row| {
                row[metric_idx] == DataValue::Str(metric.to_string())
                    && row[condition_idx] == DataValue::Str(condition.to_string())
            })
            .filter_map(|row| match &row[value_idx] {
                DataValue::Float(f) => Some(*f),
                DataValue::Int(i) => Some(*i as f64),
                _ => None,
            })
            .collect();
        if values.is_empty() {
            return Err(ComparisonError::NoData("no matching values".to_string()));
        }
        Ok(values)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataframe::{Column, DataType, DataValue, ResearchDataFrame};
    fn sample_df() -> ResearchDataFrame {
        let mut df = ResearchDataFrame::new(vec![
            Column {
                name: "metric".to_string(),
                dtype: DataType::Categorical,
            },
            Column {
                name: "condition".to_string(),
                dtype: DataType::Categorical,
            },
            Column {
                name: "value".to_string(),
                dtype: DataType::Numeric,
            },
        ]);
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("batch".to_string()),
            DataValue::Float(5.0),
        ]);
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("batch".to_string()),
            DataValue::Float(3.0),
        ]);
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("expanding".to_string()),
            DataValue::Float(4.0),
        ]);
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Str("expanding".to_string()),
            DataValue::Float(6.0),
        ]);
        df
    }
    #[test]
    fn compare_conditions_works() {
        let engine = ComparisonEngine::new(sample_df());
        let comparison = engine
            .compare_conditions("queue_time", "batch", "expanding")
            .unwrap();
        assert_eq!(comparison.metric, "queue_time");
        assert_eq!(comparison.condition_a, "batch");
        assert_eq!(comparison.condition_b, "expanding");
        assert!((comparison.mean_a - 4.0).abs() < 1e-9);
        assert!((comparison.mean_b - 5.0).abs() < 1e-9);
        assert!((comparison.difference - 1.0).abs() < 1e-9);
    }
}
