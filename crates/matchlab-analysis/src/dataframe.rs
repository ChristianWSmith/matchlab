//! Analysis data model: standardized tabular/data-frame-like
//! representation for research results — a stable research data interchange
//! model for analysis, plotting, export, and reports.
use matchlab_experiments::identity::*;
/// Data types for columns.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Numeric,
    Categorical,
    Boolean,
    Timestamp,
    String,
}
/// A value in a data frame cell.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum DataValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    Str(String),
    Null,
}
/// A column in the data frame.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    pub name: String,
    pub dtype: DataType,
}
/// Provenance for the data frame.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DataProvenance {
    pub source_study: Option<StudyId>,
    pub source_experiment: Option<ExperimentId>,
    pub source_metric: Option<String>,
}
/// A standardized research data frame.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResearchDataFrame {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<DataValue>>,
    pub provenance: Option<DataProvenance>,
}
impl ResearchDataFrame {
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            provenance: None,
        }
    }
    pub fn add_row(&mut self, row: Vec<DataValue>) {
        self.rows.push(row);
    }
    pub fn n_rows(&self) -> usize {
        self.rows.len()
    }
    pub fn n_cols(&self) -> usize {
        self.columns.len()
    }
    /// Select specific columns.
    pub fn select(&self, column_names: &[&str]) -> ResearchDataFrame {
        let indices: Vec<usize> = column_names
            .iter()
            .filter_map(|name| self.columns.iter().position(|c| c.name == *name))
            .collect();
        let new_columns: Vec<Column> = indices.iter().map(|&i| self.columns[i].clone()).collect();
        let new_rows: Vec<Vec<DataValue>> = self
            .rows
            .iter()
            .map(|row| indices.iter().map(|&i| row[i].clone()).collect())
            .collect();
        ResearchDataFrame {
            columns: new_columns,
            rows: new_rows,
            provenance: self.provenance.clone(),
        }
    }
    /// Filter rows by a predicate on a column.
    pub fn filter<F>(&self, column_name: &str, predicate: F) -> ResearchDataFrame
    where
        F: Fn(&DataValue) -> bool,
    {
        let col_idx = self
            .columns
            .iter()
            .position(|c| c.name == column_name)
            .expect("column not found");
        let new_rows: Vec<Vec<DataValue>> = self
            .rows
            .iter()
            .filter(|row| predicate(&row[col_idx]))
            .cloned()
            .collect();
        ResearchDataFrame {
            columns: self.columns.clone(),
            rows: new_rows,
            provenance: self.provenance.clone(),
        }
    }
    /// Sort by a column.
    pub fn sort_by(&mut self, column_name: &str, ascending: bool) {
        let col_idx = self
            .columns
            .iter()
            .position(|c| c.name == column_name)
            .expect("column not found");
        self.rows.sort_by(|a, b| {
            let cmp = match (&a[col_idx], &b[col_idx]) {
                (DataValue::Float(x), DataValue::Float(y)) => x.partial_cmp(y).unwrap(),
                (DataValue::Int(x), DataValue::Int(y)) => x.cmp(y),
                (DataValue::Str(x), DataValue::Str(y)) => x.cmp(y),
                _ => std::cmp::Ordering::Equal,
            };
            if ascending { cmp } else { cmp.reverse() }
        });
    }
    /// Take only the first n rows.
    pub fn limit(&self, n: usize) -> ResearchDataFrame {
        ResearchDataFrame {
            columns: self.columns.clone(),
            rows: self.rows.iter().take(n).cloned().collect(),
            provenance: self.provenance.clone(),
        }
    }
    /// Convert to CSV string.
    pub fn to_csv(&self) -> String {
        let header: Vec<&str> = self.columns.iter().map(|c| c.name.as_str()).collect();
        let mut out = header.join(",") + "\n";
        for row in &self.rows {
            let cells: Vec<String> = row
                .iter()
                .map(|v| match v {
                    DataValue::Float(f) => format!("{f}"),
                    DataValue::Int(i) => format!("{i}"),
                    DataValue::Bool(b) => format!("{b}"),
                    DataValue::Str(s) => s.clone(),
                    DataValue::Null => String::new(),
                })
                .collect();
            out += &cells.join(",");
            out += "\n";
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample_df() -> ResearchDataFrame {
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
        df
    }
    #[test]
    fn basic_basics() {
        let df = sample_df();
        assert_eq!(df.n_rows(), 3);
        assert_eq!(df.n_cols(), 2);
    }
    #[test]
    fn select_columns() {
        let df = sample_df();
        let selected = df.select(&["value"]);
        assert_eq!(selected.n_cols(), 1);
        assert_eq!(selected.n_rows(), 3);
    }
    #[test]
    fn filter_rows() {
        let df = sample_df();
        let filtered = df.filter("metric", |v| *v == DataValue::Str("queue_time".to_string()));
        assert_eq!(filtered.n_rows(), 2);
    }
    #[test]
    fn sort_by_column() {
        let mut df = sample_df();
        df.sort_by("value", true);
        let values: Vec<f64> = df
            .rows
            .iter()
            .filter_map(|r| match &r[1] {
                DataValue::Float(f) => Some(*f),
                _ => None,
            })
            .collect();
        assert!(values.windows(2).all(|w| w[0] <= w[1]));
    }
    #[test]
    fn limit_rows() {
        let df = sample_df();
        let limited = df.limit(2);
        assert_eq!(limited.n_rows(), 2);
    }
    #[test]
    fn to_csv() {
        let df = sample_df();
        let csv = df.to_csv();
        assert!(csv.contains("metric,value"));
        assert!(csv.contains("queue_time"));
    }
}
