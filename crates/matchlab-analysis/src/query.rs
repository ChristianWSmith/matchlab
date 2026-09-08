//! Analysis query API: unified API for retrieving experiment
//! data. Researchers query studies, experiments, conditions, replications,
//! metrics, observations, and provenance without directly querying storage.
use matchlab_experiments::identity::*;
/// An analysis query specifying what data to retrieve.
#[derive(Debug, Clone, Default)]
pub struct AnalysisQuery {
    pub study_id: Option<StudyId>,
    pub experiment_id: Option<ExperimentId>,
    pub metric_filter: Vec<String>,
    pub condition_filter: Vec<String>,
    pub replication_range: Option<(u64, u64)>,
    pub group_by: Vec<String>,
    pub order_by: Option<String>,
    pub limit: Option<usize>,
}
/// A single row of query results.
#[derive(Debug, Clone)]
pub struct QueryRow {
    pub values: Vec<DataValue>,
    pub provenance: RowProvenance,
}
/// A value in a query result row.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    Float(f64),
    Int(i64),
    String(String),
    Bool(bool),
    Null,
}
/// Provenance for a single row.
#[derive(Debug, Clone)]
pub struct RowProvenance {
    pub study_id: Option<StudyId>,
    pub experiment_id: Option<ExperimentId>,
    pub replication_id: Option<ReplicationId>,
    pub metric: Option<String>,
}
/// Result of a query: columns + rows.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<QueryRow>,
}
impl QueryResult {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
}
impl AnalysisQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn for_study(study_id: StudyId) -> Self {
        Self {
            study_id: Some(study_id),
            ..Self::default()
        }
    }
    pub fn filter_metric(mut self, metric: &str) -> Self {
        self.metric_filter.push(metric.to_string());
        self
    }
    pub fn filter_condition(mut self, condition: &str) -> Self {
        self.condition_filter.push(condition.to_string());
        self
    }
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
    pub fn order_by(mut self, column: &str) -> Self {
        self.order_by = Some(column.to_string());
        self
    }
}
/// Execute a query against in-memory test data (placeholder for real storage).
pub fn execute_query(_query: &AnalysisQuery) -> QueryResult {
    QueryResult {
        columns: vec![],
        rows: vec![],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_result_basics() {
        let result = QueryResult {
            columns: vec!["metric".to_string(), "value".to_string()],
            rows: vec![QueryRow {
                values: vec![
                    DataValue::String("queue_time".to_string()),
                    DataValue::Float(5.0),
                ],
                provenance: RowProvenance {
                    study_id: Some(StudyId("s1".to_string())),
                    experiment_id: None,
                    replication_id: None,
                    metric: Some("queue_time".to_string()),
                },
            }],
        };
        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
    }
    #[test]
    fn query_builder() {
        let query = AnalysisQuery::for_study(StudyId("s1".to_string()))
            .filter_metric("queue_time")
            .filter_condition("batch")
            .limit(100)
            .order_by("queue_time");
        assert_eq!(query.study_id, Some(StudyId("s1".to_string())));
        assert_eq!(query.metric_filter, vec!["queue_time".to_string()]);
        assert_eq!(query.condition_filter, vec!["batch".to_string()]);
        assert_eq!(query.limit, Some(100));
    }
    #[test]
    fn data_value_equality() {
        assert_eq!(DataValue::Float(1.0), DataValue::Float(1.0));
        assert_eq!(DataValue::Int(42), DataValue::Int(42));
        assert_eq!(
            DataValue::String("test".to_string()),
            DataValue::String("test".to_string())
        );
        assert_ne!(DataValue::Float(1.0), DataValue::Float(2.0));
    }
}
