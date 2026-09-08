//! Interactive study exploration: navigation layer over stored
//! experiments, allowing researchers to move between study, conditions,
//! replications, metrics, and provenance.
use matchlab_experiments::identity::*;
/// The current navigation view.
#[derive(Debug, Clone)]
pub enum NavigationView {
    Overview,
    Design,
    Conditions(Vec<String>),
    Replications(Vec<ReplicationId>),
    Metrics(Vec<String>),
    Observations { condition: String, metric: String },
    StatisticalResults,
    Visualizations,
    Provenance,
}
/// A view of a condition.
#[derive(Debug, Clone)]
pub struct ConditionView {
    pub name: String,
    pub n_replications: usize,
    pub metrics: Vec<String>,
}
/// Interactive navigator for a study.
pub struct StudyNavigator {
    pub study_id: StudyId,
    pub study_name: String,
    pub conditions: Vec<ConditionView>,
    pub current_view: NavigationView,
}
impl StudyNavigator {
    pub fn new(study_id: StudyId, study_name: String, conditions: Vec<ConditionView>) -> Self {
        Self {
            study_id,
            study_name,
            conditions,
            current_view: NavigationView::Overview,
        }
    }
    /// Navigate to a specific view.
    pub fn navigate(&mut self, view: NavigationView) {
        self.current_view = view;
    }
    /// Get the current view.
    pub fn current_view(&self) -> &NavigationView {
        &self.current_view
    }
    /// Drill down into a condition.
    pub fn drill_condition(&mut self, condition: &str) {
        self.current_view = NavigationView::Conditions(vec![condition.to_string()]);
    }
    /// Drill down into a replication.
    pub fn drill_replication(&mut self, rep_id: ReplicationId) {
        self.current_view = NavigationView::Replications(vec![rep_id]);
    }
    /// Back to overview.
    pub fn back_to_overview(&mut self) {
        self.current_view = NavigationView::Overview;
    }
    /// Get a description of the current view.
    pub fn describe(&self) -> String {
        match &self.current_view {
            NavigationView::Overview => format!("Study: {}", self.study_name),
            NavigationView::Design => "Design view".to_string(),
            NavigationView::Conditions(conds) => format!("Conditions: {}", conds.join(", ")),
            NavigationView::Replications(reps) => format!("Replications: {} items", reps.len()),
            NavigationView::Metrics(metrics) => format!("Metrics: {}", metrics.join(", ")),
            NavigationView::Observations { condition, metric } => {
                format!("Observations: {condition} / {metric}")
            }
            NavigationView::StatisticalResults => "Statistical Results".to_string(),
            NavigationView::Visualizations => "Visualizations".to_string(),
            NavigationView::Provenance => "Provenance".to_string(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigator_starts_at_overview() {
        let nav = StudyNavigator::new(StudyId("s1".to_string()), "test study".to_string(), vec![]);
        assert!(matches!(nav.current_view(), NavigationView::Overview));
    }
    #[test]
    fn navigator_drill_condition() {
        let mut nav =
            StudyNavigator::new(StudyId("s1".to_string()), "test study".to_string(), vec![]);
        nav.drill_condition("batch");
        assert!(matches!(nav.current_view(), NavigationView::Conditions(_)));
    }
    #[test]
    fn navigator_back_to_overview() {
        let mut nav =
            StudyNavigator::new(StudyId("s1".to_string()), "test study".to_string(), vec![]);
        nav.drill_condition("batch");
        nav.back_to_overview();
        assert!(matches!(nav.current_view(), NavigationView::Overview));
    }
    #[test]
    fn navigator_describe() {
        let nav = StudyNavigator::new(StudyId("s1".to_string()), "test study".to_string(), vec![]);
        let desc = nav.describe();
        assert!(desc.contains("test study"));
    }
}
