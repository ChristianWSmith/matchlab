//! Statistical analysis interface: exposes v0.3's statistical
//! machinery through the workbench, allowing researchers to perform common
//! analyses directly against stored experiment results.
use crate::dataframe::ResearchDataFrame;
use crate::effect::ConfidenceInterval;
use crate::multiple_comparisons::Correction;
use crate::power::{PowerSpec, required_replications};
/// Error type for analysis operations.
#[derive(Debug, Clone)]
pub enum AnalysisError {
    NoData(String),
    InvalidInput(String),
    ComputationError(String),
}
impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::NoData(e) => write!(f, "no data: {e}"),
            AnalysisError::InvalidInput(e) => write!(f, "invalid input: {e}"),
            AnalysisError::ComputationError(e) => write!(f, "computation error: {e}"),
        }
    }
}
/// Power analysis result.
#[derive(Debug, Clone)]
pub struct PowerAnalysis {
    pub required_n: u64,
    pub achieved_power: f64,
    pub effect_sd: f64,
    pub alpha: f64,
}
/// The statistical analysis API : connects statistical analyses to the
/// study's existing estimands, design, replication structure, and provenance.
pub struct AnalysisAPI {
    pub data: ResearchDataFrame,
}
impl AnalysisAPI {
    pub fn new(data: ResearchDataFrame) -> Self {
        Self { data }
    }
    /// Compute a confidence interval for a numeric column.
    pub fn compute_ci(&self, column: &str, conf: f64) -> Result<ConfidenceInterval, AnalysisError> {
        let values = self.extract_numeric(column)?;
        if values.is_empty() {
            return Err(AnalysisError::NoData("empty column".to_string()));
        }
        Ok(crate::effect::student_t_ci(&values, conf))
    }
    /// Compute the mean of a numeric column.
    pub fn compute_mean(&self, column: &str) -> Result<f64, AnalysisError> {
        let values = self.extract_numeric(column)?;
        if values.is_empty() {
            return Err(AnalysisError::NoData("empty column".to_string()));
        }
        Ok(values.iter().sum::<f64>() / values.len() as f64)
    }
    /// Compute the standard deviation of a numeric column.
    pub fn compute_stddev(&self, column: &str) -> Result<f64, AnalysisError> {
        let values = self.extract_numeric(column)?;
        if values.len() < 2 {
            return Err(AnalysisError::NoData("need at least 2 values".to_string()));
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
        Ok(variance.sqrt())
    }
    /// Apply a multiple-comparison correction to p-values.
    pub fn apply_correction(p_values: &[f64], method: &Correction) -> Vec<f64> {
        match method {
            Correction::Holm => crate::multiple_comparisons::holm(p_values),
            Correction::BenjaminiHochberg => {
                crate::multiple_comparisons::benjamini_hochberg(p_values)
            }
        }
    }
    /// Compute power analysis for a given effect size.
    pub fn compute_power(
        effect_sd: f64,
        minimum_effect: f64,
        alpha: f64,
        target_power: f64,
    ) -> PowerAnalysis {
        let spec = PowerSpec {
            alpha,
            target_power,
            minimum_effect,
        };
        let n = required_replications(effect_sd, &spec);
        let power = crate::power::achieved_power(effect_sd, n, minimum_effect, alpha);
        PowerAnalysis {
            required_n: n,
            achieved_power: power,
            effect_sd,
            alpha,
        }
    }
    fn extract_numeric(&self, column: &str) -> Result<Vec<f64>, AnalysisError> {
        let col_idx = match self.data.columns.iter().position(|c| c.name == column) {
            Some(idx) => idx,
            None => {
                return Err(AnalysisError::InvalidInput(format!(
                    "column '{column}' not found"
                )));
            }
        };
        let mut result = Vec::new();
        for row in &self.data.rows {
            match &row[col_idx] {
                crate::dataframe::DataValue::Float(f) => result.push(*f),
                crate::dataframe::DataValue::Int(i) => result.push(*i as f64),
                _ => {}
            }
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataframe::{Column, DataType, DataValue, ResearchDataFrame};
    fn sample_api() -> AnalysisAPI {
        let mut df = ResearchDataFrame::new(vec![
            Column {
                name: "metric".to_string(),
                dtype: DataType::Categorical,
            },
            Column {
                name: "value".to_string(),
                dtype: DataType::Numeric,
            },
        ]);
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Float(5.0),
        ]);
        df.add_row(vec![
            DataValue::Str("match_quality".to_string()),
            DataValue::Float(0.9),
        ]);
        df.add_row(vec![
            DataValue::Str("queue_time".to_string()),
            DataValue::Float(3.0),
        ]);
        AnalysisAPI::new(df)
    }
    #[test]
    fn compute_mean_works() {
        let api = sample_api();
        let mean = api.compute_mean("value").unwrap();
        assert!((mean - 2.966666).abs() < 0.001);
    }
    #[test]
    fn compute_ci_works() {
        let api = sample_api();
        let ci = api.compute_ci("value", 0.95).unwrap();
        assert!(ci.lower < ci.upper);
    }
    #[test]
    fn apply_correction_works() {
        let p_values = vec![0.01, 0.04, 0.03, 0.20];
        let corrected = AnalysisAPI::apply_correction(&p_values, &Correction::Holm);
        assert_eq!(corrected.len(), 4);
        for c in &corrected {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
    }
    #[test]
    fn compute_power_works() {
        let result = AnalysisAPI::compute_power(15.0, 10.0, 0.05, 0.80);
        assert!(result.required_n > 0);
        assert!(result.achieved_power > 0.0);
    }
}
